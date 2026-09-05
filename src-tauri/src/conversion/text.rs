use std::path::Path;

use super::engine;
use super::jobs::JobRegistry;
use super::process::{self, ExecuteOutcome};

/// Maps a Nexara output format id to the Pandoc writer name to pass via
/// `-t`. Only needed where Pandoc's own extension-based inference gets it
/// wrong: a `.txt` target with no explicit `-t` silently falls back to
/// Pandoc's Markdown writer — verified directly, headings/bold/links kept
/// their literal `#`/`**`/`[...]` syntax instead of being flattened to real
/// plain text. `.md` is spelled out too, for clarity, even though it
/// already matches Pandoc's own inference. Every other format here (HTML,
/// DOCX, EPUB) infers correctly from the output path's extension.
fn pandoc_writer(output_format: &str) -> Option<&'static str> {
    match output_format {
        "txt" => Some("plain"),
        "md" => Some("markdown"),
        _ => None,
    }
}

/// Builds a `pandoc <input> [-t <writer>] -o <output>` argument list.
pub fn build_args(input: &str, output_tmp: &str, output_format: &str) -> Vec<String> {
    let mut args = vec![input.to_string()];
    if let Some(writer) = pandoc_writer(output_format) {
        args.push("-t".to_string());
        args.push(writer.to_string());
    }
    args.push("-o".to_string());
    args.push(output_tmp.to_string());
    args
}

/// Spawns Pandoc and tracks the child in the shared job registry so
/// cancellation works the same way it does for other engines. Resolves
/// through Nexara's bundled copy first (see `provisioning`), falling back
/// to a system install on PATH if that's somehow missing.
pub async fn execute(registry: &JobRegistry, job_id: &str, args: &[String]) -> Result<ExecuteOutcome, String> {
    process::run_and_track(registry, job_id, &engine::binary_path("pandoc"), args).await
}

/// DOCX/EPUB are both ZIP-based and get a real magic-byte check. PDF output
/// (produced via the LibreOffice hand-off in `commands::convert_text_to_pdf`
/// — Pandoc alone can't write PDF without a LaTeX engine, verified directly)
/// gets the same `%PDF` check every other engine's PDF output does. HTML,
/// Markdown, and plain text have no reliable fixed signature, so those are
/// accepted on size alone.
pub fn validate_output(path: &Path, output_format: &str) -> Result<(), String> {
    let signature: Option<&[u8]> = match output_format {
        "docx" | "epub" => Some(b"PK\x03\x04"),
        "pdf" => Some(b"%PDF"),
        _ => None,
    };
    let Some(signature) = signature else {
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
    fn build_args_passes_input_then_output_with_no_writer_override_for_html() {
        let args = build_args(r"C:\in\sample.md", r"C:\tmp\out.html", "html");
        assert_eq!(args, vec![r"C:\in\sample.md".to_string(), "-o".to_string(), r"C:\tmp\out.html".to_string()]);
    }

    #[test]
    fn build_args_forces_plain_writer_for_txt_target() {
        // Regression test: without this, Pandoc silently writes Markdown
        // syntax into a ".txt" file instead of flattened plain text.
        let args = build_args(r"C:\in\sample.html", r"C:\tmp\out.txt", "txt");
        let idx = args.iter().position(|a| a == "-t").expect("expected an explicit -t flag for txt output");
        assert_eq!(args[idx + 1], "plain");
    }

    #[test]
    fn build_args_forces_markdown_writer_for_md_target() {
        let args = build_args(r"C:\in\sample.txt", r"C:\tmp\out.md", "md");
        let idx = args.iter().position(|a| a == "-t").expect("expected an explicit -t flag for md output");
        assert_eq!(args[idx + 1], "markdown");
    }

    #[test]
    fn build_args_relies_on_extension_inference_for_docx_and_epub() {
        for (ext, fmt) in [("docx", "docx"), ("epub", "epub")] {
            let output = format!(r"C:\tmp\out.{ext}");
            let args = build_args(r"C:\in\sample.md", &output, fmt);
            assert!(!args.contains(&"-t".to_string()), "'{fmt}' should not need an explicit writer flag");
        }
    }

    #[test]
    fn validate_output_accepts_real_docx_header() {
        let dir = std::env::temp_dir().join("nexara-test-text-validate-docx");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.docx");
        std::fs::write(&path, [0x50, 0x4B, 0x03, 0x04, 0, 0, 0]).unwrap();
        assert!(validate_output(&path, "docx").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_accepts_real_pdf_header() {
        let dir = std::env::temp_dir().join("nexara-test-text-validate-pdf");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.pdf");
        std::fs::write(&path, b"%PDF-1.7\n...").unwrap();
        assert!(validate_output(&path, "pdf").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_rejects_wrong_header() {
        let dir = std::env::temp_dir().join("nexara-test-text-validate-bad");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("not-a.docx");
        std::fs::write(&path, b"not a docx").unwrap();
        assert!(validate_output(&path, "docx").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_skips_formats_without_signature() {
        let dir = std::env::temp_dir().join("nexara-test-text-validate-skip");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.txt");
        std::fs::write(&path, b"just plain text").unwrap();
        assert!(validate_output(&path, "txt").is_ok());
        assert!(validate_output(&path, "html").is_ok());
        assert!(validate_output(&path, "md").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
