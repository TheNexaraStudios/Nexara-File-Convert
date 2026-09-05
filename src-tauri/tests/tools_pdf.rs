//! Smoke tests that exercise real MuPDF (`mutool`) — and, for JPG/WebP page
//! export, ImageMagick too — for the Merge PDF, Split PDF, and PDF-to-Images
//! tools. Skipped (not failed) when the relevant tool isn't available,
//! matching the project's rule that optional-engine tests must not fail the
//! whole suite.

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

fn work_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("nexara-tools-pdf-smoke-test").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn page_count(pdf: &std::path::Path) -> usize {
    let output = Command::new("mutool").args(["pages", pdf.to_str().unwrap()]).output().unwrap();
    String::from_utf8_lossy(&output.stdout).matches("pagenum").count()
}

#[test]
fn merge_combines_real_page_counts() {
    if !mutool_available() {
        eprintln!("skipping merge_combines_real_page_counts: mutool not found on PATH");
        return;
    }
    let one_page = fixture("sample.pdf");
    let three_page = fixture("sample-multipage.pdf");
    let dir = work_dir("merge");
    let output = dir.join("merged.pdf");

    let status = Command::new("mutool")
        .args(["merge", "-o", output.to_str().unwrap(), one_page.to_str().unwrap(), three_page.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success(), "mutool merge exited with a failure");

    let bytes = std::fs::read(&output).unwrap();
    assert!(bytes.starts_with(b"%PDF"), "merged output doesn't start with the PDF signature");
    assert_eq!(page_count(&output), 4, "expected 1 + 3 = 4 pages in the merged PDF");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn split_range_extracts_exactly_the_requested_pages() {
    if !mutool_available() {
        eprintln!("skipping split_range_extracts_exactly_the_requested_pages: mutool not found on PATH");
        return;
    }
    let input = fixture("sample-multipage.pdf");
    let dir = work_dir("split-range");
    let output = dir.join("range.pdf");

    let status =
        Command::new("mutool").args(["convert", "-o", output.to_str().unwrap(), "-F", "pdf", input.to_str().unwrap(), "1-2"]).status().unwrap();
    assert!(status.success(), "mutool convert exited with a failure");
    assert_eq!(page_count(&output), 2, "expected exactly the 2 requested pages");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn split_each_page_produces_one_valid_single_page_pdf_per_page() {
    if !mutool_available() {
        eprintln!("skipping split_each_page_produces_one_valid_single_page_pdf_per_page: mutool not found on PATH");
        return;
    }
    let input = fixture("sample-multipage.pdf");
    let dir = work_dir("split-each");

    for page in 1..=3 {
        let output = dir.join(format!("document-page-{page:03}.pdf"));
        let status = Command::new("mutool")
            .args(["convert", "-o", output.to_str().unwrap(), "-F", "pdf", input.to_str().unwrap(), &page.to_string()])
            .status()
            .unwrap();
        assert!(status.success(), "mutool convert exited with a failure on page {page}");
        assert_eq!(page_count(&output), 1, "page {page}'s extracted file should have exactly 1 page");
        let bytes = std::fs::read(&output).unwrap();
        assert!(bytes.starts_with(b"%PDF"), "page {page}'s output doesn't start with the PDF signature");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// Regression test: mutool substitutes the *sequential position* of a
/// selected page into `%d`, not its real page number — verified directly.
/// Selecting non-contiguous pages "1,3" out of the 3-page fixture must
/// produce two files at sequence positions 1 and 2, not "page 3" appearing
/// literally in mutool's own output filename.
#[test]
fn raster_export_of_noncontiguous_pages_uses_sequential_not_real_numbering() {
    if !mutool_available() {
        eprintln!("skipping raster_export_of_noncontiguous_pages_uses_sequential_not_real_numbering: mutool not found on PATH");
        return;
    }
    let input = fixture("sample-multipage.pdf");
    let dir = work_dir("raster-sequential");
    let pattern = dir.join("raster-%d.png");

    let status = Command::new("mutool")
        .args(["convert", "-o", pattern.to_str().unwrap(), "-O", "resolution=100", input.to_str().unwrap(), "1,3"])
        .status()
        .unwrap();
    assert!(status.success());

    assert!(dir.join("raster-1.png").is_file(), "expected sequential position 1 (real page 1)");
    assert!(dir.join("raster-2.png").is_file(), "expected sequential position 2 (real page 3)");
    assert!(!dir.join("raster-3.png").is_file(), "mutool should not have written a literal 'page 3' filename");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn raster_export_respects_requested_dpi() {
    if !mutool_available() {
        eprintln!("skipping raster_export_respects_requested_dpi: mutool not found on PATH");
        return;
    }
    if !magick_available() {
        eprintln!("skipping raster_export_respects_requested_dpi: magick not found on PATH (needed to measure pixel size)");
        return;
    }
    let input = fixture("sample-multipage.pdf");
    let dir = work_dir("raster-dpi");

    let low = dir.join("low-%d.png");
    Command::new("mutool").args(["convert", "-o", low.to_str().unwrap(), "-O", "resolution=72", input.to_str().unwrap(), "1"]).status().unwrap();
    let high = dir.join("high-%d.png");
    Command::new("mutool").args(["convert", "-o", high.to_str().unwrap(), "-O", "resolution=300", input.to_str().unwrap(), "1"]).status().unwrap();

    let width_of = |p: &std::path::Path| -> u32 {
        let out = Command::new("magick").args(["identify", "-format", "%w", p.to_str().unwrap()]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
    };
    let low_w = width_of(&dir.join("low-1.png"));
    let high_w = width_of(&dir.join("high-1.png"));
    assert!(high_w > low_w * 3, "300 DPI ({high_w}px) should be much wider than 72 DPI ({low_w}px)");

    let _ = std::fs::remove_dir_all(&dir);
}

/// JPG/WebP page export needs the PNG-then-ImageMagick hand-off, since
/// mutool's raster convert has no JPEG/WebP writer at all — verified
/// directly: `-o out.jpg` exits 0 but silently writes nothing.
#[test]
fn mutool_raster_convert_silently_produces_nothing_for_jpg() {
    if !mutool_available() {
        eprintln!("skipping mutool_raster_convert_silently_produces_nothing_for_jpg: mutool not found on PATH");
        return;
    }
    let input = fixture("sample-multipage.pdf");
    let dir = work_dir("jpg-silent-failure");
    let output = dir.join("out.jpg");

    let status = Command::new("mutool").args(["convert", "-o", output.to_str().unwrap(), input.to_str().unwrap(), "1"]).status().unwrap();
    assert!(status.success(), "mutool exits 0 even though it wrote nothing — this is the documented quirk");
    assert!(!output.is_file(), "mutool should NOT have produced a .jpg file directly");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn png_to_jpg_handoff_produces_a_real_jpeg() {
    if !mutool_available() || !magick_available() {
        eprintln!("skipping png_to_jpg_handoff_produces_a_real_jpeg: mutool or magick not found on PATH");
        return;
    }
    let input = fixture("sample-multipage.pdf");
    let dir = work_dir("jpg-handoff");
    let png_pattern = dir.join("raster-%d.png");

    let status =
        Command::new("mutool").args(["convert", "-o", png_pattern.to_str().unwrap(), input.to_str().unwrap(), "1"]).status().unwrap();
    assert!(status.success());
    let rendered_png = dir.join("raster-1.png");
    assert!(rendered_png.is_file());

    let final_jpg = dir.join("document-page-001.jpg");
    let status = Command::new("magick")
        .args([rendered_png.to_str().unwrap(), "-background", "white", "-flatten", final_jpg.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success(), "magick exited with a failure converting the rendered page to JPG");

    let bytes = std::fs::read(&final_jpg).unwrap();
    assert!(bytes.starts_with(&[0xFF, 0xD8, 0xFF]), "output doesn't start with the JPEG signature");

    let _ = std::fs::remove_dir_all(&dir);
}
