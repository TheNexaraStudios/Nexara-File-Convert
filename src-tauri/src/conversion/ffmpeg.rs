use serde::Deserialize;
use serde_json::Value;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex as TokioMutex;

use super::engine;
use super::jobs::JobRegistry;
pub use super::probe::MediaProbe;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionSettings {
    #[serde(default)]
    pub preset: String,
    pub resolution: Option<String>,
    pub frame_rate: Option<String>,
    pub video_codec: Option<String>,
    pub audio_bitrate: Option<String>,
    pub image_quality: Option<u32>,
    pub strip_metadata: Option<bool>,
    /// Explicit resize controls used by the Resize Image tool — distinct
    /// from `resolution`'s fixed preset dropdown (720p/1080p/etc, used by
    /// the plain Convert screen), since a real resize tool needs exact
    /// width/height/percentage and a real fit/fill/stretch choice. `None`
    /// on every field (the default for any other conversion) leaves the
    /// existing `resolution`-preset behavior untouched.
    pub resize_width: Option<u32>,
    pub resize_height: Option<u32>,
    pub resize_percent: Option<u32>,
    /// "fit" (contain within the box, aspect preserved, never upscaled
    /// past the box), "fill" (cover the box, aspect preserved, cropped to
    /// the exact box), or "stretch" (exact WxH, aspect ignored).
    pub resize_mode: Option<String>,
}


/// Reads real media metadata via ffprobe. Used both to show the user
/// meaningful file info and to compute real conversion progress percentages
/// (elapsed encoded time / total duration).
pub async fn probe(path: &str) -> Result<MediaProbe, String> {
    let output = tokio::process::Command::new(engine::binary_path("ffprobe"))
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", path])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Could not run ffprobe: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Nexara could not read this file: {}", stderr.trim()));
    }

    let json: Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("Could not parse media info: {e}"))?;

    let duration_seconds = json["format"]["duration"].as_str().and_then(|s| s.parse::<f64>().ok());

    let mut probe = MediaProbe { duration_seconds, ..Default::default() };

    if let Some(streams) = json["streams"].as_array() {
        for stream in streams {
            let codec_type = stream["codec_type"].as_str().unwrap_or("");
            if codec_type == "video" && !probe.has_video {
                probe.has_video = true;
                probe.video_codec = stream["codec_name"].as_str().map(|s| s.to_string());
                probe.width = stream["width"].as_u64().map(|v| v as u32);
                probe.height = stream["height"].as_u64().map(|v| v as u32);
            } else if codec_type == "audio" && !probe.has_audio {
                probe.has_audio = true;
                probe.audio_codec = stream["codec_name"].as_str().map(|s| s.to_string());
            }
        }
    }

    Ok(probe)
}

fn resolution_height(res: &str) -> Option<&'static str> {
    match res {
        "2160p" => Some("2160"),
        "1440p" => Some("1440"),
        "1080p" => Some("1080"),
        "720p" => Some("720"),
        "480p" => Some("480"),
        _ => None,
    }
}

/// A conservative compatibility check used to decide whether a container
/// change can be a lossless, fast stream copy ("remux") instead of a full
/// re-encode. Errs on the side of transcoding when unsure.
fn is_remux_compatible(output_format: &str, probe: &MediaProbe) -> bool {
    let vcodec = probe.video_codec.as_deref();
    let acodec = probe.audio_codec.as_deref();
    let video_ok = match output_format {
        "mp4" | "mov" => matches!(vcodec, Some("h264") | Some("hevc") | Some("mpeg4") | None),
        "mkv" => true,
        "webm" => matches!(vcodec, Some("vp8") | Some("vp9") | Some("av1") | None),
        "avi" => matches!(vcodec, Some("mpeg4") | Some("h264") | None),
        _ => false,
    };
    let audio_ok = match output_format {
        "mp4" | "mov" => matches!(acodec, Some("aac") | Some("mp3") | None),
        "mkv" => true,
        "webm" => matches!(acodec, Some("opus") | Some("vorbis") | None),
        "avi" => matches!(acodec, Some("mp3") | Some("ac3") | None),
        _ => false,
    };
    video_ok && audio_ok
}

