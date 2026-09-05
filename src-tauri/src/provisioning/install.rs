//! Silent installation for the two engines that genuinely need a real OS
//! install rather than a plain extract (LibreOffice, Calibre — MSI; and
//! FontForge — Inno Setup). Each runs fully unattended: no dialogs, no
//! reboot prompt, nothing for the user to click through.

use std::path::Path;

/// Runs `msiexec /i <path> /qn /norestart`, which installs the package
/// silently to its default location (Program Files) — matching exactly how
/// a user who ran the installer by hand and clicked through defaults would
/// end up, so the existing Program-Files probing in `conversion::engine`
/// finds it unchanged.
pub async fn run_msi_silent(msi_path: &Path) -> Result<(), String> {
    let output = tokio::process::Command::new("msiexec")
        .arg("/i")
        .arg(msi_path)
        .arg("/qn")
        .arg("/norestart")
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Could not start msiexec: {e}"))?;

    // msiexec's own docs list 0 (success) and 3010 (success, reboot
    // required — which /norestart suppresses, but the exit code still
    // reports it) as the only non-error outcomes.
    match output.status.code() {
        Some(0) | Some(3010) => Ok(()),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("msiexec exited with code {:?}: {}", output.status.code(), stderr.trim()))
        }
    }
}

/// Runs an Inno Setup installer fully unattended via its documented silent
/// switches (`/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`) — no custom `/DIR`,
/// so it lands at its normal default location and resolves through the same
/// Program-Files probing every other engine uses.
pub async fn run_inno_silent(installer_path: &Path) -> Result<(), String> {
    let output = tokio::process::Command::new(installer_path)
        .arg("/VERYSILENT")
        .arg("/SUPPRESSMSGBOXES")
        .arg("/NORESTART")
        .arg("/SP-")
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Could not start the installer: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Installer exited with code {:?}: {}", output.status.code(), stderr.trim()))
    }
}
