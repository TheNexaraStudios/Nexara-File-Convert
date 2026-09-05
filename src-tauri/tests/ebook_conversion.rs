//! Smoke tests that exercise real Calibre (`ebook-convert`) against a
//! generated fixture in `tests/fixtures/`. Skipped (not failed) when
//! `ebook-convert` isn't available, matching the project's rule that
//! optional-engine tests must not fail the whole suite.

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn ebook_convert_available() -> bool {
    Command::new("ebook-convert").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn run_case(fixture_name: &str, out_ext: &str, expect_header: Option<&[u8]>) {
    if !ebook_convert_available() {
        eprintln!("skipping {fixture_name} -> .{out_ext}: ebook-convert not found on PATH");
        return;
    }

    let input = fixture(fixture_name);
    assert!(input.is_file(), "fixture {fixture_name} is missing");

    let work_dir = std::env::temp_dir().join("nexara-ebook-smoke-test").join(format!("{fixture_name}-{out_ext}"));
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join(format!("out.{out_ext}"));

    let status = Command::new("ebook-convert").args([input.to_str().unwrap(), output.to_str().unwrap()]).status().expect("ebook-convert should run");
    assert!(status.success(), "ebook-convert exited with a failure converting {fixture_name} -> .{out_ext}");

    let metadata = std::fs::metadata(&output).unwrap_or_else(|_| panic!("expected output at {}", output.display()));
    assert!(metadata.len() > 0, "output file for {fixture_name} -> .{out_ext} is empty");

    if let Some(header) = expect_header {
        let bytes = std::fs::read(&output).unwrap();
        assert!(bytes.starts_with(header), "output for {fixture_name} -> .{out_ext} doesn't start with the expected magic bytes");
    }

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn epub_to_txt_produces_text() {
    run_case("sample.epub", "txt", None);
}

#[test]
fn epub_to_fb2_produces_output() {
    run_case("sample.epub", "fb2", None);
}

#[test]
fn epub_to_pdf_produces_valid_pdf() {
    run_case("sample.epub", "pdf", Some(b"%PDF"));
}