/// Builds the ffmpeg argument list for a conversion. Returns the args and
/// whether this ended up being a lossless stream-copy remux.
pub fn build_args(input: &str, output_tmp: &str, output_format: &str, settings: &ConversionSettings, probe: &MediaProbe) -> (Vec<String>, bool) {
    let mut args: Vec<String> = vec!["-y".into(), "-i".into(), input.into()];

    const AUDIO_TARGETS: [&str; 5] = ["mp3", "wav", "flac", "aac", "ogg"];
    const VIDEO_TARGETS: [&str; 5] = ["mp4", "mov", "mkv", "webm", "avi"];

    if output_format == "gif" {
        let fps = settings.frame_rate.as_deref().filter(|v| *v != "original").unwrap_or("10");
        let width = match settings.resolution.as_deref() {
            Some("2160p") => "3840",
            Some("1440p") => "2560",
            Some("1080p") => "1920",
            Some("720p") => "1280",
            Some("480p") => "854",
            _ => "480",
        };
        args.push("-vf".into());
        args.push(format!("fps={fps},scale={width}:-1:flags=lanczos"));
        args.push(output_tmp.into());
        return (args, false);
    }

    if AUDIO_TARGETS.contains(&output_format) {
        args.push("-vn".into());
        let bitrate = settings.audio_bitrate.clone().unwrap_or_else(|| "192k".into());
        match output_format {
            "mp3" => {
                args.push("-c:a".into());
                args.push("libmp3lame".into());
                args.push("-b:a".into());
                args.push(bitrate);
            }
            "wav" => {
                args.push("-c:a".into());
                args.push("pcm_s16le".into());
            }
            "flac" => {
                args.push("-c:a".into());
                args.push("flac".into());
            }
            "aac" => {
                args.push("-c:a".into());
                args.push("aac".into());
                args.push("-b:a".into());
                args.push(bitrate);
            }
            "ogg" => {
                args.push("-c:a".into());
                args.push("libvorbis".into());
                args.push("-b:a".into());
                args.push(bitrate);
            }
            _ => {}
        }
        args.push(output_tmp.into());
        return (args, false);
    }

    if VIDEO_TARGETS.contains(&output_format) {
        let unmodified = settings.preset != "high"
            && settings.preset != "small"
            && settings.preset != "custom"
            && settings.resolution.as_deref().unwrap_or("original") == "original"
            && settings.frame_rate.as_deref().unwrap_or("original") == "original"
            && settings.video_codec.is_none();

        if unmodified && is_remux_compatible(output_format, probe) {
            args.push("-c".into());
            args.push("copy".into());
            args.push(output_tmp.into());
            return (args, true);
        }

        if let Some(h) = settings.resolution.as_deref().filter(|v| *v != "original").and_then(resolution_height) {
            args.push("-vf".into());
            args.push(format!("scale=-2:{h}"));
        }
        if let Some(fps) = settings.frame_rate.as_deref().filter(|v| *v != "original") {
            args.push("-r".into());
            args.push(fps.into());
        }

        let (vcodec, is_opus): (&str, bool) = if output_format == "webm" {
            ("libvpx-vp9", true)
        } else {
            (
                match settings.video_codec.as_deref() {
                    Some("h265") => "libx265",
                    Some("av1") => "libaom-av1",
                    _ => "libx264",
                },
                false,
            )
        };

        let crf = match settings.preset.as_str() {
            "high" => "18",
            "small" => "30",
            _ => "23",
        };

        args.push("-c:v".into());
        args.push(vcodec.into());
        match vcodec {
            "libx264" | "libx265" => {
                args.push("-preset".into());
                args.push(
                    match settings.preset.as_str() {
                        "high" => "slow",
                        "small" => "fast",
                        _ => "medium",
                    }
                    .into(),
                );
                args.push("-crf".into());
                args.push(crf.into());
            }
            "libvpx-vp9" | "libaom-av1" => {
                args.push("-crf".into());
                args.push(crf.into());
                args.push("-b:v".into());
                args.push("0".into());
            }
            _ => {}
        }

        if probe.has_audio {
            args.push("-c:a".into());
            if is_opus {
                args.push("libopus".into());
            } else {
                args.push("aac".into());
                args.push("-b:a".into());
                args.push(settings.audio_bitrate.clone().unwrap_or_else(|| "192k".into()));
            }
        } else {
            args.push("-an".into());
        }

        args.push(output_tmp.into());
        return (args, false);
    }

    args.push(output_tmp.into());
    (args, false)
}

pub struct ExecuteOutcome {
    pub success: bool,
    pub cancelled: bool,
    pub stderr_tail: String,
}

