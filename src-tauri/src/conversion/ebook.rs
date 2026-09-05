use std::path::Path;

use super::engine;
use super::jobs::JobRegistry;
use super::process::{self, ExecuteOutcome};

/// Builds the `ebook-convert <input> <output>` argument list. Unlike
/// LibreOffice, Calibre lets us name the output file directly and infers
/// the target format from its extension — no filename prediction/rename
/// dance needed afterwards.
pub fn build_args(input: &str, output_tmp: &str) -> Vec<String> {
    vec![input.to_string(), output_tmp.to_string()]
}

/// Spawns Calibre's `ebook-convert` and tracks the child in the shared job
/// registry so cancellation works the same way it does for other engines.
/// A single e-book conversion reports no incremental progress worth
/// parsing, so — like image and office — this only reports completion.
pub async fn execute(registry: &JobRegistry, job_id: &str, args: &[String]) -> Result<ExecuteOutcome, String> {
    process::run_and_track(registry, job_id, &engine::binary_path("ebook-convert"), args).await
}

/// A lightweight sanity check beyond "the file exists and is non-empty":
/// EPUB is a ZIP container, so it has a reliable signature to check. MOBI,
/// AZW3, and FB2 don't have a simple fixed-offset magic byte Nexara can
/// cheaply verify, so those are accepted on size alone.
pub fn validate_output(path: &Path, output_format: &str) -> Result<(), String> {
    if output_format != "epub" {
        return Ok(());
    }
    let mut header = [0u8; 4];
    let read_len = {
        use std::io::Read;
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        file.read(&mut header).map_err(|e| e.to_string())?
    };
    if header[..read_len].starts_with(b"PK\x03\x04") {
        Ok(())
    } else {
        Err("the output doesn't look like a valid EPUB file".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_passes_input_then_output() {
        let args = build_args(r"C:\in\book.mobi", r"C:\tmp\out.epub");
        assert_eq!(args, vec![r"C:\in\book.mobi".to_string(), r"C:\tmp\out.epub".to_string()]);
    }

    #[test]
    fn validate_output_accepts_real_epub_header() {
        let dir = std::env::temp_dir().join("nexara-test-ebook-validate");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.epub");
        std::fs::write(&path, [0x50, 0x4B, 0x03, 0x04, 0]).unwrap();
        assert!(validate_output(&path, "epub").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_rejects_wrong_epub_header() {
        let dir = std::env::temp_dir().join("nexara-test-ebook-validate2");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("not-a.epub");
        std::fs::write(&path, b"not an epub file").unwrap();
        assert!(validate_output(&path, "epub").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_skips_formats_without_signature() {
        let dir = std::env::temp_dir().join("nexara-test-ebook-validate3");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.mobi");
        std::fs::write(&path, b"whatever").unwrap();
        assert!(validate_output(&path, "mobi").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
