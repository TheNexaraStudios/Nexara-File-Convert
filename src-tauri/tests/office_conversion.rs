//! Smoke tests that exercise real LibreOffice against tiny generated
//! fixtures in `tests/fixtures/`. Skipped (not failed) when `soffice`
//! isn't available, matching the project's rule that optional-engine
//! tests must not fail the whole suite.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

/// `cargo test` runs tests in parallel threads by default. Even with a
/// distinct profile per test, running multiple headless `soffice`
/// instances at once was observed to hang indefinitely (not just flake) —
/// so these tests serialize through this lock rather than relying on
/// `--test-threads=1`, matching the same protection the app itself uses.
static SOFFICE_LOCK: Mutex<()> = Mutex::new(());

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn resolve_soffice() -> Option<String> {
    for candidate in [r"C:\Program Files\LibreOffice\program\soffice.com", r"C:\Program Files (x86)\LibreOffice\program\soffice.com"]
    {
        if std::path::Path::new(candidate).is_file() {
            return Some(candidate.to_string());
        }
    }
    Command::new("soffice").arg("--version").output().ok().filter(|o| o.status.success()).map(|_| "soffice".to_string())
}

/// Each run gets a fresh LibreOffice profile nested inside its own already
/// fresh work_dir (removed and recreated per call, deleted after). Reusing
/// a profile path across runs is unsafe: a force-killed instance skips
/// LibreOffice's own lock cleanup, and the next run against that same path
/// then hangs waiting on a lock nobody will ever release.
fn profile_arg(work_dir: &std::path::Path) -> String {
    let dir = work_dir.join("lo-profile");
    let path_str = dir.to_string_lossy().replace('\\', "/");
    format!("-env:UserInstallation=file:///{path_str}")
}

fn run_case(fixture_name: &str, out_ext: &str, expect_header: Option<&[u8]>) {
    let Some(soffice) = resolve_soffice() else {
        eprintln!("skipping {fixture_name} -> .{out_ext}: LibreOffice (soffice) not found");
        return;
    };

    let _guard = SOFFICE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    let input = fixture(fixture_name);
    assert!(input.is_file(), "fixture {fixture_name} is missing");

    let work_dir = std::env::temp_dir().join("nexara-office-smoke-test").join(format!("{fixture_name}-{out_ext}"));
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();

    let status = Command::new(&soffice)
        .args([
            "--headless",
            "--norestore",
            &profile_arg(&work_dir),
            "--convert-to",
            out_ext,
            "--outdir",
            work_dir.to_str().unwrap(),
            input.to_str().unwrap(),
        ])
        .status()
        .expect("soffice should run");
    assert!(status.success(), "soffice exited with a failure converting {fixture_name} -> .{out_ext}");

    let stem = input.file_stem().unwrap().to_str().unwrap();
    let output = work_dir.join(format!("{stem}.{out_ext}"));
    let metadata = std::fs::metadata(&output).unwrap_or_else(|_| panic!("expected output at {}", output.display()));
    assert!(metadata.len() > 0, "output file for {fixture_name} -> .{out_ext} is empty");

    if let Some(header) = expect_header {
        let bytes = std::fs::read(&output).unwrap();
        assert!(bytes.starts_with(header), "output for {fixture_name} -> .{out_ext} doesn't start with the expected magic bytes");
    }

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn docx_to_pdf_produces_valid_pdf() {
    run_case("sample.docx", "pdf", Some(b"%PDF"));
}

#[test]
fn docx_to_odt_produces_valid_odt() {
    run_case("sample.docx", "odt", Some(b"PK\x03\x04"));
}

#[test]
fn docx_to_txt_produces_text() {
    run_case("sample.docx", "txt", None);
}

#[test]
fn xlsx_to_csv_produces_text() {
    run_case("sample.xlsx", "csv", None);
}

#[test]
fn xlsx_to_pdf_produces_valid_pdf() {
    run_case("sample.xlsx", "pdf", Some(b"%PDF"));
}

fn resolve_pandoc() -> Option<String> {
    Command::new("pandoc").arg("--version").output().ok().filter(|o| o.status.success()).map(|_| "pandoc".to_string())
}

/// Mirrors `commands::convert_text_to_pdf`'s two-step hand-off: Pandoc alone
/// can't write PDF (needs a separate LaTeX install — verified directly), and
/// LibreOffice hangs headless on Markdown input specifically (also verified
/// directly — identical content saved as `.txt` converts instantly, `.md`
/// hangs indefinitely). So Markdown is normalized to HTML via Pandoc first,
/// then LibreOffice exports that HTML to PDF. Lives in this file (not
/// `text_conversion.rs`) specifically so it shares `SOFFICE_LOCK` with the
/// other tests here — `cargo test` runs separate test binaries concurrently
/// by default, and two headless `soffice` instances at once is exactly the
/// hang this lock exists to prevent.
#[test]
fn md_to_pdf_via_libreoffice_handoff_produces_valid_pdf() {
    let Some(soffice) = resolve_soffice() else {
        eprintln!("skipping sample.md -> .pdf: LibreOffice (soffice) not found");
        return;
    };
    let Some(pandoc) = resolve_pandoc() else {
        eprintln!("skipping sample.md -> .pdf: Pandoc not found");
        return;
    };

    let _guard = SOFFICE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    let input = fixture("sample.md");
    assert!(input.is_file(), "fixture sample.md is missing");

    let work_dir = std::env::temp_dir().join("nexara-office-smoke-test").join("md-to-pdf-handoff");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();

    let intermediate = work_dir.join("intermediate.html");
    let status = Command::new(&pandoc).args([input.to_str().unwrap(), "-o", intermediate.to_str().unwrap()]).status().unwrap();
    assert!(status.success(), "pandoc exited with a failure converting sample.md -> intermediate .html");

    let status = Command::new(&soffice)
        .args([
            "--headless",
            "--norestore",
            &profile_arg(&work_dir),
            "--convert-to",
            "pdf",
            "--outdir",
            work_dir.to_str().unwrap(),
            intermediate.to_str().unwrap(),
        ])
        .status()
        .expect("soffice should run");
    assert!(status.success(), "soffice exited with a failure converting the intermediate .html -> .pdf");

    let output = work_dir.join("intermediate.pdf");
    let bytes = std::fs::read(&output).unwrap_or_else(|_| panic!("expected output at {}", output.display()));
    assert!(bytes.starts_with(b"%PDF"), "output for sample.md -> .pdf doesn't start with the expected magic bytes");

    let _ = std::fs::remove_dir_all(&work_dir);
}
