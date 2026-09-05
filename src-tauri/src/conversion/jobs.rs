use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::Mutex;

/// Tracks the OS process backing each in-flight conversion job so a
/// `cancel_conversion` call can terminate exactly that job's child process
/// and nothing else.
#[derive(Default)]
pub struct JobRegistry(pub Mutex<HashMap<String, Arc<Mutex<Child>>>>);

/// Terminates the process registered for `job_id`, if any. Returns `true`
/// if a running job was found and signalled to stop. The job's own
/// `execute` call detects the resulting removal from the registry and
/// reports the outcome as cancelled rather than failed.
pub async fn cancel(registry: &JobRegistry, job_id: &str) -> bool {
    let child_arc = registry.0.lock().await.remove(job_id);
    match child_arc {
        Some(child_arc) => {
            let mut child = child_arc.lock().await;
            // A plain kill only terminates the process we directly spawned.
            // That's fine for ffmpeg/magick (they do the work themselves),
            // but LibreOffice's `soffice.com` launcher spawns a separate
            // `soffice.bin` backend as a *child* process — killing just the
            // launcher would leave that backend running, still holding the
            // profile lock, poisoning every conversion after it. On
            // Windows, kill the whole process tree instead.
            #[cfg(target_os = "windows")]
            if let Some(pid) = child.id() {
                let _ = tokio::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).output().await;
            }
            let _ = child.start_kill();
            true
        }
        None => false,
    }
}
