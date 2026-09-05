//! Smoke tests that exercise real ImageMagick against tiny generated
//! fixtures in `tests/fixtures/`. Skipped (not failed) when `magick` isn't
//! available on PATH, matching the project's rule that optional-engine
//! tests must not fail the whole suite.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn magick_available() -> bool {
    Command::new("magick").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn identify(path: &Path) -> String {
    let output = Command::new("magick")
        .args(["identify", "-format", "%m", path.to_str().unwrap()])
        .output()
        .expect("magick identify should run");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn run_case(fixture_name: &str, out_ext: &str, expected_format: &str) {
    if !magick_available() {
        eprintln!("skipping {fixture_name} -> .{out_ext}: ImageMagick not found on PATH");
        return;
    }

    let input = fixture(fixture_name);
    assert!(input.is_file(), "fixture {fixture_name} is missing");

    let work_dir = std::env::temp_dir().join("nexara-image-smoke-test").join(format!("{fixture_name}-{out_ext}"));
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join(format!("out.{out_ext}"));

    let status = Command::new("magick")
        .args([input.to_str().unwrap(), "-quality", "85", output.to_str().unwrap()])
        .status()
        .expect("magick should run");
    assert!(status.success(), "magick exited with a failure converting {fixture_name} -> .{out_ext}");

    let metadata = std::fs::metadata(&output).expect("output file should exist");
    assert!(metadata.len() > 0, "output file for {fixture_name} -> .{out_ext} is empty");

    let format = identify(&output);
    assert_eq!(format.to_uppercase(), expected_format, "expected {expected_format}, ImageMagick reports {format}");

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn png_to_jpg_produces_valid_jpeg() {
    run_case("sample.png", "jpg", "JPEG");
}

#[test]
fn jpg_to_png_produces_valid_png() {
    run_case("sample.jpg", "png", "PNG");
}

#[test]
fn png_to_webp_produces_valid_webp() {
    run_case("sample.png", "webp", "WEBP");
}

#[test]
fn webp_roundtrip_back_to_png() {
    if !magick_available() {
        eprintln!("skipping webp roundtrip: ImageMagick not found on PATH");
        return;
    }
    let work_dir = std::env::temp_dir().join("nexara-image-smoke-test").join("webp-roundtrip");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let webp = work_dir.join("mid.webp");

    let status1 =
        Command::new("magick").args([fixture("sample.png").to_str().unwrap(), webp.to_str().unwrap()]).status().unwrap();
    assert!(status1.success());

    let png_back = work_dir.join("back.png");
    let status2 = Command::new("magick").args([webp.to_str().unwrap(), png_back.to_str().unwrap()]).status().unwrap();
    assert!(status2.success());

    assert!(std::fs::metadata(&png_back).unwrap().len() > 0);
    assert_eq!(identify(&png_back).to_uppercase(), "PNG");

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn png_to_tiff_produces_valid_tiff() {
    run_case("sample.png", "tiff", "TIFF");
}

#[test]
fn png_to_bmp_produces_valid_bmp() {
    run_case("sample.png", "bmp", "BMP");
}

fn dimensions(path: &Path) -> (u32, u32) {
    let output = Command::new("magick").args(["identify", "-format", "%w %h", path.to_str().unwrap()]).output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.trim().split(' ');
    (parts.next().unwrap().parse().unwrap(), parts.next().unwrap().parse().unwrap())
}

/// Real proof of the Resize Image tool's "stretch" mode: `WxH!` geometry
/// ignores the source's aspect ratio and produces exactly the requested
/// box, even though sample.png (64x64) isn't square-compatible with a
/// non-square target.
#[test]
fn resize_stretch_produces_exact_dimensions_ignoring_aspect_ratio() {
    if !magick_available() {
        eprintln!("skipping resize_stretch_produces_exact_dimensions_ignoring_aspect_ratio: ImageMagick not found on PATH");
        return;
    }
    let work_dir = std::env::temp_dir().join("nexara-image-smoke-test").join("resize-stretch");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join("out.png");

    let status =
        Command::new("magick").args([fixture("sample.png").to_str().unwrap(), "-resize", "300x100!", output.to_str().unwrap()]).status().unwrap();
    assert!(status.success());
    assert_eq!(dimensions(&output), (300, 100));

    let _ = std::fs::remove_dir_all(&work_dir);
}

/// Real proof of "fit" mode: plain `WxH` geometry fits *within* the box
/// while preserving aspect ratio, so a square 64x64 source asked to fit in
/// a 300x100 box comes out at 100x100 (bounded by the shorter dimension),
/// not stretched to fill the whole box.
#[test]
fn resize_fit_preserves_aspect_ratio_within_the_box() {
    if !magick_available() {
        eprintln!("skipping resize_fit_preserves_aspect_ratio_within_the_box: ImageMagick not found on PATH");
        return;
    }
    let work_dir = std::env::temp_dir().join("nexara-image-smoke-test").join("resize-fit");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join("out.png");

    let status =
        Command::new("magick").args([fixture("sample.png").to_str().unwrap(), "-resize", "300x100", output.to_str().unwrap()]).status().unwrap();
    assert!(status.success());
    let (w, h) = dimensions(&output);
    assert_eq!(h, 100, "the constraining dimension (height) should be exactly 100");
    assert!(w <= 300, "width should never exceed the box");
    assert_eq!(w, h, "a square source fit into any box should stay square");

    let _ = std::fs::remove_dir_all(&work_dir);
}

/// Real proof of "fill" mode: `WxH^` plus `-gravity center -extent WxH`
/// crops to exactly fill the box after an aspect-preserving resize —
/// unlike "fit", the output dimensions always exactly match the box.
#[test]
fn resize_fill_crops_to_exactly_match_the_box() {
    if !magick_available() {
        eprintln!("skipping resize_fill_crops_to_exactly_match_the_box: ImageMagick not found on PATH");
        return;
    }
    let work_dir = std::env::temp_dir().join("nexara-image-smoke-test").join("resize-fill");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join("out.png");

    let status = Command::new("magick")
        .args([
            fixture("sample.png").to_str().unwrap(),
            "-resize",
            "300x100^",
            "-gravity",
            "center",
            "-extent",
            "300x100",
            output.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(dimensions(&output), (300, 100));

    let _ = std::fs::remove_dir_all(&work_dir);
}

/// Real proof of percentage resizing: 50% of the 64x64 fixture is exactly
/// 32x32.
#[test]
fn resize_percent_scales_proportionally() {
    if !magick_available() {
        eprintln!("skipping resize_percent_scales_proportionally: ImageMagick not found on PATH");
        return;
    }
    let work_dir = std::env::temp_dir().join("nexara-image-smoke-test").join("resize-percent");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join("out.png");

    let (orig_w, orig_h) = dimensions(&fixture("sample.png"));
    let status =
        Command::new("magick").args([fixture("sample.png").to_str().unwrap(), "-resize", "50%", output.to_str().unwrap()]).status().unwrap();
    assert!(status.success());
    assert_eq!(dimensions(&output), (orig_w / 2, orig_h / 2));

    let _ = std::fs::remove_dir_all(&work_dir);
}
