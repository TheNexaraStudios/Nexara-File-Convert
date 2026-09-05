use crate::conversion::archive::UTF8_CONSOLE;
use crate::conversion::engine;
use crate::conversion::jobs::JobRegistry;
use crate::conversion::process::{self, ExecuteOutcome};

/// Formats 7-Zip can genuinely *create* — verified directly against this
/// build. RAR is deliberately excluded: 7-Zip can extract it but never
/// write it, matching the existing archive-conversion engine's rule.
pub const CREATABLE_FORMATS: &[&str] = &["zip", "7z", "tar", "gz"];

/// Only zip and 7z got a real, verified encryption test — AES-256 via
/// `-p<password>` plus `-mem=AES256` (zip) / `-mhe=on` (7z, which also
/// hides filenames, not just contents). TAR/gzip have no meaningful
/// encryption in 7-Zip's implementation, so password is never offered for
/// them rather than silently producing an unencrypted archive.
pub fn supports_password(format: &str) -> bool {
    matches!(format, "zip" | "7z")
}

fn type_flag(format: &str) -> &'static str {
    match format {
        "zip" => "-tzip",
        "7z" => "-t7z",
        "tar" => "-ttar",
        "gz" => "-tgzip",
        _ => "-tzip",
    }
}

fn creation_args(inputs: &[String], output_path: &str, format: &str, compression_level: u8, password: Option<&str>) -> Vec<String> {
    let level = compression_level.min(9);
    let mut args = vec!["a".to_string(), type_flag(format).to_string(), format!("-mx={level}"), output_path.to_string()];
    args.extend(inputs.iter().cloned());
    if let Some(p) = password {
        if supports_password(format) {
            args.push(format!("-p{p}"));
            args.push(if format == "7z" { "-mhe=on".to_string() } else { "-mem=AES256".to_string() });
        }
    }
    args.push(UTF8_CONSOLE.to_string());
    args
}

/// Creates `output_path` from `inputs` (files and/or folders, in any mix of
/// locations — not required to share a parent directory). Password is
/// silently ignored for formats `supports_password` rejects, rather than
/// erroring, so a caller that always passes the user's password field
/// through doesn't need format-specific branching of its own.
pub async fn create(
    registry: &JobRegistry,
    job_id: &str,
    inputs: &[String],
    output_path: &str,
    format: &str,
    compression_level: u8,
    password: Option<&str>,
) -> Result<ExecuteOutcome, String> {
    if inputs.is_empty() {
        return Err("Select at least one file or folder to archive.".to_string());
    }
    let Some(binary) = engine::resolve_7z() else {
        return Err("7-Zip (7z) could not be found on this system.".to_string());
    };

    if format == "gz" {
        // gzip only wraps a single stream, so any number of inputs — one
        // file, several files, a folder — first gets tarred together, the
        // same two-pass approach the format-conversion archive engine
        // already uses for `.tar.gz` output.
        return create_tar_then_gzip(registry, job_id, &binary, inputs, output_path).await;
    }

    let args = creation_args(inputs, output_path, format, compression_level, password);
    process::run_and_track(registry, job_id, &binary, &args).await
}

async fn create_tar_then_gzip(
    registry: &JobRegistry,
    job_id: &str,
    binary: &str,
    inputs: &[String],
    output_path: &str,
) -> Result<ExecuteOutcome, String> {
    let tar_path = format!("{output_path}.tmp.tar");
    let mut tar_args = vec!["a".to_string(), "-ttar".to_string(), tar_path.clone()];
    tar_args.extend(inputs.iter().cloned());
    tar_args.push(UTF8_CONSOLE.to_string());

    let tar_outcome = process::run_and_track(registry, job_id, binary, &tar_args).await?;
    if !tar_outcome.success || tar_outcome.cancelled {
        let _ = std::fs::remove_file(&tar_path);
        return Ok(tar_outcome);
    }

    let gz_args = vec!["a".to_string(), "-tgzip".to_string(), output_path.to_string(), tar_path.clone(), UTF8_CONSOLE.to_string()];
    let gz_outcome = process::run_and_track(registry, job_id, binary, &gz_args).await;
    let _ = std::fs::remove_file(&tar_path);
    gz_outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_args_includes_type_flag_level_output_then_inputs() {
        let inputs = vec![r"C:\a.txt".to_string(), r"C:\folder".to_string()];
        let args = creation_args(&inputs, r"C:\out.zip", "zip", 9, None);
        assert_eq!(args[0], "a");
        assert_eq!(args[1], "-tzip");
        assert_eq!(args[2], "-mx=9");
        assert_eq!(args[3], r"C:\out.zip");
        assert!(args.contains(&r"C:\a.txt".to_string()));
        assert!(args.contains(&r"C:\folder".to_string()));
    }

    #[test]
    fn creation_args_clamps_compression_level_to_nine() {
        let args = creation_args(&["a.txt".to_string()], "out.zip", "zip", 15, None);
        assert!(args.contains(&"-mx=9".to_string()));
    }

    #[test]
    fn creation_args_adds_password_and_aes256_for_zip() {
        let args = creation_args(&["a.txt".to_string()], "out.zip", "zip", 5, Some("secret"));
        assert!(args.contains(&"-psecret".to_string()));
        assert!(args.contains(&"-mem=AES256".to_string()));
    }

    #[test]
    fn creation_args_adds_password_and_header_encryption_for_7z() {
        let args = creation_args(&["a.txt".to_string()], "out.7z", "7z", 5, Some("secret"));
        assert!(args.contains(&"-psecret".to_string()));
        assert!(args.contains(&"-mhe=on".to_string()));
    }

    #[test]
    fn creation_args_ignores_password_for_tar() {
        let args = creation_args(&["a.txt".to_string()], "out.tar", "tar", 5, Some("secret"));
        assert!(!args.iter().any(|a| a.starts_with("-p")));
    }

    #[test]
    fn supports_password_only_for_zip_and_7z() {
        assert!(supports_password("zip"));
        assert!(supports_password("7z"));
        assert!(!supports_password("tar"));
        assert!(!supports_password("gz"));
    }
}
