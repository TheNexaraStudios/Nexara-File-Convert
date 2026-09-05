use serde::Serialize;

use crate::conversion::engine;
use crate::provisioning;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionFailure {
    pub id: String,
    pub message: String,
}

/// A cheap, local, no-network snapshot of whether every engine Nexara knows
/// about is currently resolvable — used both to decide whether the
/// first-run setup screen needs to show at all, and to power the
/// Conversion Engines settings screen's health check.
#[tauri::command]
pub fn get_engine_readiness(app: tauri::AppHandle) -> Vec<provisioning::EngineReadiness> {
    provisioning::readiness_snapshot(&app)
}

/// Runs full provisioning for every engine that isn't already ready:
/// extracting bundled archives, downloading + hash-verifying + extracting
/// or silently installing anything download-tier. Emits
/// `nexara://provisioning-progress` events throughout so the frontend can
/// show real per-engine progress. Safe to call again after a partial
/// failure — already-ready engines are skipped instantly.
#[tauri::command]
pub async fn run_engine_provisioning(app: tauri::AppHandle) -> Vec<ProvisionFailure> {
    let failures = provisioning::ensure_all(&app).await;
    // Refresh the resolved-binary cache so newly-extracted/installed
    // engines are picked up by the rest of the app without a restart.
    engine::init_resolved_binaries(&app);
    failures.into_iter().map(|(id, message)| ProvisionFailure { id, message }).collect()
}
