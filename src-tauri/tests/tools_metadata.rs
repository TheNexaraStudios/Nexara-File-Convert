//! Smoke tests proving the real engine output the Metadata Inspector reads
//! (ffprobe JSON, ImageMagick's `identify -format`, `mutool info -M`, `7z l
//! -slt`) actually comes back in the shape `tools::metadata`'s parsers
//! expect. Those parsers themselves are unit-tested against fixture text
//! captured from these same tools; this file is the "the fixture text was
//! real" half of that proof. Skipped (not failed) when a tool isn't
//! available, matching the project's rule that optional-engine tests must
//! not fail the whole suite.

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

#[test]
fn ffprobe_json_has_the_fields_metadata_inspection_needs() {
    if Command::new("ffprobe").arg("-version").output().is_err() {
        eprintln!("skipping ffprobe_json_has_the_fields_metadata_inspection_needs: ffprobe not found on PATH");
        return;
    }
    let input = fixture("sample.mp4");
    let output = Command::new("ffprobe")
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["format"]["duration"].is_string(), "expected format.duration");
    let streams = json["streams"].as_array().expect("expected a streams array");
    assert!(streams.iter().any(|s| s["codec_type"] == "video"), "expected at least one video stream");
    assert!(streams[0]["codec_name"].is_string(), "expected the first stream to name its codec");
}

#[test]
fn magick_identify_format_string_has_the_fields_metadata_inspection_needs() {
    if Command::new("magick").arg("-version").output().map(|o| !o.status.success()).unwrap_or(true) {
        eprintln!("skipping magick_identify_format_string_has_the_fields_metadata_inspection_needs: magick not found on PATH");
        return;
    }
    let input = fixture("sample.png");
    let target = format!("{}[0]", input.to_str().unwrap());
    let output = Command::new("magick").args(["identify", "-format", "%w|%h|%m|%[colorspace]|%z|%A", &target]).output().unwrap();
    assert!(output.status.success());

    let text = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = text.trim().split('|').collect();
    assert_eq!(parts.len(), 6, "expected exactly 6 pipe-delimited fields, got: {text}");
    assert!(parts[0].parse::<u32>().is_ok(), "width should be numeric, got: {}", parts[0]);
    assert!(parts[1].parse::<u32>().is_ok(), "height should be numeric, got: {}", parts[1]);
    assert_eq!(parts[2], "PNG");
}

#[test]
fn mutool_info_has_the_pages_line_metadata_inspection_needs() {
    if Command::new("mutool").output().is_err() {
        eprintln!("skipping mutool_info_has_the_pages_line_metadata_inspection_needs: mutool not found on PATH");
        return;
    }
    let input = fixture("sample-multipage.pdf");
    let output = Command::new("mutool").args(["info", "-M", input.to_str().unwrap()]).output().unwrap();
    assert!(output.status.success());

    let text = String::from_utf8_lossy(&output.stdout);
    let pages_line = text.lines().find(|l| l.starts_with("Pages: "));
    assert_eq!(pages_line, Some("Pages: 3"));
    assert!(text.contains('[') && text.contains(']'), "expected a MediaBox entry, got: {text}");
}

#[test]
fn sevenzip_listing_has_the_fields_metadata_inspection_needs() {
    let sevenzip = if std::path::Path::new(r"C:\Program Files\7-Zip\7z.exe").is_file() {
        r"C:\Program Files\7-Zip\7z.exe".to_string()
    } else if Command::new("7z").output().is_ok() {
        "7z".to_string()
    } else {
        eprintln!("skipping sevenzip_listing_has_the_fields_metadata_inspection_needs: 7z not found on PATH");
        return;
    };
    let input = fixture("sample.zip");
    let output = Command::new(&sevenzip).args(["l", "-slt", input.to_str().unwrap(), "-sccUTF-8"]).output().unwrap();
    assert!(output.status.success());

    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("----------"), "expected the long separator before real entries");
    assert!(text.lines().any(|l| l.trim_start().starts_with("Path = ")), "expected at least one entry Path");
    assert!(text.lines().any(|l| l.trim_start().starts_with("Size = ")), "expected at least one entry Size");
}
