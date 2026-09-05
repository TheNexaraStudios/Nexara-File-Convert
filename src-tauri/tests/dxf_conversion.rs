//! Smoke tests that exercise real Inkscape against a generated DXF fixture
//! in `tests/fixtures/`. DXF rides on the same "vector" engine as SVG/EPS/PS
//! (see conversion-engines.md) rather than a dedicated CAD engine — Inkscape
//! imports DXF natively. Skipped (not failed) when Inkscape isn't available,
//! matching the project's rule that optional-engine tests must not fail the
//! whole suite.

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

fn run_case(out_ext: &str, expect_header: &[u8]) {
    let Some(inkscape) = resolve_inkscape() else {
        eprintln!("skipping sample.dxf -> .{out_ext}: Inkscape not found");
        return;
    };

    let input = fixture("sample.dxf");
    assert!(input.is_file(), "fixture sample.dxf is missing");

    let work_dir = std::env::temp_dir().join("nexara-dxf-smoke-test").join(out_ext);
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join(format!("out.{out_ext}"));

    let status = Command::new(&inkscape).args([input.to_str().unwrap(), "-o", output.to_str().unwrap()]).status().unwrap();
    assert!(status.success(), "inkscape exited with a failure converting sample.dxf -> .{out_ext}");

    let bytes = std::fs::read(&output).unwrap_or_else(|_| panic!("expected output at {}", output.display()));
    assert!(!bytes.is_empty(), "output for sample.dxf -> .{out_ext} is empty");
    assert!(bytes.starts_with(expect_header), "output for sample.dxf -> .{out_ext} doesn't start with the expected magic bytes");

    // The DXF fixture has a real line and a real circle in it — verify the
    // renderer actually drew geometry, not just an empty canvas.
    if out_ext == "svg" {
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("<path"), "expected the DXF's line/circle entities to render as SVG paths");
    }

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn dxf_to_svg_renders_real_geometry() {
    run_case("svg", b"<?xml");
}

#[test]
fn dxf_to_pdf_produces_valid_pdf() {
    run_case("pdf", b"%PDF");
}