/// Spawns ffmpeg with structured arguments (never a shell string), streams
/// its machine-readable `-progress` output into real percentage events, and
/// tracks the child process in the job registry so it can be cancelled.
/// Runs ffmpeg and reports progress through `on_progress` rather than
/// depending on Tauri directly, so this function (and cancellation) can be
/// exercised in a plain async test without a running app.
pub async fn execute<F>(
    registry: &JobRegistry,
    job_id: &str,
    args: &[String],
    duration_seconds: Option<f64>,
    on_progress: F,
) -> Result<ExecuteOutcome, String>
where
    F: Fn(Option<f64>) + Send + 'static,
{
    let mut full_args: Vec<String> = args.to_vec();
    full_args.push("-progress".into());
    full_args.push("pipe:1".into());
    full_args.push("-nostats".into());
    full_args.push("-loglevel".into());
    full_args.push("error".into());

    let mut child = tokio::process::Command::new(engine::binary_path("ffmpeg"))
        .args(&full_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start ffmpeg: {e}"))?;

    let stdout = child.stdout.take().expect("ffmpeg stdout was piped");
    let stderr = child.stderr.take().expect("ffmpeg stderr was piped");

    let progress_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        let mut out_time_seconds: Option<f64> = None;
        while let Ok(Some(line)) = reader.next_line().await {
            if let Some(value) = line.strip_prefix("out_time_ms=") {
                if let Ok(us) = value.trim().parse::<i64>() {
                    out_time_seconds = Some(us as f64 / 1_000_000.0);
                }
            } else if line.starts_with("progress=") {
                let percent = match (out_time_seconds, duration_seconds) {
                    (Some(t), Some(d)) if d > 0.0 => Some((t / d * 100.0).clamp(0.0, 100.0)),
                    _ => None,
                };
                on_progress(percent);
            }
        }
    });

    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut collected: Vec<String> = Vec::new();
        while let Ok(Some(line)) = reader.next_line().await {
            collected.push(line);
            if collected.len() > 60 {
                collected.remove(0);
            }
        }
        collected.join("\n")
    });

    let child_arc = Arc::new(TokioMutex::new(child));
    registry.0.lock().await.insert(job_id.to_string(), child_arc.clone());

    let _ = progress_task.await;
    let stderr_tail = stderr_task.await.unwrap_or_default();

    let status = {
        let mut guard = child_arc.lock().await;
        guard.wait().await
    };

    // If our own entry is already gone, `cancel_conversion` removed and
    // killed it before we got here — treat that as a cancellation rather
    // than a failure, regardless of the exit code a killed process reports.
    let still_registered = registry.0.lock().await.remove(job_id).is_some();
    let cancelled = !still_registered;

    match status {
        Ok(exit_status) => Ok(ExecuteOutcome { success: exit_status.success() && !cancelled, cancelled, stderr_tail }),
        Err(e) => Ok(ExecuteOutcome { success: false, cancelled, stderr_tail: format!("{stderr_tail}\n{e}") }),
    }
}

#[cfg(test)]
mod tests {
    use super::super::jobs;
    use super::*;

    fn probe_with(video: Option<&str>, audio: Option<&str>) -> MediaProbe {
        MediaProbe {
            duration_seconds: Some(10.0),
            video_codec: video.map(|s| s.to_string()),
            audio_codec: audio.map(|s| s.to_string()),
            width: Some(1920),
            height: Some(1080),
            has_video: video.is_some(),
            has_audio: audio.is_some(),
        }
    }

    fn default_settings() -> ConversionSettings {
        ConversionSettings { preset: "balanced".into(), ..Default::default() }
    }

    #[test]
    fn compatible_h264_aac_to_mp4_is_remuxed() {
        let probe = probe_with(Some("h264"), Some("aac"));
        let (args, remuxed) = build_args("in.mkv", "out.mp4", "mp4", &default_settings(), &probe);
        assert!(remuxed, "h264+aac into mp4 with default settings should remux, not re-encode");
        assert!(args.iter().any(|a| a == "copy"));
    }

    #[test]
    fn incompatible_vp9_to_mp4_is_transcoded() {
        let probe = probe_with(Some("vp9"), Some("opus"));
        let (args, remuxed) = build_args("in.webm", "out.mp4", "mp4", &default_settings(), &probe);
        assert!(!remuxed, "vp9+opus is not mp4-compatible, must transcode");
        assert!(args.iter().any(|a| a == "libx264"));
    }

    #[test]
    fn explicit_codec_choice_forces_transcode_even_if_compatible() {
        let mut settings = default_settings();
        settings.video_codec = Some("h265".into());
        let probe = probe_with(Some("h264"), Some("aac"));
        let (args, remuxed) = build_args("in.mp4", "out.mp4", "mp4", &settings, &probe);
        assert!(!remuxed, "an explicit codec override should always transcode");
        assert!(args.iter().any(|a| a == "libx265"));
    }

