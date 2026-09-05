//! Archive extraction, using Nexara's own bundled 7-Zip as the one universal
//! extractor for every `.7z`/`.zip` payload — bundled or downloaded — so
//! nothing else needs its own unpacking logic.

use std::path::Path;

/// Extracts `archive_path` into `dest_dir` (created if missing), overwriting
/// anything already there. Returns 7-Zip's own error output on failure.
pub async fn extract_with_7z(seven_zip_exe: &Path, archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
    tokio::fs::create_dir_all(dest_dir).await.map_err(|e| format!("Could not create extraction directory: {e}"))?;

    let output = tokio::process::Command::new(seven_zip_exe)
        .arg("x")
        .arg(archive_path)
        .arg(format!("-o{}", dest_dir.display()))
        .arg("-y")
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Could not start 7-Zip to extract {}: {e}", archive_path.display()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!("7-Zip could not extract {}: {}", archive_path.display(), if stderr.trim().is_empty() { stdout.trim() } else { stderr.trim() }))
    }
}
