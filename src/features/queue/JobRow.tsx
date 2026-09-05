import { Loader2, FolderOpen, ExternalLink, X, Trash2, Zap } from "lucide-react";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import type { ConversionJob } from "../../types/job";
import { useJobStore } from "../../stores/useJobStore";
import { useRegistryStore } from "../../stores/useRegistryStore";
import { relativeTime, formatBytes, formatPercentChange } from "../../utils/format";
import "./JobRow.css";

const STATUS_LABEL: Record<ConversionJob["status"], string> = {
  queued: "Queued",
  analyzing: "Analyzing…",
  preparing: "Preparing…",
  converting: "Converting…",
  finalizing: "Finalizing…",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
};

export function JobRow({ job }: { job: ConversionJob }) {
  const cancelJob = useJobStore((s) => s.cancelJob);
  const removeJob = useJobStore((s) => s.removeJob);
  const formatsById = useRegistryStore((s) => s.formatsById);
  const outputFormat = formatsById[job.outputFormatId];
  const isActive = ["queued", "analyzing", "preparing", "converting", "finalizing"].includes(job.status);

  return (
    <div className={`job-row job-row--${job.status} anim-rise-in`}>
      <div className="job-row__main">
        <div className="job-row__title">
          <span>{job.file.name}</span>
          <span className="job-row__arrow">→</span>
          <span className="job-row__target">{outputFormat?.id.toUpperCase() ?? job.outputFormatId.toUpperCase()}</span>
        </div>
        <div className="job-row__sub">
          <StatusBadge status={job.status} />
          <span className="job-row__time">{relativeTime(job.completedAt ?? job.createdAt)}</span>
        </div>

        {isActive && (
          <div className="job-row__progress-row">
            <div className="job-row__progress">
              <div
                className={`job-row__progress-bar ${job.progress === null ? "job-row__progress-bar--indeterminate" : ""}`}
                style={job.progress !== null ? { width: `${job.progress}%` } : undefined}
              />
            </div>
            {job.progress !== null && <span className="job-row__percent">{Math.round(job.progress)}%</span>}
          </div>
        )}

        {job.status === "completed" && job.outputSizeBytes !== undefined && (
          <div className="job-row__size-compare">
            {job.remuxed && (
              <span className="job-row__remux-tag">
                <Zap size={10} /> Remuxed, no re-encode
              </span>
            )}
            {formatBytes(job.file.sizeBytes)} → {formatBytes(job.outputSizeBytes)}
            {job.file.sizeBytes > 0 && (
              <span className="job-row__size-change"> ({formatPercentChange(job.file.sizeBytes, job.outputSizeBytes)})</span>
            )}
          </div>
        )}

        {job.status === "failed" && job.error && (
          <div className="job-row__error">
            <p>{job.error.message}</p>
            {job.error.technical && (
              <details>
                <summary>Technical details</summary>
                <code>{job.error.technical}</code>
              </details>
            )}
          </div>
        )}
      </div>

      <div className="job-row__actions">
        {isActive && (
          <button className="job-row__action" onClick={() => cancelJob(job.id)} title="Cancel">
            <X size={14} />
          </button>
        )}
        {job.status === "completed" && job.outputPath && (
          <>
            <button className="job-row__action" onClick={() => openPath(job.outputPath!)} title="Open file">
              <ExternalLink size={14} />
            </button>
            <button className="job-row__action" onClick={() => revealItemInDir(job.outputPath!)} title="Show in folder">
              <FolderOpen size={14} />
            </button>
          </>
        )}
        {!isActive && (
          <button className="job-row__action" onClick={() => removeJob(job.id)} title="Remove from queue">
            <Trash2 size={14} />
          </button>
        )}
      </div>
    </div>
  );
}

function StatusBadge({ status }: { status: ConversionJob["status"] }) {
  const isSpinning = ["analyzing", "preparing", "converting", "finalizing"].includes(status);
  return (
    <span className={`status-badge status-badge--${status}`}>
      {isSpinning && <Loader2 size={11} className="anim-spin" />}
      {STATUS_LABEL[status]}
    </span>
  );
}
