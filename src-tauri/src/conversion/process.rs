use std::process::Stdio;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

use super::jobs::JobRegistry;

pub struct ExecuteOutcome {
    pub success: bool,
    pub cancelled: bool,
    pub stderr_tail: String,
}

/// Spawns `program` with `args`, tracking the child in the shared job
/// registry so `cancel_conversion` can terminate it. Used by engines (image,
/// office) that run as a single batch process with no incremental progress
/// to stream — ffmpeg has its own richer variant that parses `-progress`
/// output as it goes.
pub async fn run_and_track(registry: &JobRegistry, job_id: &str, program: &str, args: &[String]) -> Result<ExecuteOutcome, String> {
    let child = tokio::process::Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start {program}: {e}"))?;

    let child_arc = Arc::new(TokioMutex::new(child));
    registry.0.lock().await.insert(job_id.to_string(), child_arc.clone());

    let (status, stderr_bytes) = {
        let (stdout, stderr) = {
            let mut guard = child_arc.lock().await;
            (guard.stdout.take(), guard.stderr.take())
        };

        let stdout_task: tokio::task::JoinHandle<()> = tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            if let Some(mut s) = stdout {
                let mut sink = Vec::new();
                let _ = s.read_to_end(&mut sink).await;
            }
        });
        let stderr_task: tokio::task::JoinHandle<Vec<u8>> = tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = Vec::new();
            if let Some(mut s) = stderr {
                let _ = s.read_to_end(&mut buf).await;
            }
            buf
        });

        let _ = stdout_task.await;
        let stderr_bytes = stderr_task.await.unwrap_or_default();

        let status = {
            let mut guard = child_arc.lock().await;
            guard.wait().await
        };

        (status, stderr_bytes)
    };

    let stderr_tail = String::from_utf8_lossy(&stderr_bytes).to_string();

    let still_registered = registry.0.lock().await.remove(job_id).is_some();
    let cancelled = !still_registered;

    match status {
        Ok(exit_status) => Ok(ExecuteOutcome { success: exit_status.success() && !cancelled, cancelled, stderr_tail }),
        Err(e) => Ok(ExecuteOutcome { success: false, cancelled, stderr_tail: format!("{stderr_tail}\n{e}") }),
    }
}
