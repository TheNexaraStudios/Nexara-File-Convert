use std::path::Path;

use crate::conversion::jobs::JobRegistry;
use crate::conversion::process::{self, ExecuteOutcome};

/// Builds `mutool merge -o <output> <input1> <input2> ...` — `mutool merge`
/// is a real, dedicated MuPDF command for exactly this (verified directly:
/// merging a 1-page, a 1-page, and a 3-page PDF produced a genuine 5-page
/// result), so no extra dependency is needed.
pub fn build_args(inputs: &[String], output_path: &str) -> Vec<String> {
    let mut args = vec!["merge".to_string(), "-o".to_string(), output_path.to_string()];
    args.extend(inputs.iter().cloned());
    args
}

pub async fn execute(registry: &JobRegistry, job_id: &str, args: &[String]) -> Result<ExecuteOutcome, String> {
    process::run_and_track(registry, job_id, "mutool", args).await
}

/// Confirms the merged output is a real PDF with the expected page count —
/// not just "a file exists", since a partial/corrupt merge could still
/// produce a non-empty file.
pub async fn validate_output(path: &Path, expected_min_pages: u32) -> Result<(), String> {
    let mut header = [0u8; 4];
    let read_len = {
        use std::io::Read;
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        file.read(&mut header).map_err(|e| e.to_string())?
    };
    if !header[..read_len].starts_with(b"%PDF") {
        return Err("the output doesn't look like a valid PDF file".to_string());
    }

    let actual = super::pages::page_count(&path.to_string_lossy()).await?;
    if actual < expected_min_pages {
        return Err(format!("expected at least {expected_min_pages} page(s) in the merged PDF, but found {actual}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_lists_output_then_every_input_in_order() {
        let inputs = vec![r"C:\a.pdf".to_string(), r"C:\b.pdf".to_string(), r"C:\c.pdf".to_string()];
        let args = build_args(&inputs, r"C:\out.pdf");
        assert_eq!(args, vec!["merge", "-o", r"C:\out.pdf", r"C:\a.pdf", r"C:\b.pdf", r"C:\c.pdf"]);
    }

    #[tokio::test]
    async fn validate_output_rejects_non_pdf_header() {
        let dir = std::env::temp_dir().join("nexara-test-pdf-merge-validate");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("not-a.pdf");
        std::fs::write(&path, b"not a pdf").unwrap();
        assert!(validate_output(&path, 1).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
