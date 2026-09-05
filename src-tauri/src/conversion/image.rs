use std::process::Stdio;

use super::engine;
use super::ffmpeg::ConversionSettings;
use super::jobs::JobRegistry;
use super::probe::MediaProbe;
use super::process::{self, ExecuteOutcome};

/// Reads width/height for an image via ImageMagick's `identify`. Only the
/// first frame is inspected (`[0]`) so multi-frame formats like animated
/// GIF or multi-size ICO don't produce multiple results.
pub async fn probe(path: &str) -> Result<MediaProbe, String> {
    let target = format!("{path}[0]");
    let output = tokio::process::Command::new(engine::binary_path("magick"))
        .args(["identify", "-format", "%w|%h", &target])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Could not run ImageMagick: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Nexara could not read this image: {}", stderr.trim()));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.trim().split('|');
    let width = parts.next().and_then(|s| s.parse::<u32>().ok());
    let height = parts.next().and_then(|s| s.parse::<u32>().ok());

    Ok(MediaProbe { width, height, ..Default::default() })
}

/// Validates output produced by the image engine. Normally that means
/// re-reading it back through ImageMagick's `identify` (proving it's
/// genuinely decodable, not just non-empty) — but PDF is the one output
/// this engine can *write* without Ghostscript while still being unable to
/// *read* PDF without it (verified directly), so re-probing a freshly
/// written PDF would always fail even on a perfectly valid file. That case
/// falls back to a magic-byte check instead.
pub async fn validate_output(path: &str, output_format: &str) -> Result<(), String> {
    if output_format == "pdf" {
        let mut header = [0u8; 4];
        let read_len = {
            use std::io::Read;
            let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
            file.read(&mut header).map_err(|e| e.to_string())?
        };
        return if header[..read_len].starts_with(b"%PDF") {
            Ok(())
        } else {
            Err("the output doesn't look like a valid PDF file".to_string())
        };
    }
    probe(path).await.map(|_| ())
}

fn resize_height(res: &str) -> Option<&'static str> {
    match res {
        "2160p" => Some("2160"),
        "1440p" => Some("1440"),
        "1080p" => Some("1080"),
        "720p" => Some("720"),
        "480p" => Some("480"),
        _ => None,
    }
}

/// Builds the ImageMagick `-resize`/`-gravity`/`-extent` arguments for the
/// Resize Image tool's explicit width/height/percentage/mode controls.
/// Geometry syntax verified directly against this build: `WxH!` stretches
/// to the exact box ignoring aspect ratio, plain `WxH` fits *within* the
/// box preserving aspect (the resulting image can be smaller than the box
/// in one dimension), and `WxH^` combined with `-gravity center -extent
/// WxH` fills the box exactly by cropping any overflow after a
/// aspect-preserving resize.
fn explicit_resize_args(settings: &ConversionSettings) -> Vec<String> {
    let mut args = Vec::new();

    if let Some(pct) = settings.resize_percent {
        args.push("-resize".into());
        args.push(format!("{pct}%"));
        return args;
    }

    let (w, h) = (settings.resize_width, settings.resize_height);
    if w.is_none() && h.is_none() {
        return args;
    }
    let mode = settings.resize_mode.as_deref().unwrap_or("fit");

    match (w, h, mode) {
        (Some(w), Some(h), "stretch") => {
            args.push("-resize".into());
            args.push(format!("{w}x{h}!"));
        }
        (Some(w), Some(h), "fill") => {
            args.push("-resize".into());
            args.push(format!("{w}x{h}^"));
            args.push("-gravity".into());
            args.push("center".into());
            args.push("-extent".into());
            args.push(format!("{w}x{h}"));
        }
        // "fit" (or fill/stretch missing one dimension, which can't stretch
        // or crop to a box that isn't fully specified — fall back to a
        // plain aspect-preserving fit instead of guessing a value).
        (Some(w), Some(h), _) => {
            args.push("-resize".into());
            args.push(format!("{w}x{h}"));
        }
        (Some(w), None, _) => {
            args.push("-resize".into());
            args.push(format!("{w}x"));
        }
        (None, Some(h), _) => {
            args.push("-resize".into());
            args.push(format!("x{h}"));
        }
        (None, None, _) => {}
    }
    args
}

