//! Smoke tests that exercise real FontForge against a generated fixture in
//! `tests/fixtures/`. `sample.ttf` is a minimal font authored specifically
//! for this test suite (a single glyph, generated via FontForge's own
//! scripting) rather than a copy of any real typeface, so it carries no
//! licensing baggage. Skipped (not failed) when FontForge isn't available,
//! matching the project's rule that optional-engine tests must not fail the
//! whole suite.

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn resolve_fontforge() -> Option<String> {
    for candidate in [
        r"C:\Program Files\FontForgeBuilds\bin\fontforge.exe",
        r"C:\Program Files\FontForge\bin\fontforge.exe",
        r"C:\Program Files (x86)\FontForgeBuilds\bin\fontforge.exe",
        r"C:\Program Files (x86)\FontForge\bin\fontforge.exe",
    ] {
        if std::path::Path::new(candidate).is_file() {
            return Some(candidate.to_string());
        }
    }
    Command::new("fontforge").arg("-version").output().ok().filter(|o| o.status.success()).map(|_| "fontforge".to_string())
}

fn run_case(out_ext: &str, expect_headers: &[&[u8]]) {
    let Some(fontforge) = resolve_fontforge() else {
        eprintln!("skipping sample.ttf -> .{out_ext}: FontForge not found");
        return;
    };

    let input = fixture("sample.ttf");
    assert!(input.is_file(), "fixture sample.ttf is missing");

    let work_dir = std::env::temp_dir().join("nexara-font-smoke-test").join(out_ext);
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join(format!("out.{out_ext}"));

    let status = Command::new(&fontforge)
        .args(["-lang=ff", "-c", "Open($1); Generate($2)", input.to_str().unwrap(), output.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success(), "fontforge exited with a failure converting sample.ttf -> .{out_ext}");

    let bytes = std::fs::read(&output).unwrap_or_else(|_| panic!("expected output at {}", output.display()));
    assert!(!bytes.is_empty(), "output for sample.ttf -> .{out_ext} is empty");
    assert!(
        expect_headers.iter().any(|h| bytes.starts_with(h)),
        "output for sample.ttf -> .{out_ext} doesn't start with any expected magic bytes"
    );

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn ttf_to_otf_produces_valid_font() {
    run_case("otf", &[b"OTTO", &[0x00, 0x01, 0x00, 0x00]]);
}

#[test]
fn ttf_to_woff_produces_valid_font() {
    run_case("woff", &[b"wOFF"]);
}

#[test]
fn ttf_to_woff2_produces_valid_font() {
    run_case("woff2", &[b"wOF2"]);
}
