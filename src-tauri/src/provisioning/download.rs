//! Streaming download with retry and SHA-256 verification. Used for every
//! download-tier engine payload — never trusts a completed download until
//! its hash matches the pinned value in `spec.rs`.

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::io::AsyncWriteExt;

const MAX_ATTEMPTS: u32 = 3;

pub type ProgressFn<'a> = dyn Fn(u64, Option<u64>) + Send + Sync + 'a;

/// Downloads `url` to `dest` (via a `.part` sibling, renamed only on
/// success), retrying transient failures up to `MAX_ATTEMPTS` times with a
/// short backoff, then verifies the result against `expected_sha256`.
/// Deletes the file and returns an error on a hash mismatch — a corrupted or
/// tampered download is never silently accepted.
pub async fn download_with_retry(url: &str, dest: &Path, expected_sha256: &str, on_progress: &ProgressFn<'_>) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("NexaraFileConvert-Provisioning/1.0")
        .build()
        .map_err(|e| format!("Could not build HTTP client: {e}"))?;

    let mut last_error = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        match try_download_once(&client, url, dest, on_progress).await {
            Ok(()) => match verify_sha256(dest, expected_sha256).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    let _ = tokio::fs::remove_file(dest).await;
                    return Err(e); // A hash mismatch is never worth retrying — the URL is wrong.
                }
            },
            Err(e) => {
                last_error = e;
                let _ = tokio::fs::remove_file(part_path(dest)).await;
                if attempt < MAX_ATTEMPTS {
                    tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
                }
            }
        }
    }
    Err(format!("Download failed after {MAX_ATTEMPTS} attempts: {last_error}"))
}

fn part_path(dest: &Path) -> std::path::PathBuf {
    dest.with_extension(format!("{}.part", dest.extension().and_then(|e| e.to_str()).unwrap_or("bin")))
}

async fn try_download_once(client: &reqwest::Client, url: &str, dest: &Path, on_progress: &ProgressFn<'_>) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| format!("Could not create download directory: {e}"))?;
    }

    let response = client.get(url).send().await.map_err(|e| format!("Request failed: {e}"))?;
    let response = response.error_for_status().map_err(|e| format!("Server returned an error: {e}"))?;
    let total = response.content_length();

    let part = part_path(dest);
    let mut file = tokio::fs::File::create(&part).await.map_err(|e| format!("Could not create temp file: {e}"))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Connection interrupted: {e}"))?;
        file.write_all(&chunk).await.map_err(|e| format!("Could not write to disk: {e}"))?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total);
    }
    file.flush().await.map_err(|e| format!("Could not flush download to disk: {e}"))?;
    drop(file);

    tokio::fs::rename(&part, dest).await.map_err(|e| format!("Could not finalize download: {e}"))?;
    Ok(())
}

pub async fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), String> {
    let bytes = tokio::fs::read(path).await.map_err(|e| format!("Could not read downloaded file to verify it: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = hex::encode(hasher.finalize());
    if actual.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        Err(format!(
            "Downloaded file did not match the expected checksum (expected {expected_hex}, got {actual}) — the download may be corrupted or the source may have changed."
        ))
    }
}

/// Minimal hex encoding so this module doesn't need the full `hex` crate
/// just for one function.
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn verify_sha256_accepts_a_matching_hash() {
        let dir = std::env::temp_dir().join("nexara-test-provisioning").join("verify-ok");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.bin");
        std::fs::write(&path, b"hello world").unwrap();
        // sha256("hello world")
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(verify_sha256(&path, expected).await.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn verify_sha256_rejects_a_mismatched_hash() {
        let dir = std::env::temp_dir().join("nexara-test-provisioning").join("verify-bad");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.bin");
        std::fs::write(&path, b"tampered content").unwrap();
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(verify_sha256(&path, expected).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn verify_sha256_is_case_insensitive() {
        let dir = std::env::temp_dir().join("nexara-test-provisioning").join("verify-case");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.bin");
        std::fs::write(&path, b"hello world").unwrap();
        let expected = "B94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9";
        assert!(verify_sha256(&path, expected).await.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
