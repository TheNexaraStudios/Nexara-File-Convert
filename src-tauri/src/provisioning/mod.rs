//! Orchestrates making every conversion engine available without the user
//! ever installing anything by hand: small, permissively-compatible engines
//! ship inside Nexara's own installer and just need extracting; large or
//! AGPL-risk engines are fetched from their official upstream source at
//! install/first-run time, hash-verified, then extracted or silently
//! installed. Everything here is idempotent — safe to call again if a
//! previous run was interrupted or partially failed.
//!
//! Every function here is generic over `R: tauri::Runtime` rather than
//! hardcoding the real `Wry` runtime, so the real end-to-end pipeline can be
//! exercised directly against `tauri::test::MockRuntime` in integration
//! tests (see `tests/provisioning.rs`) — using the app's real
//! `tauri.conf.json` context, so resource resolution behaves identically to
//! the shipped app.

pub mod download;
pub mod extract;
pub mod install;
pub mod spec;

use serde::Serialize;
use spec::{EngineSpec, PayloadKind, BUNDLED, DOWNLOADED, FFPROBE_EXE_RELATIVE};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, Runtime};

pub fn engines_root<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path().app_data_dir().map(|d| d.join("engines")).map_err(|e| format!("Could not resolve app data directory: {e}"))
}

fn cache_root<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path().app_data_dir().map(|d| d.join("download-cache")).map_err(|e| format!("Could not resolve app data directory: {e}"))
}

/// Where 7-Zip itself lives — the one bundled binary every other archive on
/// this list gets extracted with, so it resolves straight from Nexara's own
/// read-only resource directory rather than needing a copy step.
pub fn seven_zip_exe_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .resolve("resources/engines/7zip/7z.exe", tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("Could not resolve bundled 7-Zip: {e}"))
}

fn resource_path<R: Runtime>(app: &AppHandle<R>, relative: &str) -> Result<PathBuf, String> {
    app.path()
        .resolve(format!("resources/{relative}"), tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("Could not resolve bundled resource '{relative}': {e}"))
}

/// The resolved, ready-to-run executable path for an archive/loose-binary
/// engine, if it's been provisioned — `None` if not yet extracted. Does not
/// apply to installer-based engines (soffice/ebook-convert/fontforge),
/// which resolve through `conversion::engine`'s existing Program-Files/PATH
/// probing instead, since a real OS install doesn't land in our app-data
/// tree.
pub fn resolved_exe_path<R: Runtime>(app: &AppHandle<R>, id: &str) -> Option<PathBuf> {
    let s = spec::find(id)?;
    match s.kind {
        PayloadKind::LooseBinary => seven_zip_exe_path(app).ok().filter(|p| p.is_file()),
        PayloadKind::SevenZipArchive | PayloadKind::ZipArchive => {
            let root = engines_root(app).ok()?;
            let exe = root.join(id).join(s.exe_relative?);
            exe.is_file().then_some(exe)
        }
        PayloadKind::MsiInstaller | PayloadKind::InnoInstaller => None,
    }
}

