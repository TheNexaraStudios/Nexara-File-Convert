//! Real, end-to-end verification of the provisioning pipeline's actual
//! mechanics — no mocks. `tauri::test`'s `MockRuntime` pulls in the same
//! WebView2 plumbing as the real desktop runtime on Windows and isn't
//! reliably runnable as a plain test binary in every environment, so these
//! tests exercise the underlying `extract`/`download` modules directly
//! against real files instead of going through an `AppHandle` — the exact
//! same code paths `provisioning::provision_one` calls, just invoked with
//! manually-resolved real paths rather than Tauri's resource resolver.

use nexara_file_convert_lib::provisioning::{download, extract, spec};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `.../src-tauri`; the bundled resource archives
    // this test extracts for real live one level up, under `resources/`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn bundled_seven_zip_exe() -> PathBuf {
    repo_root().join("resources/engines/7zip/7z.exe")
}

fn test_scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("nexara-provisioning-test").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn bundled_seven_zip_is_present_and_runs() {
    let exe = bundled_seven_zip_exe();
    assert!(exe.is_file(), "bundled 7z.exe missing at {}", exe.display());

    let output = std::process::Command::new(&exe).output().expect("could not run bundled 7z.exe");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("7-Zip"), "unexpected 7z output: {text}");
}

#[tokio::test]
async fn every_bundled_archive_matches_its_pinned_hash() {
    // Confirms the actual files shipped in resources/ still match what
    // spec.rs claims to have verified — catches a resource silently
    // replaced or corrupted without the spec being updated to match.
    for s in spec::BUNDLED {
        if s.kind == spec::PayloadKind::LooseBinary {
            continue; // 7z.exe/7z.dll are checked as loose files below instead.
        }
        let path = repo_root().join("resources").join(s.resource_relative.expect("bundled spec has a resource path"));
        assert!(path.is_file(), "{}: bundled resource missing at {}", s.id, path.display());
        let result = download::verify_sha256(&path, s.sha256).await;
        assert!(result.is_ok(), "{}: {result:?}", s.id);
    }

    let seven_zip_dir = repo_root().join("resources/engines/7zip");
    for (file, expected_sha256) in [
        ("7z.exe", "6ee3c0ed0b27663c1b948ae85a7c0bb073aed1498983182f3f0df1f6a8c30b2f"),
        ("7z.dll", "65e4c1f855f9ef6e8f0f5df8e3f27d9eb5f07311408639da0a1ca0b8f4871b0d"),
    ] {
        let result = download::verify_sha256(&seven_zip_dir.join(file), expected_sha256).await;
        assert!(result.is_ok(), "{file}: {result:?}");
    }
}

#[tokio::test]
async fn imagemagick_extracts_from_its_real_bundled_archive_and_runs() {
    let spec = spec::find("magick").expect("magick spec missing");
    let archive = repo_root().join("resources").join(spec.resource_relative.unwrap());
    let dest = test_scratch_dir("imagemagick-extract");

    extract::extract_with_7z(&bundled_seven_zip_exe(), &archive, &dest).await.expect("extraction failed");

    let exe = dest.join(spec.exe_relative.unwrap());
    assert!(exe.is_file(), "magick.exe not found after extraction at {}", exe.display());

    let output = std::process::Command::new(&exe).arg("-version").output().expect("could not run extracted magick.exe");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("ImageMagick"), "unexpected magick -version output: {text}");

    let _ = std::fs::remove_dir_all(&dest);
}

#[tokio::test]
async fn pandoc_extracts_from_its_real_bundled_archive_and_runs() {
    let spec = spec::find("pandoc").expect("pandoc spec missing");
    let archive = repo_root().join("resources").join(spec.resource_relative.unwrap());
    let dest = test_scratch_dir("pandoc-extract");

    extract::extract_with_7z(&bundled_seven_zip_exe(), &archive, &dest).await.expect("extraction failed");

    let exe = dest.join(spec.exe_relative.unwrap());
    let output = std::process::Command::new(&exe).arg("--version").output().expect("could not run extracted pandoc.exe");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("pandoc"), "unexpected pandoc --version output: {text}");

    let _ = std::fs::remove_dir_all(&dest);
}

/// The one real network test in this suite: downloads MuPDF's official
/// Windows release for real, verifies its pinned SHA-256 for real, extracts
/// it with the bundled 7-Zip, and runs the resulting `mutool.exe`. Slow
/// (tens of MB over the network) and network-dependent by nature, but a
/// mocked version of this test would prove nothing about whether the real
/// download-tier pipeline — the one thing this whole feature exists to get
/// right — actually works end to end.
#[tokio::test]
async fn mutool_downloads_verifies_and_extracts_for_real() {
    let spec = spec::find("mutool").expect("mutool spec missing");
    let dest_dir = test_scratch_dir("mutool-download");
    let archive_path = dest_dir.join("mutool.zip");

    let progress = |_downloaded: u64, _total: Option<u64>| {};
    download::download_with_retry(spec.url.unwrap(), &archive_path, spec.sha256, &progress)
        .await
        .expect("real download of the official MuPDF release failed");

    let extract_dir = dest_dir.join("extracted");
    extract::extract_with_7z(&bundled_seven_zip_exe(), &archive_path, &extract_dir).await.expect("extraction failed");

    let exe = extract_dir.join(spec.exe_relative.unwrap());
    assert!(exe.is_file(), "mutool.exe not found after extraction at {}", exe.display());

    let output = std::process::Command::new(&exe).arg("-v").output().expect("could not run downloaded mutool.exe");
    let combined = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(combined.to_lowercase().contains("mutool"), "unexpected mutool -v output: {combined}");

    let _ = std::fs::remove_dir_all(&dest_dir);
}

#[test]
fn readiness_snapshot_lists_every_known_engine() {
    let ids: Vec<&str> = spec::BUNDLED.iter().chain(spec::DOWNLOADED.iter()).map(|s| s.id).collect();
    for expected in ["7z", "magick", "inkscape", "pandoc", "ffmpeg", "mutool", "ebook-convert", "soffice", "fontforge"] {
        assert!(ids.contains(&expected), "engine spec table missing '{expected}': {ids:?}");
    }
}