/// Builds a `magick <input> [options] <output>` argument list. ImageMagick
/// infers input/output format from each path's extension, so the temp
/// output path must already carry the target extension.
pub fn build_args(input: &str, output_tmp: &str, settings: &ConversionSettings) -> Vec<String> {
    let mut args: Vec<String> = vec![format!("{input}[0]")];

    let explicit_resize = explicit_resize_args(settings);
    if !explicit_resize.is_empty() {
        args.extend(explicit_resize);
    } else if let Some(h) = settings.resolution.as_deref().filter(|v| *v != "original").and_then(resize_height) {
        args.push("-resize".into());
        args.push(format!("x{h}"));
    }

    let quality = settings.image_quality.unwrap_or(match settings.preset.as_str() {
        "high" => 95,
        "small" => 65,
        _ => 85,
    });
    args.push("-quality".into());
    args.push(quality.to_string());

    if settings.strip_metadata.unwrap_or(false) {
        args.push("-strip".into());
    }

    // Flatten transparency onto white when writing a format with no alpha
    // channel, instead of leaving it to ImageMagick's per-format default
    // (which can silently produce a black background for some formats).
    if output_tmp.to_lowercase().ends_with(".jpg") || output_tmp.to_lowercase().ends_with(".jpeg") {
        args.push("-background".into());
        args.push("white".into());
        args.push("-flatten".into());
    }

    args.push(output_tmp.into());
    args
}

