use std::path::{Path, PathBuf};

use super::engine;
use super::jobs::JobRegistry;
use super::process::{self, ExecuteOutcome};

/// 7-Zip writes non-ASCII console output (paths, error text) in the system's
/// legacy codepage by default, not UTF-8 — verified directly: a Turkish
/// filename came back from `7z` as raw single-byte codepage bytes, which
/// `String::from_utf8_lossy` then mangled into mojibake in error messages
/// shown to the user. `-sccUTF-8` forces real UTF-8 output instead (verified
/// directly to fix it, in either argument position).
pub(crate) const UTF8_CONSOLE: &str = "-sccUTF-8";

fn type_flag(format: &str) -> &'static str {
    match format {
        "zip" => "-tzip",
        "7z" => "-t7z",
        "tar" => "-ttar",
        "gz" => "-tgzip",
        _ => "-tzip",
    }
}

/// Scans `7z l -slt` output for the first entry whose path looks unsafe to
/// extract — a `..` segment (Zip Slip) or an absolute path — and returns it.
/// Pure and synchronous so the parsing itself (the security-critical part)
/// is unit-testable without spawning 7z.
fn find_unsafe_entry(listing_output: &str) -> Option<String> {
    let mut past_header = false;
    for line in listing_output.lines() {
        let trimmed = line.trim();
        if !past_header {
            // `7z l -slt` prints a short "--" separator before the
            // archive's OWN summary block (whose "Path = " is the archive
            // file itself — legitimately absolute, not an entry), then a
            // longer "----------" separator before the real per-entry
            // blocks. Only the long one marks the transition we want; a
            // naive "all dashes" check matches the short one too and would
            // misclassify the archive's own path as an unsafe entry.
            if trimmed.len() >= 5 && trimmed.chars().all(|c| c == '-') {
                past_header = true;
            }
            continue;
        }
        if let Some(entry_path) = line.strip_prefix("Path = ") {
            // `Path::is_absolute()` alone isn't enough on Windows: it
            // requires a drive-letter prefix, so a POSIX-style leading
            // slash like `/etc/passwd` is NOT considered absolute — even
            // though extracting to it still escapes the target directory
            // (it resolves against the current drive's root). Check for
            // a leading separator explicitly too.
            let looks_unsafe = entry_path.split(['/', '\\']).any(|segment| segment == "..")
                || entry_path.starts_with('/')
                || entry_path.starts_with('\\')
                || Path::new(entry_path).is_absolute();
            if looks_unsafe {
                return Some(entry_path.to_string());
            }
        }
    }
    None
}

/// Lists an archive's entries via `7z l -slt` and rejects it outright if any
/// entry path looks unsafe to extract. This runs *before* extraction, so a
/// malicious archive never gets the chance to write outside its target
/// directory. `pub(crate)` so the Extract Archive tool (`tools::archive_extract`)
/// reuses this exact check instead of re-implementing Zip Slip protection.
pub(crate) async fn validate_entries(binary: &str, archive_path: &str, password: Option<&str>) -> Result<(), String> {
    let mut args = vec!["l".to_string(), "-slt".to_string(), archive_path.to_string(), UTF8_CONSOLE.to_string()];
    if let Some(p) = password {
        args.push(format!("-p{p}"));
    }
    let output = tokio::process::Command::new(binary)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Could not list archive contents: {e}"))?;

    if !output.status.success() {
        // 7z's "a password is needed" prompt goes to stdout ("Enter
        // password (will not be echoed):"), while a *wrong* password's
        // error goes to stderr ("Wrong password?") — verified directly by
        // capturing each stream separately, so both are checked here
        // rather than assuming either one alone.
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stdout.contains("Enter password") || stderr.contains("Wrong password") {
            return Err("This archive is password-protected. Enter the correct password and try again.".to_string());
        }
        return Err(format!("Could not read this archive: {}", stderr.trim()));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    if let Some(entry_path) = find_unsafe_entry(&text) {
        return Err(format!("This archive contains an unsafe path and was rejected for your safety: {entry_path}"));
    }

    Ok(())
}

pub(crate) async fn extract(registry: &JobRegistry, job_id: &str, binary: &str, archive_path: &str, dest_dir: &str) -> Result<ExecuteOutcome, String> {
    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    let args = vec!["x".to_string(), "-y".to_string(), format!("-o{dest_dir}"), archive_path.to_string(), UTF8_CONSOLE.to_string()];
    process::run_and_track(registry, job_id, binary, &args).await
}

/// If extraction produced exactly one `.tar` file (the common shape for a
/// `.tar.gz`/`.tgz` input, since gzip itself only wraps a single stream),
/// returns its path so the caller can extract that inner layer too.
fn find_single_tar(dir: &Path) -> Option<PathBuf> {
    let entries: Vec<_> = std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).collect();
    if entries.len() != 1 {
        return None;
    }
    let path = entries[0].path();
    if path.extension().and_then(|e| e.to_str())?.eq_ignore_ascii_case("tar") {
        Some(path)
    } else {
        None
    }
}

