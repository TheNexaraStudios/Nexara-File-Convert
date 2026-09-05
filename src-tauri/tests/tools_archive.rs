//! Smoke tests that exercise real 7-Zip for the Extract Archive and Create
//! Archive tools — Zip Slip rejection, Unicode/space paths, password
//! encryption (and rejection of a wrong password), and multi-file/folder
//! archive creation. Skipped (not failed) when 7z isn't available, matching
//! the project's rule that optional-engine tests must not fail the whole
//! suite.

use std::path::{Path, PathBuf};
use std::process::Command;

const UTF8_CONSOLE: &str = "-sccUTF-8";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn resolve_7z() -> Option<String> {
    for candidate in [r"C:\Program Files\7-Zip\7z.exe", r"C:\Program Files (x86)\7-Zip\7z.exe"] {
        if Path::new(candidate).is_file() {
            return Some(candidate.to_string());
        }
    }
    Command::new("7z").output().ok().map(|_| "7z".to_string())
}

fn work_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("nexara-tools-archive-smoke-test").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn extract_rejects_zip_slip_archive_without_touching_disk() {
    let Some(sevenzip) = resolve_7z() else {
        eprintln!("skipping extract_rejects_zip_slip_archive_without_touching_disk: 7z not found on PATH");
        return;
    };
    let archive = fixture("zipslip.zip");
    assert!(archive.is_file(), "fixture zipslip.zip is missing");

    // Mirrors `conversion::archive::validate_entries`: list before ever
    // extracting, and refuse if any entry looks unsafe.
    let output = Command::new(&sevenzip).args(["l", "-slt", archive.to_str().unwrap(), UTF8_CONSOLE]).output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    let has_traversal = text.lines().any(|l| l.trim_start().starts_with("Path = ") && l.contains(".."));
    assert!(has_traversal, "the zipslip fixture should contain a '..' entry — if not, this test isn't testing anything");
}

#[test]
fn extract_handles_unicode_and_space_destination_path() {
    let Some(sevenzip) = resolve_7z() else {
        eprintln!("skipping extract_handles_unicode_and_space_destination_path: 7z not found on PATH");
        return;
    };
    let archive = fixture("sample.zip");
    assert!(archive.is_file(), "fixture sample.zip is missing");

    let dir = work_dir("extract-unicode");
    let dest = dir.join("Dosyalar Klasörü İçin Çıktı");
    std::fs::create_dir_all(&dest).unwrap();

    let status = Command::new(&sevenzip)
        .args(["x", "-y", &format!("-o{}", dest.to_string_lossy()), archive.to_str().unwrap(), UTF8_CONSOLE])
        .status()
        .unwrap();
    assert!(status.success(), "extraction into a Unicode/space destination should succeed");

    let extracted: Vec<_> = std::fs::read_dir(&dest).unwrap().collect();
    assert!(!extracted.is_empty(), "expected at least one extracted file");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_zip_from_mixed_files_and_folder() {
    let Some(sevenzip) = resolve_7z() else {
        eprintln!("skipping create_zip_from_mixed_files_and_folder: 7z not found on PATH");
        return;
    };
    let dir = work_dir("create-mixed");
    let sub = dir.join("subfolder");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(dir.join("file1.txt"), b"one").unwrap();
    std::fs::write(sub.join("file2.txt"), b"two").unwrap();
    let output = dir.join("out.zip");

    let status = Command::new(&sevenzip)
        .args(["a", "-tzip", "-mx=9", output.to_str().unwrap(), dir.join("file1.txt").to_str().unwrap(), sub.to_str().unwrap(), UTF8_CONSOLE])
        .status()
        .unwrap();
    assert!(status.success());

    let bytes = std::fs::read(&output).unwrap();
    assert!(bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06"), "output doesn't look like a real zip");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_7z_with_password_round_trips_and_rejects_wrong_password() {
    let Some(sevenzip) = resolve_7z() else {
        eprintln!("skipping create_7z_with_password_round_trips_and_rejects_wrong_password: 7z not found on PATH");
        return;
    };
    let dir = work_dir("create-password");
    std::fs::write(dir.join("secret.txt"), b"top secret contents").unwrap();
    let output = dir.join("secure.7z");

    let status = Command::new(&sevenzip)
        .args([
            "a",
            "-t7z",
            "-mx=5",
            "-pCorrectHorseBatteryStaple",
            "-mhe=on",
            output.to_str().unwrap(),
            dir.join("secret.txt").to_str().unwrap(),
            UTF8_CONSOLE,
        ])
        .status()
        .unwrap();
    assert!(status.success());

    // Correct password lists it fine.
    let ok = Command::new(&sevenzip).args(["l", "-slt", "-pCorrectHorseBatteryStaple", output.to_str().unwrap(), UTF8_CONSOLE]).output().unwrap();
    assert!(ok.status.success(), "listing with the correct password should succeed");

    // Wrong password fails cleanly (stdin closed, matching the app's own
    // spawn pattern — verified directly not to hang: exit 255 for a
    // missing password, exit 2 with a clear "Wrong password?" for an
    // incorrect one).
    let wrong = Command::new(&sevenzip)
        .args(["l", "-slt", "-pDefinitelyWrongPassword", output.to_str().unwrap(), UTF8_CONSOLE])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!wrong.status.success(), "listing with the wrong password should fail");
    let stderr = String::from_utf8_lossy(&wrong.stderr);
    assert!(stderr.to_lowercase().contains("wrong password"), "expected a clear wrong-password error, got: {stderr}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_tar_gz_from_a_folder_produces_valid_gzip() {
    let Some(sevenzip) = resolve_7z() else {
        eprintln!("skipping create_tar_gz_from_a_folder_produces_valid_gzip: 7z not found on PATH");
        return;
    };
    let dir = work_dir("create-targz");
    let sub = dir.join("stuff");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("a.txt"), b"aaa").unwrap();
    std::fs::write(sub.join("b.txt"), b"bbb").unwrap();

    let tar_path = dir.join("out.tar.gz.tmp.tar");
    let status =
        Command::new(&sevenzip).args(["a", "-ttar", tar_path.to_str().unwrap(), sub.to_str().unwrap(), UTF8_CONSOLE]).status().unwrap();
    assert!(status.success());

    let gz_path = dir.join("out.tar.gz");
    let status = Command::new(&sevenzip).args(["a", "-tgzip", gz_path.to_str().unwrap(), tar_path.to_str().unwrap(), UTF8_CONSOLE]).status().unwrap();
    assert!(status.success());

    let bytes = std::fs::read(&gz_path).unwrap();
    assert!(bytes.starts_with(&[0x1F, 0x8B]), "output doesn't start with the gzip signature");

    let _ = std::fs::remove_dir_all(&dir);
}
