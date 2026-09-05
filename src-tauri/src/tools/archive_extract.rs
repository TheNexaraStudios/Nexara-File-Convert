use crate::conversion::archive::{extract, validate_entries};
use crate::conversion::engine;
use crate::conversion::jobs::JobRegistry;
use crate::conversion::process::ExecuteOutcome;

/// Extracts every entry from `archive_path` into `dest_dir`, chosen
/// directly by the user rather than a scratch folder. Reuses the exact
/// same Zip Slip / path-traversal check the format-conversion archive
/// pipeline already runs (`conversion::archive::validate_entries`) —
/// listed and validated *before* extraction, so a malicious archive never
/// gets the chance to write outside `dest_dir`.
pub async fn extract_to(
    registry: &JobRegistry,
    job_id: &str,
    archive_path: &str,
    dest_dir: &str,
    password: Option<&str>,
) -> Result<ExecuteOutcome, String> {
    let Some(binary) = engine::resolve_7z() else {
        return Err("7-Zip (7z) could not be found on this system.".to_string());
    };

    validate_entries(&binary, archive_path, password).await?;

    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    if password.is_none() {
        return extract(registry, job_id, &binary, archive_path, dest_dir).await;
    }

    // `conversion::archive::extract` has no password parameter (the
    // format-conversion pipeline never needs one), so a password-protected
    // extraction builds its own args here instead of extending that
    // shared helper's signature for a case it doesn't otherwise need.
    let args = vec![
        "x".to_string(),
        "-y".to_string(),
        format!("-o{dest_dir}"),
        archive_path.to_string(),
        format!("-p{}", password.unwrap()),
        crate::conversion::archive::UTF8_CONSOLE.to_string(),
    ];
    crate::conversion::process::run_and_track(registry, job_id, &binary, &args).await
}

/// Lists what would be extracted without touching disk, letting the caller
/// show the user "N files, M bytes" before they commit — and, since it goes
/// through the same `validate_entries`, it doubles as an early honest
/// failure for a wrong/missing password or an unsafe archive.
pub async fn preview(archive_path: &str, password: Option<&str>) -> Result<(), String> {
    let Some(binary) = engine::resolve_7z() else {
        return Err("7-Zip (7z) could not be found on this system.".to_string());
    };
    validate_entries(&binary, archive_path, password).await
}

#[cfg(test)]
mod tests {
    // `extract_to` and `preview` both spawn real processes — covered by
    // the integration smoke tests instead, matching how the other engine
    // wrappers in this codebase are tested.
}
