export type JobStatus =
  | "queued"
  | "analyzing"
  | "preparing"
  | "converting"
  | "finalizing"
  | "completed"
  | "failed"
  | "cancelled";

export interface SourceFile {
  id: string;
  path: string;
  name: string;
  extension: string;
  sizeBytes: number;
  /** Format id from the registry, or null if the extension isn't recognized. */
  detectedFormatId: string | null;
}

export type QualityPreset = "high" | "balanced" | "small" | "custom";

export type ResizeMode = "fit" | "fill" | "stretch";

export interface ConversionSettings {
  preset: QualityPreset;
  resolution?: string;
  frameRate?: string;
  videoCodec?: string;
  audioBitrate?: string;
  imageQuality?: number;
  stripMetadata?: boolean;
  /** Explicit resize controls (Resize Image tool + any image conversion's
   * settings panel) — distinct from `resolution`'s fixed video presets. */
  resizeWidth?: number;
  resizeHeight?: number;
  resizePercent?: number;
  resizeMode?: ResizeMode;
}

export interface ConversionJob {
  id: string;
  file: SourceFile;
  outputFormatId: string;
  settings: ConversionSettings;
  status: JobStatus;
  /** 0-100, or null when the engine can't report a percentage (indeterminate). */
  progress: number | null;
  error?: {
    message: string;
    technical?: string;
  };
  outputPath?: string;
  outputSizeBytes?: number;
  remuxed?: boolean;
  createdAt: number;
  completedAt?: number;
}

export const DEFAULT_SETTINGS: ConversionSettings = {
  preset: "balanced",
};
