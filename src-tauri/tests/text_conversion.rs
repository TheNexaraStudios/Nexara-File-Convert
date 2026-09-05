//! Smoke tests that exercise real Pandoc against fixtures in
//! `tests/fixtures/`. Skipped (not failed) when Pandoc isn't available,
//! matching the project's rule that optional-engine tests must not fail the
//! whole suite. The one Pandoc pair that needs LibreOffice too — Markdown to
//! PDF, since Pandoc alone can't write PDF without a separate LaTeX install —
//! is tested in `office_conversion.rs` instead, sharing that file's
//! cross-test `soffice` lock (LibreOffice hangs if more than one headless
//! instance runs at once, verified directly, and `cargo test` runs separate
//! test binaries concurrently by default).

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn resolve_pandoc() -> Option<String> {
    Command::new("pandoc").arg("--version").output().ok().filter(|o| o.status.success()).map(|_| "pandoc".to_string())
}

fn run_case(fixture_name: &str, out_ext: &str, writer: Option<&str>, expect_header: Option<&[u8]>) {
    let Some(pandoc) = resolve_pandoc() else {
        eprintln!("skipping {fixture_name} -> .{out_ext}: Pandoc not found");
        return;
    };

    let input = fixture(fixture_name);
    assert!(input.is_file(), "fixture {fixture_name} is missing");

    let work_dir = std::env::temp_dir().join("nexara-text-smoke-test").join(format!("{fixture_name}-{out_ext}"));
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join(format!("out.{out_ext}"));

    let mut args = vec![input.to_str().unwrap().to_string()];
    if let Some(w) = writer {
        args.push("-t".to_string());
        args.push(w.to_string());
    }
    args.push("-o".to_string());
    args.push(output.to_str().unwrap().to_string());

    let status = Command::new(&pandoc).args(&args).status().unwrap();
    assert!(status.success(), "pandoc exited with a failure converting {fixture_name} -> .{out_ext}");

    let bytes = std::fs::read(&output).unwrap_or_else(|_| panic!("expected output at {}", output.display()));
    assert!(!bytes.is_empty(), "output for {fixture_name} -> .{out_ext} is empty");

    if let Some(header) = expect_header {
        assert!(bytes.starts_with(header), "output for {fixture_name} -> .{out_ext} doesn't start with the expected magic bytes");
    }

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn md_to_html_renders_real_markup() {
    let Some(pandoc) = resolve_pandoc() else {
        eprintln!("skipping sample.md -> .html: Pandoc not found");
        return;
    };

    let input = fixture("sample.md");
    let work_dir = std::env::temp_dir().join("nexara-text-smoke-test").join("md-to-html-content-check");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join("out.html");

    let status = Command::new(&pandoc).args([input.to_str().unwrap(), "-o", output.to_str().unwrap()]).status().unwrap();
    assert!(status.success(), "pandoc exited with a failure converting sample.md -> .html");

    let text = std::fs::read_to_string(&output).unwrap();
    assert!(text.contains("<h1"), "expected the Markdown heading to render as a real <h1>, got: {text}");
    assert!(text.contains("<strong>"), "expected **test** to render as real <strong>, got: {text}");

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn md_to_docx_produces_valid_docx() {
    run_case("sample.md", "docx", None, Some(b"PK\x03\x04"));
}

#[test]
fn md_to_epub_produces_valid_epub() {
    run_case("sample.md", "epub", None, Some(b"PK\x03\x04"));
}

#[test]
fn md_to_txt_flattens_to_real_plain_text_not_markdown() {
    // Regression test: without an explicit `-t plain`, Pandoc silently
    // writes Markdown syntax into a ".txt" file instead of flattened plain
    // text — verified directly. Confirm the literal `**`/`#`/`[...]`
    // syntax marks are gone from the output.
    let Some(_pandoc) = resolve_pandoc() else {
        eprintln!("skipping sample.md -> .txt: Pandoc not found");
        return;
    };

    let input = fixture("sample.md");
    let work_dir = std::env::temp_dir().join("nexara-text-smoke-test").join("md-to-plain-txt");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join("out.txt");

    let status =
        Command::new("pandoc").args([input.to_str().unwrap(), "-t", "plain", "-o", output.to_str().unwrap()]).status().unwrap();
    assert!(status.success(), "pandoc exited with a failure converting sample.md -> .txt");

    let text = std::fs::read_to_string(&output).unwrap();
    assert!(!text.contains("**"), "expected bold markers to be flattened away, got: {text}");
    assert!(!text.trim_start().starts_with('#'), "expected the heading marker to be flattened away, got: {text}");

    let _ = std::fs::remove_dir_all(&work_dir);
}
