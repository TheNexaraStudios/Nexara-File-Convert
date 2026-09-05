use std::path::Path;

use super::engine;
use super::jobs::JobRegistry;
use super::process::{self, ExecuteOutcome};

/// `mutool convert` always inserts the page number into the output
/// filename — even without an explicit `%d` pattern, `page.png` becomes
/// `page1.png` (verified directly: no separator is added). Using `%d`
/// explicitly makes that behavior obvious rather than accidental, and the
/// caller predicts the resulting filename (`<stem>1<ext>`) to move it into
/// place afterwards, the same pattern used for LibreOffice's output.
pub fn output_pattern(temp_dir: &str) -> String {
    Path::new(temp_dir).join("page-%d.png").to_string_lossy().to_string()
}

pub fn predicted_first_page_path(temp_dir: &str) -> std::path::PathBuf {
    Path::new(temp_dir).join("page-1.png")
}

/// Builds `mutool convert -o <pattern> <input> 1` — rasterizing only the
/// first page. Nexara's per-job model produces exactly one output file, so
/// a multi-page PDF can't become "N images" in a single job; the first
/// page (the common thumbnail/preview use case) is what's actually offered
/// rather than silently only ever giving page 1 while claiming to convert
/// "the PDF" in general.
pub fn build_args(input: &str, temp_dir: &str) -> Vec<String> {
    vec!["convert".to_string(), "-o".to_string(), output_pattern(temp_dir), input.to_string(), "1".to_string()]
}

/// Spawns MuPDF's `mutool` and tracks the child in the shared job registry
/// so cancellation works the same way it does for other engines.
pub async fn execute(registry: &JobRegistry, job_id: &str, args: &[String]) -> Result<ExecuteOutcome, String> {
    process::run_and_track(registry, job_id, &engine::binary_path("mutool"), args).await
}

/// A lightweight sanity check beyond "the file exists and is non-empty":
/// confirms the produced file actually starts with the PNG signature.
pub fn validate_output(path: &Path, output_format: &str) -> Result<(), String> {
    if output_format != "png" {
        return Ok(());
    }
    let mut header = [0u8; 8];
    let read_len = {
        use std::io::Read;
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        file.read(&mut header).map_err(|e| e.to_string())?
    };
    if header[..read_len].starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        Ok(())
    } else {
        Err("the output doesn't look like a valid PNG file".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_uses_explicit_page_number_pattern() {
        let args = build_args(r"C:\in\doc.pdf", r"C:\tmp");
        assert_eq!(args[0], "convert");
        assert_eq!(args[1], "-o");
        assert!(args[2].ends_with("page-%d.png"), "got: {}", args[2]);
        assert_eq!(args[3], r"C:\in\doc.pdf");
        assert_eq!(args[4], "1");
    }

    #[test]
    fn predicted_first_page_path_matches_mutools_real_naming() {
        // mutool substitutes %d with the page number and nothing else, so
        // "page-%d.png" with page 1 becomes "page-1.png" — verified against
        // the real binary.
        let predicted = predicted_first_page_path(r"C:\tmp");
        assert_eq!(predicted, std::path::Path::new(r"C:\tmp\page-1.png"));
    }

    #[test]
    fn validate_output_accepts_real_png_header() {
        let dir = std::env::temp_dir().join("nexara-test-pdf-validate");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.png");
        std::fs::write(&path, [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).unwrap();
        assert!(validate_output(&path, "png").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_rejects_wrong_png_header() {
        let dir = std::env::temp_dir().join("nexara-test-pdf-validate2");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("not-a.png");
        std::fs::write(&path, b"not a png file").unwrap();
        assert!(validate_output(&path, "png").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
