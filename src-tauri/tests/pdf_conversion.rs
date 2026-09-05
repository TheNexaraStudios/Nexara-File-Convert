//! Smoke tests that exercise real MuPDF (`mutool`) and ImageMagick against
//! generated fixtures in `tests/fixtures/`. Skipped (not failed) when the
//! relevant tool isn't available, matching the project's rule that
//! optional-engine tests must not fail the whole suite.

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn mutool_available() -> bool {
    Command::new("mutool").output().is_ok()
}

fn magick_available() -> bool {
    Command::new("magick").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Real proof that PDF -> PNG rasterization (via MuPDF, no Ghostscript
/// needed) actually produces a valid, non-empty image for page 1.
#[test]
fn pdf_first_page_to_png_produces_valid_image() {
    if !mutool_available() {
        eprintln!("skipping pdf_first_page_to_png_produces_valid_image: mutool not found on PATH");
        return;
    }

    let input = fixture("sample.pdf");
    assert!(input.is_file(), "fixture sample.pdf is missing");

    let work_dir = std::env::temp_dir().join("nexara-pdf-smoke-test").join("pdf-to-png");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    // mutool always substitutes the page number into the output filename
    // (verified directly against the real binary) — "page-%d.png" with
    // page 1 becomes "page-1.png", exactly what src/conversion/pdf.rs
    // predicts and renames.
    let pattern = work_dir.join("page-%d.png");
    let output = work_dir.join("page-1.png");

    let status = Command::new("mutool").args(["convert", "-o", pattern.to_str().unwrap(), input.to_str().unwrap(), "1"]).status().unwrap();
    assert!(status.success(), "mutool convert exited with a failure");

    let bytes = std::fs::read(&output).unwrap();
    assert!(!bytes.is_empty(), "output PNG is empty");
    assert!(bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]), "output doesn't start with the PNG signature");

    let _ = std::fs::remove_dir_all(&work_dir);
}

/// Real proof that ImageMagick can write PDF directly from a raster image
/// with no Ghostscript delegate installed (the reverse direction — PDF as
/// input — genuinely can't work without it, which is why the registry only
/// offers image -> pdf, not pdf -> anything-but-png).
#[test]
fn image_to_pdf_produces_valid_pdf_without_ghostscript() {
    if !magick_available() {
        eprintln!("skipping image_to_pdf_produces_valid_pdf_without_ghostscript: magick not found on PATH");
        return;
    }

    let input = fixture("sample.png");
    assert!(input.is_file(), "fixture sample.png is missing");

    let work_dir = std::env::temp_dir().join("nexara-pdf-smoke-test").join("image-to-pdf");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join("out.pdf");

    let status = Command::new("magick").args([input.to_str().unwrap(), output.to_str().unwrap()]).status().unwrap();
    assert!(status.success(), "magick exited with a failure writing PDF");

    let bytes = std::fs::read(&output).unwrap();
    assert!(!bytes.is_empty(), "output PDF is empty");
    assert!(bytes.starts_with(b"%PDF"), "output doesn't start with the PDF signature");

    let _ = std::fs::remove_dir_all(&work_dir);
}
