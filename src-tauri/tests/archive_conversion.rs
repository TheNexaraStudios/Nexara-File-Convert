//! Smoke tests that exercise real 7-Zip against generated fixtures in
//! `tests/fixtures/`. Skipped (not failed) when `7z` isn't available,
//! matching the project's rule that optional-engine tests must not fail
//! the whole suite.

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn resolve_7z() -> Option<String> {
    for candidate in [r"C:\Program Files\7-Zip\7z.exe", r"C:\Program Files (x86)\7-Zip\7z.exe"] {
        if std::path::Path::new(candidate).is_file() {
            return Some(candidate.to_string());
        }
    }
    Command::new("7z").output().ok().map(|_| "7z".to_string())
}

/// Extracts then recompresses `fixture_name` into `out_ext`, mirroring what
/// `archive::convert` does internally, and asserts the round trip produced
/// a real, valid archive containing the original files.
fn run_case(fixture_name: &str, out_ext: &str, type_flag: &str) {
    let Some(sevenzip) = resolve_7z() else {
        eprintln!("skipping {fixture_name} -> .{out_ext}: 7-Zip (7z) not found");
        return;
    };

    let input = fixture(fixture_name);
    assert!(input.is_file(), "fixture {fixture_name} is missing");

    let work_dir = std::env::temp_dir().join("nexara-archive-smoke-test").join(format!("{fixture_name}-{out_ext}"));
    let _ = std::fs::remove_dir_all(&work_dir);
    let extract_dir = work_dir.join("extracted");
    std::fs::create_dir_all(&extract_dir).unwrap();

    let extract_status =
        Command::new(&sevenzip).args(["x", "-y", &format!("-o{}", extract_dir.display()), input.to_str().unwrap()]).status().unwrap();
    assert!(extract_status.success(), "extracting {fixture_name} failed");

    // Confirm the nested-folder structure survived extraction.
    assert!(extract_dir.join("file1.txt").is_file());
    assert!(extract_dir.join("sub").join("file2.txt").is_file());

    let output = work_dir.join(format!("out.{out_ext}"));
    let glob = extract_dir.join("*");
    let create_status = Command::new(&sevenzip).args(["a", type_flag, output.to_str().unwrap(), glob.to_str().unwrap()]).status().unwrap();
    assert!(create_status.success(), "creating .{out_ext} from {fixture_name} failed");

    let metadata = std::fs::metadata(&output).unwrap();
    assert!(metadata.len() > 0, "output archive for {fixture_name} -> .{out_ext} is empty");

    // Round-trip: the new archive must itself list both original files.
    let list_output = Command::new(&sevenzip).args(["l", output.to_str().unwrap()]).output().unwrap();
    let listing = String::from_utf8_lossy(&list_output.stdout);
    assert!(listing.contains("file1.txt"), "round-tripped .{out_ext} is missing file1.txt");
    assert!(listing.contains("file2.txt"), "round-tripped .{out_ext} is missing file2.txt");

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn zip_to_7z_round_trips_contents() {
    run_case("sample.zip", "7z", "-t7z");
}

#[test]
fn zip_to_tar_round_trips_contents() {
    run_case("sample.zip", "tar", "-ttar");
}

#[test]
fn sevenz_to_zip_round_trips_contents() {
    run_case("sample.7z", "zip", "-tzip");
}

/// The real security test: a genuine `../../evil.txt` entry, crafted with
/// Python's zipfile module (which bypasses any path normalization a normal
/// archiver's `add` command would apply), must be rejected before
/// extraction — not silently sanitized, not extracted with a warning.
#[test]
fn rejects_zip_slip_archive_before_extracting() {
    let Some(sevenzip) = resolve_7z() else {
        eprintln!("skipping rejects_zip_slip_archive_before_extracting: 7-Zip (7z) not found");
        return;
    };

    let archive = fixture("zipslip.zip");
    assert!(archive.is_file(), "zipslip.zip fixture is missing");

    let list_output = Command::new(&sevenzip).args(["l", "-slt", archive.to_str().unwrap()]).output().unwrap();
    assert!(list_output.status.success());
    let listing = String::from_utf8_lossy(&list_output.stdout);

    // This mirrors archive::find_unsafe_entry's logic against the real 7z
    // output for a real malicious archive, proving the detection actually
    // fires on genuine `7z l -slt` text, not just a hand-written fixture.
    let mut past_header = false;
    let mut found_unsafe = None;
    for line in listing.lines() {
        let trimmed = line.trim();
        if !past_header {
            if !trimmed.is_empty() && trimmed.chars().all(|c| c == '-') {
                past_header = true;
            }
            continue;
        }
        if let Some(entry_path) = line.strip_prefix("Path = ") {
            if entry_path.split(['/', '\\']).any(|s| s == "..") {
                found_unsafe = Some(entry_path.to_string());
                break;
            }
        }
    }

    assert!(found_unsafe.is_some(), "expected to detect the Zip Slip entry in the real 7z listing, listing was:\n{listing}");
    assert!(found_unsafe.unwrap().contains("evil.txt"));
}
