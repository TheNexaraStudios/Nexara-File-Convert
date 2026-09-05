//! Smoke tests that exercise real Inkscape against a generated fixture in
//! `tests/fixtures/`. Skipped (not failed) when Inkscape isn't available,
//! matching the project's rule that optional-engine tests must not fail
//! the whole suite.

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn resolve_inkscape() -> Option<String> {
    for candidate in [r"C:\Program Files\Inkscape\bin\inkscape.exe", r"C:\Program Files (x86)\Inkscape\bin\inkscape.exe"] {
        if std::path::Path::new(candidate).is_file() {
            return Some(candidate.to_string());
        }
    }
    Command::new("inkscape").arg("--version").output().ok().filter(|o| o.status.success()).map(|_| "inkscape".to_string())
}

fn run_case(out_ext: &str, expect_header: Option<&[u8]>) {
    let Some(inkscape) = resolve_inkscape() else {
        eprintln!("skipping sample.svg -> .{out_ext}: Inkscape not found");
        return;
    };

    let input = fixture("sample.svg");
    assert!(input.is_file(), "fixture sample.svg is missing");

    let work_dir = std::env::temp_dir().join("nexara-vector-smoke-test").join(out_ext);
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join(format!("out.{out_ext}"));

    let status = Command::new(&inkscape).args([input.to_str().unwrap(), "-o", output.to_str().unwrap()]).status().unwrap();
    assert!(status.success(), "inkscape exited with a failure converting sample.svg -> .{out_ext}");

    let bytes = std::fs::read(&output).unwrap_or_else(|_| panic!("expected output at {}", output.display()));
    assert!(!bytes.is_empty(), "output for sample.svg -> .{out_ext} is empty");

    if let Some(header) = expect_header {
        assert!(bytes.starts_with(header), "output for sample.svg -> .{out_ext} doesn't start with the expected magic bytes");
    }

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn svg_to_png_produces_valid_image() {
    run_case("png", Some(&[0x89, 0x50, 0x4E, 0x47]));
}

#[test]
fn svg_to_pdf_produces_valid_pdf() {
    run_case("pdf", Some(b"%PDF"));
}

#[test]
fn svg_to_eps_produces_output() {
    run_case("eps", None);
}
