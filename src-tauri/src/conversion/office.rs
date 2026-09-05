use std::sync::OnceLock;
use tokio::sync::Mutex as TokioMutex;

use super::engine;
use super::jobs::JobRegistry;
use super::process::{self, ExecuteOutcome};

/// Even with a dedicated profile per call, running more than one headless
/// `soffice` at a time is unreliable on Windows in practice — instances can
/// hang indefinitely waiting on each other for reasons that go deeper than
/// the profile lock file (observed directly while testing this engine).
/// Rather than rely on the rest of the app never scheduling two office
/// conversions concurrently, enforce it here: every office conversion
/// queues behind this lock, so at most one `soffice` process ever runs.
static OFFICE_LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();

fn office_lock() -> &'static TokioMutex<()> {
    OFFICE_LOCK.get_or_init(|| TokioMutex::new(()))
}

/// LibreOffice only allows one running instance per user profile — if the
/// user has LibreOffice open themselves, or a previous headless run didn't
/// fully exit *cleanly*, a second invocation against a reused profile hangs
/// waiting for it. Force-killing a hung/cancelled run (which we do — see
/// `jobs::cancel`) skips LibreOffice's own lock cleanup, so a *persistent*
/// shared profile can get permanently poisoned by exactly the failure mode
/// this exists to guard against. The fix is to never reuse one: each
/// conversion gets a throwaway profile nested inside its own per-job temp
/// directory, which the caller already deletes once the job finishes.
fn profile_uri(temp_dir: &str) -> String {
    let dir = std::path::Path::new(temp_dir).join("lo-profile");
    let path_str = dir.to_string_lossy().replace('\\', "/");
    format!("file:///{path_str}")
}

/// Builds the `soffice --headless --convert-to ...` argument list.
/// LibreOffice picks the output filename itself (`<input-stem>.<ext>` inside
/// `--outdir`), so the caller is responsible for locating and normalizing
/// that file afterwards.
pub fn build_args(input: &str, temp_dir: &str, output_format: &str) -> Vec<String> {
    vec![
        "--headless".into(),
        "--norestore".into(),
        format!("-env:UserInstallation={}", profile_uri(temp_dir)),
        "--convert-to".into(),
        output_format.into(),
        "--outdir".into(),
        temp_dir.into(),
        input.into(),
    ]
}

/// Spawns LibreOffice headless and tracks the child in the shared job
/// registry so cancellation works the same way it does for other engines.
/// A single document conversion is normally too fast (and LibreOffice
/// reports no progress at all) to bother with anything but completion.
pub async fn execute(registry: &JobRegistry, job_id: &str, args: &[String]) -> Result<ExecuteOutcome, String> {
    let Some(binary) = engine::resolve_soffice() else {
        return Err("LibreOffice (soffice) could not be found on this system.".to_string());
    };
    let _guard = office_lock().lock().await;
    process::run_and_track(registry, job_id, &binary, args).await
}

/// A lightweight sanity check beyond "the file exists and is non-empty":
/// confirms the produced file actually starts with the magic bytes its
/// claimed format requires. Formats with no reliable signature (plain text,
/// CSV, RTF, HTML) are accepted on size alone.
pub fn validate_output(path: &std::path::Path, output_format: &str) -> Result<(), String> {
    let mut header = [0u8; 8];
    let read_len = {
        use std::io::Read;
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        file.read(&mut header).map_err(|e| e.to_string())?
    };
    let header = &header[..read_len];

    let ok = match output_format {
        "pdf" => header.starts_with(b"%PDF"),
        "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" | "epub" => {
            header.starts_with(b"PK\x03\x04") || header.starts_with(b"PK\x05\x06")
        }
        "doc" | "xls" | "ppt" => header.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]),
        _ => return Ok(()), // txt, csv, html, rtf, md: no reliable magic bytes to check
    };

    if ok {
        Ok(())
    } else {
        Err(format!("the output doesn't look like a valid {} file", output_format.to_uppercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_includes_convert_to_and_outdir() {
        let args = build_args(r"C:\in\doc.docx", r"C:\tmp", "pdf");
        assert!(args.iter().any(|a| a == "--convert-to"));
        assert!(args.iter().any(|a| a == "pdf"));
        assert!(args.iter().any(|a| a == "--outdir"));
        assert!(args.contains(&r"C:\tmp".to_string()));
        assert_eq!(args.last().unwrap(), r"C:\in\doc.docx");
    }

    #[test]
    fn validate_output_accepts_real_pdf_header() {
        let dir = std::env::temp_dir().join("nexara-test-office-validate");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.pdf");
        std::fs::write(&path, b"%PDF-1.7\n...").unwrap();
        assert!(validate_output(&path, "pdf").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_rejects_wrong_header() {
        let dir = std::env::temp_dir().join("nexara-test-office-validate2");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("not-a.pdf");
        std::fs::write(&path, b"this is not a pdf").unwrap();
        assert!(validate_output(&path, "pdf").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_accepts_zip_based_docx() {
        let dir = std::env::temp_dir().join("nexara-test-office-validate3");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.docx");
        std::fs::write(&path, [0x50, 0x4B, 0x03, 0x04, 0, 0, 0]).unwrap();
        assert!(validate_output(&path, "docx").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_skips_formats_without_signature() {
        let dir = std::env::temp_dir().join("nexara-test-office-validate4");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.txt");
        std::fs::write(&path, b"just some plain text").unwrap();
        assert!(validate_output(&path, "txt").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
