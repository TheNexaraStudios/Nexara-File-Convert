import { create } from "zustand";
import { persist } from "zustand/middleware";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { SourceFile, ConversionJob, ConversionSettings, JobStatus } from "../types/job";
import { DEFAULT_SETTINGS } from "../types/job";
import type { MediaProbe } from "../types/media";
import { makeId, fileNameParts, pathToFileName, dirname } from "../utils/format";
import { useRegistryStore } from "./useRegistryStore";
import { useSettingsStore } from "./useSettingsStore";

const TERMINAL_STATUSES = new Set<JobStatus>(["completed", "failed", "cancelled"]);

/** Keeps the persisted history from growing without bound over long-term use. */
const MAX_PERSISTED_JOBS = 300;

export interface PendingFile {
  file: SourceFile;
  outputFormatId: string | null;
  settings: ConversionSettings;
  probe?: MediaProbe;
}

interface RawDroppedFile {
  path: string;
  name?: string;
  sizeBytes?: number;
}

interface ConversionOutcome {
  outcome: "completed" | "cancelled" | "failed";
  outputPath?: string;
  outputSizeBytes?: number;
  remuxed?: boolean;
  message?: string;
  technical?: string;
}

/** Optional hint from a Tools-screen shortcut: prefer this output format (if
 * it's actually a valid target for the file's detected type — otherwise the
 * usual first-compatible-format default is used) and start new files from
 * these settings instead of the plain defaults. */
export interface AddFilesHint {
  preferredFormatId?: string;
  preferredSettings?: Partial<ConversionSettings>;
}

interface JobState {
  pending: PendingFile[];
  jobs: ConversionJob[];
  addFiles: (files: RawDroppedFile[], hint?: AddFilesHint) => void;
  removeFile: (fileId: string) => void;
  clearPending: () => void;
  setOutputFormat: (fileId: string, formatId: string) => void;
  setSettings: (fileId: string, settings: ConversionSettings) => void;
  startJob: (fileId: string) => Promise<void>;
  startAll: () => Promise<void>;
  cancelJob: (jobId: string) => void;
  removeJob: (jobId: string) => void;
  clearCompleted: () => void;
  clearHistory: () => void;
  setProgress: (jobId: string, percent: number | null) => void;
}

async function resolveOutputDir(sourcePath: string): Promise<string | null> {
  const settings = useSettingsStore.getState();
  if (settings.outputLocationMode === "same-folder") {
    return dirname(sourcePath);
  }
  if (settings.outputLocationMode === "custom" && settings.customOutputDir) {
    return settings.customOutputDir;
  }
  const chosen = await open({ directory: true, multiple: false, title: "Choose where to save the converted file" });
  return typeof chosen === "string" ? chosen : null;
}