async fn create(
    registry: &JobRegistry,
    job_id: &str,
    binary: &str,
    output_path: &str,
    output_format: &str,
    source_glob: &str,
) -> Result<ExecuteOutcome, String> {
    if output_format == "gz" {
        // gzip only wraps a single stream, so a multi-file ".tar.gz" needs
        // two passes: tar the contents, then gzip the tar.
        let tar_path = format!("{output_path}.tmp.tar");
        let tar_args =
            vec!["a".to_string(), "-ttar".to_string(), tar_path.clone(), source_glob.to_string(), UTF8_CONSOLE.to_string()];
        let tar_outcome = process::run_and_track(registry, job_id, binary, &tar_args).await?;
        if !tar_outcome.success || tar_outcome.cancelled {
            let _ = std::fs::remove_file(&tar_path);
            return Ok(tar_outcome);
        }

        let gz_args =
            vec!["a".to_string(), "-tgzip".to_string(), output_path.to_string(), tar_path.clone(), UTF8_CONSOLE.to_string()];
        let gz_outcome = process::run_and_track(registry, job_id, binary, &gz_args).await;
        let _ = std::fs::remove_file(&tar_path);
        gz_outcome
    } else {
        let args = vec![
            "a".to_string(),
            type_flag(output_format).to_string(),
            output_path.to_string(),
            source_glob.to_string(),
            UTF8_CONSOLE.to_string(),
        ];
        process::run_and_track(registry, job_id, binary, &args).await
    }
}

/// Converts one archive format to another by extracting the input into a
/// scratch folder and recompressing its contents into the target format.
/// 7-Zip can read RAR but never write it, matching the registry (RAR never
/// appears as an output target).
pub async fn convert(
    registry: &JobRegistry,
    job_id: &str,
    input_path: &str,
    work_dir: &str,
    output_path: &str,
    output_format: &str,
) -> Result<ExecuteOutcome, String> {
    let Some(binary) = engine::resolve_7z() else {
        return Err("7-Zip (7z) could not be found on this system.".to_string());
    };

    validate_entries(&binary, input_path, None).await?;

    let extract_dir = Path::new(work_dir).join("extracted");
    let extract_dir_str = extract_dir.to_string_lossy().to_string();
    let extract_outcome = extract(registry, job_id, &binary, input_path, &extract_dir_str).await?;
    if !extract_outcome.success || extract_outcome.cancelled {
        return Ok(extract_outcome);
    }

    let source_dir = if let Some(tar_path) = find_single_tar(&extract_dir) {
        let inner_dir = extract_dir.join("_untarred");
        let inner_dir_str = inner_dir.to_string_lossy().to_string();
        let untar_outcome = extract(registry, job_id, &binary, &tar_path.to_string_lossy(), &inner_dir_str).await?;
        if !untar_outcome.success || untar_outcome.cancelled {
            return Ok(untar_outcome);
        }
        inner_dir
    } else {
        extract_dir
    };

    let source_glob = source_dir.join("*").to_string_lossy().to_string();
    create(registry, job_id, &binary, output_path, output_format, &source_glob).await
}

