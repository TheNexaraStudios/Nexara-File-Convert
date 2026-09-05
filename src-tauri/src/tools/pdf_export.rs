use std::path::Path;

use crate::conversion::jobs::JobRegistry;
use crate::conversion::process;

use super::pages::to_mutool_range;
use super::MultiOutcome;

/// mutool's raster output has no JPEG/WebP writer — verified directly:
/// `mutool convert -o out.jpg ...` exits 0 but silently writes nothing at
/// all. PNG is the only raster format it genuinely produces, so JPG/WebP
/// output renders to PNG first and hands off to ImageMagick, the same
/// cross-engine pattern already used for Markdown → PDF.
const MUTOOL_RASTER_FORMATS: &[&str] = &["png"];

fn raster_args(input: &str, temp_pattern: &str, pages: &[u32], dpi: u32) -> Vec<String> {
    vec![
        "convert".to_string(),
        "-o".to_string(),
        temp_pattern.to_string(),
        "-O".to_string(),
        format!("resolution={dpi}"),
        input.to_string(),
        to_mutool_range(pages),
    ]
}

fn magick_convert_args(input: &Path, output: &Path, format: &str) -> Vec<String> {
    let mut args = vec![input.to_string_lossy().to_string()];
    if format == "jpg" {
        // Transparent PDF backgrounds turn black in JPEG without this —
        // same fix already applied for every other JPG output in the app.
        args.push("-background".to_string());
        args.push("white".to_string());
        args.push("-flatten".to_string());
    }
    args.push(output.to_string_lossy().to_string());
    args
}

/// Renders `pages` from `input` into `output_dir` as `<base_name>-page-NNN.<format>`.
///
/// mutool substitutes each selected page's *sequential position* into `%d`,
/// not its real page number — verified directly: selecting pages "1,3"
/// produced `raster-1.png`/`raster-2.png`, not `raster-1.png`/`raster-3.png`.
/// So mutool always renders into a throwaway sequential-numbered pattern
/// here, and each result is renamed afterward using `pages[i]` (the same
/// ordered list that built mutool's selector, so position `i` in the
/// output always corresponds to `pages[i]`).
pub async fn export_pages(
    registry: &JobRegistry,
    job_id: &str,
    input: &str,
    output_dir: &Path,
    base_name: &str,
    pages: &[u32],
    dpi: u32,
    format: &str,
) -> MultiOutcome {
    let temp_pattern = output_dir.join("nexara-raster-%d.png");
    let args = raster_args(input, &temp_pattern.to_string_lossy(), pages, dpi);

    let outcome = match process::run_and_track(registry, job_id, "mutool", &args).await {
        Ok(o) => o,
        Err(e) => return MultiOutcome::Failed(e),
    };
    if outcome.cancelled {
        return MultiOutcome::Cancelled;
    }
    if !outcome.success {
        let detail =
            if outcome.stderr_tail.trim().is_empty() { "mutool exited with an error rendering pages".to_string() } else { outcome.stderr_tail };
        return MultiOutcome::Failed(detail);
    }

    let mut outputs = Vec::with_capacity(pages.len());
    for (i, &page) in pages.iter().enumerate() {
        let sequential = i + 1;
        let rendered = output_dir.join(format!("nexara-raster-{sequential}.png"));
        if !rendered.is_file() {
            return MultiOutcome::Failed(format!("expected page {page}'s render at {} but it's missing", rendered.display()));
        }

        let final_path = output_dir.join(format!("{base_name}-page-{page:03}.{format}"));

        if MUTOOL_RASTER_FORMATS.contains(&format) {
            if let Err(e) = std::fs::rename(&rendered, &final_path) {
                return MultiOutcome::Failed(format!("couldn't move page {page}'s render into place: {e}"));
            }
        } else {
            let magick_args = magick_convert_args(&rendered, &final_path, format);
            let magick_outcome = match process::run_and_track(registry, job_id, "magick", &magick_args).await {
                Ok(o) => o,
                Err(e) => {
                    let _ = std::fs::remove_file(&rendered);
                    return MultiOutcome::Failed(e);
                }
            };
            let _ = std::fs::remove_file(&rendered);
            if magick_outcome.cancelled {
                return MultiOutcome::Cancelled;
            }
            if !magick_outcome.success {
                let detail = if magick_outcome.stderr_tail.trim().is_empty() {
                    format!("ImageMagick exited with an error converting page {page}")
                } else {
                    magick_outcome.stderr_tail
                };
                return MultiOutcome::Failed(detail);
            }
        }

        if !final_path.is_file() {
            return MultiOutcome::Failed(format!("page {page}'s output is missing after conversion"));
        }
        outputs.push(final_path);
    }

    MultiOutcome::Completed(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_args_includes_resolution_and_collapsed_page_range() {
        let args = raster_args(r"C:\in.pdf", r"C:\tmp\raster-%d.png", &[1, 2, 3], 150);
        assert!(args.contains(&"resolution=150".to_string()));
        assert_eq!(args.last().unwrap(), "1-3");
        assert_eq!(args[2], r"C:\tmp\raster-%d.png");
    }

    #[test]
    fn magick_convert_args_flattens_only_for_jpg() {
        let jpg_args = magick_convert_args(Path::new("in.png"), Path::new("out.jpg"), "jpg");
        assert!(jpg_args.contains(&"-flatten".to_string()));

        let webp_args = magick_convert_args(Path::new("in.png"), Path::new("out.webp"), "webp");
        assert!(!webp_args.contains(&"-flatten".to_string()));
    }
}
