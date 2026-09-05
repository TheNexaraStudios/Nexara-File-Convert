use std::path::Path;

use super::engine;
use super::jobs::JobRegistry;
use super::process::{self, ExecuteOutcome};

/// Builds `inkscape <input> -o <output>` — Inkscape, like Calibre, lets us
/// name the output file directly and infers the target format from its
/// extension, so no filename prediction/rename dance is needed afterwards.
pub fn build_args(input: &str, output_tmp: &str) -> Vec<String> {
    vec![input.to_string(), "-o".to_string(), output_tmp.to_string()]
}

/// Spawns Inkscape's CLI export and tracks the child in the shared job
/// registry so cancellation works the same way it does for other engines.
pub async fn execute(registry: &JobRegistry, job_id: &str, args: &[String]) -> Result<ExecuteOutcome, String> {
    let Some(binary) = engine::resolve_inkscape() else {
        return Err("Inkscape could not be found on this system.".to_string());
    };
    process::run_and_track(registry, job_id, &binary, args).await
}

/// A lightweight sanity check beyond "the file exists and is non-empty":
/// PNG and PDF both have reliable magic bytes to check. SVG/EPS/PS are
/// text-based with no single fixed signature Nexara can cheaply verify, so
/// those are accepted on size alone.
pub fn validate_output(path: &Path, output_format: &str) -> Result<(), String> {
    let expected: Option<&[u8]> = match output_format {
        "png" => Some(&[0x89, 0x50, 0x4E, 0x47]),
        "pdf" => Some(b"%PDF"),
        _ => None,
    };
    let Some(signature) = expected else {
        return Ok(());
    };

    let mut header = [0u8; 4];
    let read_len = {
        use std::io::Read;
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        file.read(&mut header).map_err(|e| e.to_string())?
    };

    if header[..read_len].starts_with(signature) {
        Ok(())
    } else {
        Err(format!("the output doesn't look like a valid {} file", output_format.to_uppercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_passes_input_then_dash_o_output() {
        let args = build_args(r"C:\in\logo.svg", r"C:\tmp\out.png");
        assert_eq!(args, vec![r"C:\in\logo.svg".to_string(), "-o".to_string(), r"C:\tmp\out.png".to_string()]);
    }

    #[test]
    fn validate_output_accepts_real_png_header() {
        let dir = std::env::temp_dir().join("nexara-test-vector-validate-png");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.png");
        std::fs::write(&path, [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A]).unwrap();
        assert!(validate_output(&path, "png").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_rejects_wrong_png_header() {
        let dir = std::env::temp_dir().join("nexara-test-vector-validate-png2");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("not-a.png");
        std::fs::write(&path, b"not a png").unwrap();
        assert!(validate_output(&path, "png").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_accepts_real_pdf_header() {
        let dir = std::env::temp_dir().join("nexara-test-vector-validate-pdf");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.pdf");
        std::fs::write(&path, b"%PDF-1.5\n...").unwrap();
        assert!(validate_output(&path, "pdf").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_skips_formats_without_signature() {
        let dir = std::env::temp_dir().join("nexara-test-vector-validate-svg");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.svg");
        std::fs::write(&path, b"<svg></svg>").unwrap();
        assert!(validate_output(&path, "svg").is_ok());
        assert!(validate_output(&path, "eps").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
