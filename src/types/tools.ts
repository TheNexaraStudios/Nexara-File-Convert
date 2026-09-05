export type ToolOutcome =
  | { outcome: "completed"; outputPaths: string[] }
  | { outcome: "cancelled" }
  | { outcome: "failed"; message: string; technical: string };

export interface BasicInfo {
  fileName: string;
  sizeBytes: number;
  modifiedAtMs: number | null;
}

export interface StreamInfo {
  codec: string | null;
  width: number | null;
  height: number | null;
  frameRate: string | null;
  sampleRate: string | null;
  channels: number | null;
}

export interface MetaTag {
  key: string;
  value: string;
}

export type MetadataInfo =
  | {
      kind: "media";
      basic: BasicInfo;
      container: string | null;
      durationSeconds: number | null;
      bitRate: number | null;
      video: StreamInfo | null;
      audio: StreamInfo | null;
      tags: MetaTag[];
    }
  | {
      kind: "image";
      basic: BasicInfo;
      width: number | null;
      height: number | null;
      format: string | null;
      colorspace: string | null;
      bitDepth: string | null;
      hasAlpha: boolean | null;
    }
  | {
      kind: "pdf";
      basic: BasicInfo;
      pageCount: number | null;
      pageWidth: number | null;
      pageHeight: number | null;
      encrypted: boolean;
    }
  | {
      kind: "archive";
      basic: BasicInfo;
      entryCount: number | null;
      uncompressedSize: number | null;
      method: string | null;
      encrypted: boolean;
    }
  | { kind: "basic"; basic: BasicInfo };

export type ResizeMode = "fit" | "fill" | "stretch";
