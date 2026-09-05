use std::path::Path;

use super::engine;
use super::jobs::JobRegistry;
use super::process::{self, ExecuteOutcome};

/// FontForge's non-interactive scripting mode: `-lang=ff -c '<script>' <args...>`
/// runs the given script with `$1`, `$2`, ... bound to the trailing
/// arguments. `Open($1)` loads the source font, `Generate($2)` writes it
/// back out in whatever format the output path's extension implies — the
/// same infer-from-extension convention Inkscape and Calibre use, so no
/// filename prediction/rename dance is needed afterwards.
pub fn build_args(input: &str, output_tmp: &str) -> Vec<String> {
    vec!["-lang=ff".to_string(), "-c".to_string(), "Open($1); Generate($2)".to_string(), input.to_string(), output_tmp.to_string()]
}

/// Spawns FontForge's scripting CLI and tracks the child in the shared job
/// registry so cancellation works the same way it does for other engines.
pub async fn execute(registry: &JobRegistry, job_id: &str, args: &[String]) -> Result<ExecuteOutcome, String> {
    let Some(binary) = engine::resolve_fontforge() else {
        return Err("FontForge could not be found on this system.".to_string());
    };
    process::run_and_track(registry, job_id, &binary, args).await
}

/// TTF/OTF/WOFF/WOFF2 all have reliable magic bytes, so every font output
/// this engine produces gets a real signature check rather than just
/// "the file exists and is non-empty". OTF covers both flavors FontForge
/// might emit for a `.otf` target: `OTTO` for CFF-flavored outlines, or the
/// same `\0\1\0\0` sfnt version TrueType itself uses for quadratic-outline
/// OpenType.
pub fn validate_output(path: &Path, output_format: &str) -> Result<(), String> {
    let signatures: &[&[u8]] = match output_format {
        "ttf" => &[&[0x00, 0x01, 0x00, 0x00], b"true"],
        "otf" => &[b"OTTO", &[0x00, 0x01, 0x00, 0x00]],
        "woff" => &[b"wOFF"],
        "woff2" => &[b"wOF2"],
        _ => &[],
    };
    if signatures.is_empty() {
        return Ok(());
    }

    let mut header = [0u8; 4];
    let read_len = {
        use std::io::Read;
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        file.read(&mut header).map_err(|e| e.to_string())?
    };

    if signatures.iter().any(|sig| header[..read_len].starts_with(sig)) {
        Ok(())
    } else {
        Err(format!("the output doesn't look like a valid {} file", output_format.to_uppercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_runs_open_then_generate_script_with_input_and_output() {
        let args = build_args(r"C:\in\sample.ttf", r"C:\tmp\out.otf");
        assert_eq!(
            args,
            vec![
                "-lang=ff".to_string(),
                "-c".to_string(),
                "Open($1); Generate($2)".to_string(),
                r"C:\in\sample.ttf".to_string(),
                r"C:\tmp\out.otf".to_string(),
            ]
        );
    }

    #[test]
    fn validate_output_accepts_real_ttf_header() {
        let dir = std::env::temp_dir().join("nexara-test-font-validate-ttf");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.ttf");
        std::fs::write(&path, [0x00, 0x01, 0x00, 0x00, 0x00, 0x0E]).unwrap();
        assert!(validate_output(&path, "ttf").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_accepts_real_otf_header() {
        let dir = std::env::temp_dir().join("nexara-test-font-validate-otf");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.otf");
        std::fs::write(&path, b"OTTO\x00\x0B").unwrap();
        assert!(validate_output(&path, "otf").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_accepts_real_woff_header() {
        let dir = std::env::temp_dir().join("nexara-test-font-validate-woff");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.woff");
        std::fs::write(&path, b"wOFF\x00\x01").unwrap();
        assert!(validate_output(&path, "woff").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_accepts_real_woff2_header() {
        let dir = std::env::temp_dir().join("nexara-test-font-validate-woff2");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.woff2");
        std::fs::write(&path, b"wOF2\x00\x01").unwrap();
        assert!(validate_output(&path, "woff2").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_rejects_wrong_header() {
        let dir = std::env::temp_dir().join("nexara-test-font-validate-bad");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("not-a.ttf");
        std::fs::write(&path, b"not a font").unwrap();
        assert!(validate_output(&path, "ttf").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