/// Same as `resolved_exe_path`, but for ffprobe specifically, which shares
/// ffmpeg's archive under a different executable name.
pub fn resolved_ffprobe_path<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    let root = engines_root(app).ok()?;
    let exe = root.join("ffmpeg").join(FFPROBE_EXE_RELATIVE);
    exe.is_file().then_some(exe)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisionPhase {
    Pending,
    Downloading,
    Verifying,
    Extracting,
    Installing,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionEvent {
    pub id: String,
    pub display_name: String,
    pub phase: ProvisionPhase,
    pub bytes_downloaded: Option<u64>,
    pub bytes_total: Option<u64>,
    pub message: Option<String>,
}

fn emit<R: Runtime>(
    app: &AppHandle<R>,
    spec: &EngineSpec,
    phase: ProvisionPhase,
    bytes_downloaded: Option<u64>,
    bytes_total: Option<u64>,
    message: Option<String>,
) {
    let _ = app.emit(
        "nexara://provisioning-progress",
        ProvisionEvent { id: spec.id.to_string(), display_name: spec.display_name.to_string(), phase, bytes_downloaded, bytes_total, message },
    );
}

/// Provisions one engine end to end: bundled archives just get extracted
/// from Nexara's own resources; download-tier archives get fetched,
/// hash-verified, and extracted; download-tier installers get fetched,
/// hash-verified, and silently installed. Safe to call repeatedly — skips
/// straight to `Ready` if the engine is already resolvable.
pub async fn provision_one<R: Runtime>(app: &AppHandle<R>, s: &EngineSpec) -> Result<(), String> {
    if s.kind == PayloadKind::LooseBinary {
        // 7-Zip ships loose and resolves directly from the resource
        // directory — nothing to extract or install.
        return if seven_zip_exe_path(app)?.is_file() {
            emit(app, s, ProvisionPhase::Ready, None, None, None);
            Ok(())
        } else {
            let msg = "Bundled 7-Zip is missing from this installation.".to_string();
            emit(app, s, ProvisionPhase::Failed, None, None, Some(msg.clone()));
            Err(msg)
        };
    }

    if already_ready(app, s) {
        emit(app, s, ProvisionPhase::Ready, None, None, None);
        return Ok(());
    }

    emit(app, s, ProvisionPhase::Pending, None, None, None);

    let archive_path: PathBuf = if s.bundled {
        resource_path(app, s.resource_relative.expect("bundled spec has a resource path"))?
    } else {
        let cache = cache_root(app)?;
        let ext = match s.kind {
            PayloadKind::ZipArchive => "zip",
            PayloadKind::SevenZipArchive => "7z",
            PayloadKind::MsiInstaller => "msi",
            PayloadKind::InnoInstaller => "exe",
            PayloadKind::LooseBinary => unreachable!(),
        };
        let dest = cache.join(format!("{}-{}.{ext}", s.id, s.version));
        if dest.is_file() && download::verify_sha256(&dest, s.sha256).await.is_ok() {
            dest
        } else {
            emit(app, s, ProvisionPhase::Downloading, Some(0), None, None);
            let app_for_progress = app.clone();
            let spec_for_progress = *s;
            let url = s.url.expect("download-tier spec has a url");
            let progress = move |downloaded: u64, total: Option<u64>| {
                emit(&app_for_progress, &spec_for_progress, ProvisionPhase::Downloading, Some(downloaded), total, None);
            };
            if let Err(e) = download::download_with_retry(url, &dest, s.sha256, &progress).await {
                emit(app, s, ProvisionPhase::Failed, None, None, Some(e.clone()));
                return Err(e);
            }
            dest
        }
    };

    emit(app, s, ProvisionPhase::Verifying, None, None, None);
    if let Err(e) = download::verify_sha256(&archive_path, s.sha256).await {
        emit(app, s, ProvisionPhase::Failed, None, None, Some(e.clone()));
        return Err(e);
    }

    let result = match s.kind {
        PayloadKind::SevenZipArchive | PayloadKind::ZipArchive => {
            emit(app, s, ProvisionPhase::Extracting, None, None, None);
            let dest_dir = engines_root(app)?.join(s.id);
            let seven_zip = seven_zip_exe_path(app)?;
            extract::extract_with_7z(&seven_zip, &archive_path, &dest_dir).await
        }
        PayloadKind::MsiInstaller => {
            emit(app, s, ProvisionPhase::Installing, None, None, None);
            install::run_msi_silent(&archive_path).await
        }
        PayloadKind::InnoInstaller => {
            emit(app, s, ProvisionPhase::Installing, None, None, None);
            install::run_inno_silent(&archive_path).await
        }
        PayloadKind::LooseBinary => unreachable!(),
    };

    if let Err(e) = result {
        emit(app, s, ProvisionPhase::Failed, None, None, Some(e.clone()));
        return Err(e);
    }

    if already_ready(app, s) {
        emit(app, s, ProvisionPhase::Ready, None, None, None);
        Ok(())
    } else {
        let msg = format!("{} finished but Nexara still couldn't find it afterward.", s.display_name);
        emit(app, s, ProvisionPhase::Failed, None, None, Some(msg.clone()));
        Err(msg)
    }
}

fn already_ready<R: Runtime>(app: &AppHandle<R>, s: &EngineSpec) -> bool {
    match s.kind {
        PayloadKind::LooseBinary => seven_zip_exe_path(app).map(|p| p.is_file()).unwrap_or(false),
        PayloadKind::SevenZipArchive | PayloadKind::ZipArchive => resolved_exe_path(app, s.id).is_some(),
        // Installer-based engines resolve through conversion::engine's own
        // probing — checked by the caller via crate::conversion::engine.
        PayloadKind::MsiInstaller | PayloadKind::InnoInstaller => crate::conversion::engine::is_installed(s.id),
    }
}

/// Runs every bundled engine first (fast, local, no network), then every
/// download-tier engine, emitting progress events throughout. Returns the
/// ids of any engine that failed to provision — an empty vec means every
/// engine is ready.
pub async fn ensure_all<R: Runtime>(app: &AppHandle<R>) -> Vec<(String, String)> {
    let mut failures = Vec::new();
    for s in BUNDLED.iter().chain(DOWNLOADED.iter()) {
        if let Err(e) = provision_one(app, s).await {
            failures.push((s.id.to_string(), e));
        }
    }
    failures
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineReadiness {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub license: String,
    pub ready: bool,
}

/// A point-in-time snapshot of every engine's readiness, for the
/// post-install/first-run health check screen and the Conversion Engines
/// settings screen alike.
pub fn readiness_snapshot<R: Runtime>(app: &AppHandle<R>) -> Vec<EngineReadiness> {
    BUNDLED
        .iter()
        .chain(DOWNLOADED.iter())
        .map(|s| EngineReadiness {
            id: s.id.to_string(),
            display_name: s.display_name.to_string(),
            version: s.version.to_string(),
            license: s.license.to_string(),
            ready: already_ready(app, s),
        })
        .collect()
}