/// A lightweight sanity check beyond "the file exists and is non-empty":
/// confirms the produced archive starts with its format's magic bytes.
pub fn validate_output(path: &Path, output_format: &str) -> Result<(), String> {
    let mut header = [0u8; 6];
    let read_len = {
        use std::io::Read;
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        file.read(&mut header).map_err(|e| e.to_string())?
    };
    let header = &header[..read_len];

    let ok = match output_format {
        "zip" => header.starts_with(b"PK\x03\x04") || header.starts_with(b"PK\x05\x06"),
        "7z" => header.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]),
        "gz" => header.starts_with(&[0x1F, 0x8B]),
        _ => return Ok(()), // tar has no reliable fixed-offset signature to check cheaply
    };

    if ok {
        Ok(())
    } else {
        Err(format!("the output doesn't look like a valid {} file", output_format.to_uppercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Matches real `7z l -slt` output exactly (reproduced from an actual
    // run against a generated fixture): a short "--" separator before the
    // archive's OWN "Path = " line, then a "----------" separator before
    // the real per-entry blocks. A first version of these fixtures omitted
    // the short "--" line and passed despite a real bug that misclassified
    // the archive's own path as an entry — this shape is what caught it.
    const SAFE_LISTING: &str = "\
--
Path = C:\\archives\\sample.zip
Type = zip
Physical Size = 294

----------
Path = folder/file1.txt
Folder = -
Size = 6

Path = folder/sub/file2.txt
Folder = -
Size = 6
";

    const ZIP_SLIP_LISTING: &str = "\
--
Path = C:\\archives\\evil.zip
Type = zip
Physical Size = 294

----------
Path = ok.txt
Folder = -
Size = 6

Path = ../../evil.txt
Folder = -
Size = 6
";

    const ABSOLUTE_PATH_LISTING: &str = "\
--
Path = C:\\archives\\evil2.zip
Type = zip
Physical Size = 294

----------
Path = /etc/passwd
Folder = -
Size = 6
";

    #[test]
    fn find_unsafe_entry_accepts_normal_relative_paths() {
        assert_eq!(find_unsafe_entry(SAFE_LISTING), None);
    }

    #[test]
    fn find_unsafe_entry_catches_zip_slip() {
        assert_eq!(find_unsafe_entry(ZIP_SLIP_LISTING), Some("../../evil.txt".to_string()));
    }

    #[test]
    fn find_unsafe_entry_catches_absolute_paths() {
        assert_eq!(find_unsafe_entry(ABSOLUTE_PATH_LISTING), Some("/etc/passwd".to_string()));
    }

    #[test]
    fn find_unsafe_entry_ignores_the_archives_own_header_path() {
        // The archive's own summary block (before the "----------" line)
        // also has a "Path = " line — for the archive file itself, which
        // legitimately IS an absolute path. That must not trigger rejection.
        let listing = "Path = C:\\Users\\me\\archive.zip\nType = zip\n\n----------\nPath = inner.txt\n";
        assert_eq!(find_unsafe_entry(listing), None);
    }

    #[test]
    fn type_flag_maps_known_formats() {
        assert_eq!(type_flag("zip"), "-tzip");
        assert_eq!(type_flag("7z"), "-t7z");
        assert_eq!(type_flag("tar"), "-ttar");
        assert_eq!(type_flag("gz"), "-tgzip");
    }

    #[test]
    fn find_single_tar_detects_lone_tar_file() {
        let dir = std::env::temp_dir().join("nexara-test-archive-find-tar");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("contents.tar"), b"fake tar").unwrap();
        assert!(find_single_tar(&dir).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_single_tar_ignores_multiple_files() {
        let dir = std::env::temp_dir().join("nexara-test-archive-find-tar-multi");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.tar"), b"fake").unwrap();
        std::fs::write(dir.join("b.txt"), b"fake").unwrap();
        assert!(find_single_tar(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_single_tar_ignores_non_tar_single_file() {
        let dir = std::env::temp_dir().join("nexara-test-archive-find-tar-notar");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("readme.txt"), b"fake").unwrap();
        assert!(find_single_tar(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_accepts_real_zip_header() {
        let dir = std::env::temp_dir().join("nexara-test-archive-validate-zip");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.zip");
        std::fs::write(&path, [0x50, 0x4B, 0x03, 0x04, 0, 0]).unwrap();
        assert!(validate_output(&path, "zip").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_rejects_wrong_zip_header() {
        let dir = std::env::temp_dir().join("nexara-test-archive-validate-zip2");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("not-a.zip");
        std::fs::write(&path, b"not a zip file").unwrap();
        assert!(validate_output(&path, "zip").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_accepts_real_7z_header() {
        let dir = std::env::temp_dir().join("nexara-test-archive-validate-7z");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fake.7z");
        std::fs::write(&path, [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]).unwrap();
        assert!(validate_output(&path, "7z").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