/// Spawns ImageMagick and tracks the child in the shared job registry so
/// cancellation works the same way it does for ffmpeg jobs. Image
/// conversions are normally too fast to report meaningful progress, so this
/// reports only completion, not intermediate percentages.
pub async fn execute(registry: &JobRegistry, job_id: &str, args: &[String]) -> Result<ExecuteOutcome, String> {
    process::run_and_track(registry, job_id, &engine::binary_path("magick"), args).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_settings() -> ConversionSettings {
        ConversionSettings { preset: "balanced".into(), ..Default::default() }
    }

    #[tokio::test]
    async fn validate_output_checks_pdf_magic_bytes_without_reading_it_back() {
        // Regression test: this must NOT call ImageMagick's `identify` on a
        // PDF, because reading PDF back requires a Ghostscript delegate
        // this build doesn't have — even a perfectly valid PDF would fail
        // that round-trip. A magic-byte check is used instead.
        let dir = std::env::temp_dir().join("nexara-test-image-validate-pdf");
        let _ = std::fs::create_dir_all(&dir);
        let good = dir.join("good.pdf");
        std::fs::write(&good, b"%PDF-1.7\n...").unwrap();
        assert!(validate_output(good.to_str().unwrap(), "pdf").await.is_ok());

        let bad = dir.join("bad.pdf");
        std::fs::write(&bad, b"not a pdf").unwrap();
        assert!(validate_output(bad.to_str().unwrap(), "pdf").await.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn explicit_resize_percent_overrides_width_height() {
        let mut settings = default_settings();
        settings.resize_percent = Some(50);
        settings.resize_width = Some(999);
        let args = build_args("in.png", "out.png", &settings);
        let idx = args.iter().position(|a| a == "-resize").unwrap();
        assert_eq!(args[idx + 1], "50%");
    }

    #[test]
    fn explicit_resize_stretch_uses_bang_geometry() {
        let mut settings = default_settings();
        settings.resize_width = Some(200);
        settings.resize_height = Some(100);
        settings.resize_mode = Some("stretch".into());
        let args = build_args("in.png", "out.png", &settings);
        let idx = args.iter().position(|a| a == "-resize").unwrap();
        assert_eq!(args[idx + 1], "200x100!");
    }

    #[test]
    fn explicit_resize_fit_uses_plain_geometry() {
        let mut settings = default_settings();
        settings.resize_width = Some(200);
        settings.resize_height = Some(100);
        settings.resize_mode = Some("fit".into());
        let args = build_args("in.png", "out.png", &settings);
        let idx = args.iter().position(|a| a == "-resize").unwrap();
        assert_eq!(args[idx + 1], "200x100");
    }

    #[test]
    fn explicit_resize_fill_resizes_then_crops_to_extent() {
        let mut settings = default_settings();
        settings.resize_width = Some(200);
        settings.resize_height = Some(100);
        settings.resize_mode = Some("fill".into());
        let args = build_args("in.png", "out.png", &settings);
        let resize_idx = args.iter().position(|a| a == "-resize").unwrap();
        assert_eq!(args[resize_idx + 1], "200x100^");
        assert!(args.contains(&"-gravity".to_string()));
        assert!(args.contains(&"-extent".to_string()));
        let extent_idx = args.iter().position(|a| a == "-extent").unwrap();
        assert_eq!(args[extent_idx + 1], "200x100");
    }

    #[test]
    fn explicit_resize_width_only_preserves_aspect() {
        let mut settings = default_settings();
        settings.resize_width = Some(150);
        let args = build_args("in.png", "out.png", &settings);
        let idx = args.iter().position(|a| a == "-resize").unwrap();
        assert_eq!(args[idx + 1], "150x");
    }

    #[test]
    fn explicit_resize_height_only_preserves_aspect() {
        let mut settings = default_settings();
        settings.resize_height = Some(150);
        let args = build_args("in.png", "out.png", &settings);
        let idx = args.iter().position(|a| a == "-resize").unwrap();
        assert_eq!(args[idx + 1], "x150");
    }

    #[test]
    fn explicit_resize_absent_falls_back_to_resolution_preset() {
        let mut settings = default_settings();
        settings.resolution = Some("720p".into());
        let args = build_args("in.png", "out.png", &settings);
        assert!(args.iter().any(|a| a == "-resize"));
        assert!(args.iter().any(|a| a == "x720"));
    }

    #[test]
    fn quality_preset_maps_to_expected_value() {
        let mut settings = default_settings();
        settings.preset = "high".into();
        let args = build_args("in.png", "out.jpg", &settings);
        let idx = args.iter().position(|a| a == "-quality").unwrap();
        assert_eq!(args[idx + 1], "95");
    }

    #[test]
    fn explicit_image_quality_overrides_preset() {
        let mut settings = default_settings();
        settings.preset = "high".into();
        settings.image_quality = Some(42);
        let args = build_args("in.png", "out.jpg", &settings);
        let idx = args.iter().position(|a| a == "-quality").unwrap();
        assert_eq!(args[idx + 1], "42");
    }

    #[test]
    fn resolution_setting_adds_resize_flag() {
        let mut settings = default_settings();
        settings.resolution = Some("720p".into());
        let args = build_args("in.png", "out.png", &settings);
        assert!(args.iter().any(|a| a == "-resize"));
        assert!(args.iter().any(|a| a == "x720"));
    }

    #[test]
    fn strip_metadata_adds_strip_flag() {
        let mut settings = default_settings();
        settings.strip_metadata = Some(true);
        let args = build_args("in.png", "out.png", &settings);
        assert!(args.iter().any(|a| a == "-strip"));
    }

    #[test]
    fn jpg_output_flattens_transparency() {
        let args = build_args("in.png", "out.jpg", &default_settings());
        assert!(args.iter().any(|a| a == "-flatten"));
    }

    #[test]
    fn png_output_does_not_flatten() {
        let args = build_args("in.jpg", "out.png", &default_settings());
        assert!(!args.iter().any(|a| a == "-flatten"));
    }
}