export const useJobStore = create<JobState>()(
  persist(
    (set, get) => ({
      pending: [],
      jobs: [],

  addFiles: (files, hint) => {
    const registry = useRegistryStore.getState();
    const additions: PendingFile[] = files.map((raw) => {
      const name = raw.name ?? pathToFileName(raw.path);
      const { extension } = fileNameParts(name);
      const detected = registry.findByExtension(extension);
      const outputs = registry.compatibleOutputs(detected?.id ?? null);
      const source: SourceFile = {
        id: makeId(),
        path: raw.path,
        name,
        extension,
        sizeBytes: raw.sizeBytes ?? 0,
        detectedFormatId: detected?.id ?? null,
      };
      const preferred = hint?.preferredFormatId;
      const outputFormatId = preferred && outputs.some((o) => o.id === preferred) ? preferred : outputs[0]?.id ?? null;
      return {
        file: source,
        outputFormatId,
        settings: { ...DEFAULT_SETTINGS, ...hint?.preferredSettings },
      };
    });
    set((s) => ({ pending: [...s.pending, ...additions] }));

    for (const addition of additions) {
      const format = registry.formatsById[addition.file.detectedFormatId ?? ""];
      if (format?.engine !== "ffmpeg" && format?.engine !== "image") continue;
      invoke<MediaProbe>("probe_media", { path: addition.file.path })
        .then((probe) => {
          set((s) => ({
            pending: s.pending.map((p) => (p.file.id === addition.file.id ? { ...p, probe } : p)),
          }));
        })
        .catch(() => {
          // Non-fatal: metadata is a nice-to-have, conversion still works without it.
        });
    }
  },

  removeFile: (fileId) => set((s) => ({ pending: s.pending.filter((p) => p.file.id !== fileId) })),

  clearPending: () => set({ pending: [] }),

  setOutputFormat: (fileId, formatId) =>
    set((s) => ({
      pending: s.pending.map((p) => (p.file.id === fileId ? { ...p, outputFormatId: formatId } : p)),
    })),

  setSettings: (fileId, settings) =>
    set((s) => ({
      pending: s.pending.map((p) => (p.file.id === fileId ? { ...p, settings } : p)),
    })),

  setProgress: (jobId, percent) =>
    set((s) => ({
      jobs: s.jobs.map((j) => (j.id === jobId && j.status === "converting" ? { ...j, progress: percent } : j)),
    })),

  startJob: async (fileId) => {
    const pendingFile = get().pending.find((p) => p.file.id === fileId);
    if (!pendingFile || !pendingFile.outputFormatId) return;

    const outputDir = await resolveOutputDir(pendingFile.file.path);
    if (!outputDir) return; // user cancelled the folder picker

    const job: ConversionJob = {
      id: makeId(),
      file: pendingFile.file,
      outputFormatId: pendingFile.outputFormatId,
      settings: pendingFile.settings,
      status: "queued",
      progress: null,
      createdAt: Date.now(),
    };

    set((s) => ({
      jobs: [job, ...s.jobs].slice(0, MAX_PERSISTED_JOBS),
      pending: s.pending.filter((p) => p.file.id !== fileId),
    }));

    set((s) => ({
      jobs: s.jobs.map((j) => (j.id === job.id ? { ...j, status: "converting", progress: null } : j)),
    }));

    try {
      const result = await invoke<ConversionOutcome>("start_conversion", {
        req: {
          jobId: job.id,
          inputPath: job.file.path,
          outputFormat: job.outputFormatId,
          outputDir,
          settings: job.settings,
        },
      });

      if (result.outcome === "completed") {
        set((s) => ({
          jobs: s.jobs.map((j) =>
            j.id === job.id
              ? {
                  ...j,
                  status: "completed",
                  progress: 100,
                  completedAt: Date.now(),
                  outputPath: result.outputPath,
                  outputSizeBytes: result.outputSizeBytes,
                  remuxed: result.remuxed,
                }
              : j
          ),
        }));
      } else if (result.outcome === "cancelled") {
        set((s) => ({
          jobs: s.jobs.map((j) => (j.id === job.id ? { ...j, status: "cancelled", progress: null, completedAt: Date.now() } : j)),
        }));
      } else {
        set((s) => ({
          jobs: s.jobs.map((j) =>
            j.id === job.id
              ? {
                  ...j,
                  status: "failed",
                  progress: null,
                  completedAt: Date.now(),
                  error: { message: result.message ?? "Conversion failed", technical: result.technical },
                }
              : j
          ),
        }));
      }
    } catch (err) {
      set((s) => ({
        jobs: s.jobs.map((j) =>
          j.id === job.id
            ? {
                ...j,
                status: "failed",
                progress: null,
                completedAt: Date.now(),
                error: { message: "Nexara hit an unexpected error", technical: String(err) },
              }
            : j
        ),
      }));
    }
  },

  startAll: async () => {
    const ids = get().pending.filter((p) => p.outputFormatId).map((p) => p.file.id);
    for (const id of ids) {
      await get().startJob(id);
    }
  },

  cancelJob: (jobId) => {
    invoke("cancel_conversion", { jobId }).catch(() => {});
  },

  removeJob: (jobId) => set((s) => ({ jobs: s.jobs.filter((j) => j.id !== jobId) })),

  clearCompleted: () =>
    set((s) => ({
      jobs: s.jobs.filter((j) => j.status !== "completed" && j.status !== "cancelled"),
    })),

  clearHistory: () =>
    set((s) => ({
      jobs: s.jobs.filter((j) => !TERMINAL_STATUSES.has(j.status)),
    })),
    }),
    {
      name: "nexara-job-history",
      // Only completed/failed/cancelled jobs are worth restoring across a
      // restart — `pending` files may no longer exist on disk, and a
      // "queued"/"converting" job's actual process is long gone by the next
      // launch, so persisting those would just leave permanent fake spinners.
      partialize: (state) => ({ jobs: state.jobs.filter((j) => TERMINAL_STATUSES.has(j.status)) }),
    }
  )
);