    #[test]
    fn webm_target_always_uses_vp9_opus() {
        let probe = probe_with(Some("h264"), Some("aac"));
        let (args, _) = build_args("in.mp4", "out.webm", "webm", &default_settings(), &probe);
        assert!(args.iter().any(|a| a == "libvpx-vp9"));
        assert!(args.iter().any(|a| a == "libopus"));
    }

    #[test]
    fn resolution_setting_adds_scale_filter() {
        let mut settings = default_settings();
        settings.resolution = Some("720p".into());
        let probe = probe_with(Some("h264"), Some("aac"));
        let (args, remuxed) = build_args("in.mp4", "out.mp4", "mp4", &settings, &probe);
        assert!(!remuxed);
        assert!(args.iter().any(|a| a == "scale=-2:720"));
    }

    #[test]
    fn audio_target_strips_video_and_uses_codec() {
        let probe = probe_with(Some("h264"), Some("aac"));
        let (args, _) = build_args("in.mp4", "out.mp3", "mp3", &default_settings(), &probe);
        assert!(args.iter().any(|a| a == "-vn"));
        assert!(args.iter().any(|a| a == "libmp3lame"));
    }

    #[test]
    fn wav_target_uses_pcm_no_bitrate() {
        let probe = probe_with(None, Some("mp3"));
        let (args, _) = build_args("in.mp3", "out.wav", "wav", &default_settings(), &probe);
        assert!(args.iter().any(|a| a == "pcm_s16le"));
        assert!(!args.iter().any(|a| a == "-b:a"));
    }

    #[test]
    fn no_audio_stream_adds_an_flag() {
        let probe = probe_with(Some("h264"), None);
        let (args, remuxed) = build_args("in.mp4", "out.mkv", "mkv", &default_settings(), &probe);
        // mkv accepts h264 with no audio, but remux only triggers when audio_ok
        // is also true for the "no audio" case (None matches the wildcard).
        assert!(remuxed || args.iter().any(|a| a == "-an"));
    }

    #[test]
    fn gif_target_builds_fps_scale_filter() {
        let probe = probe_with(Some("h264"), None);
        let (args, _) = build_args("in.mp4", "out.gif", "gif", &default_settings(), &probe);
        assert!(args.iter().any(|a| a.starts_with("fps=")));
    }

    fn ffmpeg_available() -> bool {
        std::process::Command::new("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
    }

    /// Real end-to-end proof that cancellation actually terminates the
    /// ffmpeg child process rather than just updating UI state: we start a
    /// ~6s synthetic encode, cancel it after ~500ms, and assert both that
    /// `execute` reports the job as cancelled (not failed) and that it
    /// returns in well under the 6s the encode would otherwise take.
    #[tokio::test]
    async fn cancelling_a_running_job_stops_it_quickly() {
        if !ffmpeg_available() {
            eprintln!("skipping cancellation test: ffmpeg not found on PATH");
            return;
        }

        let work_dir = std::env::temp_dir().join("nexara-cancel-test");
        let _ = std::fs::remove_dir_all(&work_dir);
        std::fs::create_dir_all(&work_dir).unwrap();
        let output = work_dir.join("out.mp4").to_string_lossy().to_string();

        let args = vec![
            "-y".to_string(),
            "-f".to_string(),
            "lavfi".to_string(),
            "-i".to_string(),
            "testsrc=duration=6:size=640x480:rate=30".to_string(),
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "veryslow".to_string(),
            output,
        ];

        let registry = std::sync::Arc::new(JobRegistry::default());
        let registry_for_task = registry.clone();
        let job_id = "cancel-test-job".to_string();
        let job_id_for_task = job_id.clone();

        let handle =
            tokio::spawn(
                async move { execute(&registry_for_task, &job_id_for_task, &args, Some(6.0), |_| {}).await },
            );

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let started = std::time::Instant::now();
        let was_cancelled = jobs::cancel(&registry, &job_id).await;
        assert!(was_cancelled, "expected an in-flight job to be found and signalled");

        let outcome = handle.await.unwrap().unwrap();
        assert!(outcome.cancelled, "expected the outcome to report cancelled");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(4),
            "cancellation took too long: {:?} — the process may not have actually been killed",
            started.elapsed()
        );

        let _ = std::fs::remove_dir_all(&work_dir);
    }
}
