import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { Copy, Loader2, Info, Lock } from "lucide-react";
import { ToolModal, Field } from "./ToolModal";
import { pathToFileName, formatBytes } from "../../utils/format";
import type { MetadataInfo } from "../../types/tools";
import { useToastStore } from "../../stores/useToastStore";

function formatDate(ms: number | null): string {
  if (ms == null) return "—";
  return new Date(ms).toLocaleString();
}

function formatDuration(seconds: number | null): string {
  if (seconds == null) return "—";
  const total = Math.round(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => n.toString().padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

/** Flattens a MetadataInfo into plain label/value pairs — both for the
 * on-screen grid and for the "Copy Metadata" clipboard text, so the two
 * never drift apart. */
function toRows(info: MetadataInfo): { label: string; value: string }[] {
  const rows: { label: string; value: string }[] = [
    { label: "File name", value: info.basic.fileName },
    { label: "File size", value: formatBytes(info.basic.sizeBytes) },
    { label: "Modified", value: formatDate(info.basic.modifiedAtMs) },
  ];

  if (info.kind === "media") {
    if (info.container) rows.push({ label: "Container", value: info.container });
    if (info.durationSeconds != null) rows.push({ label: "Duration", value: formatDuration(info.durationSeconds) });
    if (info.bitRate != null) rows.push({ label: "Bit rate", value: `${Math.round(info.bitRate / 1000)} kbps` });
    if (info.video) {
      rows.push({ label: "Video codec", value: info.video.codec ?? "—" });
      if (info.video.width && info.video.height) rows.push({ label: "Resolution", value: `${info.video.width}×${info.video.height}` });
      if (info.video.frameRate) rows.push({ label: "Frame rate", value: info.video.frameRate });
    }
    if (info.audio) {
      rows.push({ label: "Audio codec", value: info.audio.codec ?? "—" });
      if (info.audio.sampleRate) rows.push({ label: "Sample rate", value: `${info.audio.sampleRate} Hz` });
      if (info.audio.channels != null) rows.push({ label: "Channels", value: String(info.audio.channels) });
    }
    for (const tag of info.tags) rows.push({ label: tag.key, value: tag.value });
  } else if (info.kind === "image") {
    if (info.width && info.height) rows.push({ label: "Dimensions", value: `${info.width}×${info.height}` });
    if (info.format) rows.push({ label: "Format", value: info.format });
    if (info.colorspace) rows.push({ label: "Colorspace", value: info.colorspace });
    if (info.bitDepth) rows.push({ label: "Bit depth", value: info.bitDepth });
    if (info.hasAlpha != null) rows.push({ label: "Transparency", value: info.hasAlpha ? "Yes" : "No" });
  } else if (info.kind === "pdf") {
    if (info.encrypted) {
      rows.push({ label: "Encrypted", value: "Yes — password required" });
    } else {
      if (info.pageCount != null) rows.push({ label: "Pages", value: String(info.pageCount) });
      if (info.pageWidth != null && info.pageHeight != null) {
        rows.push({ label: "Page size", value: `${info.pageWidth.toFixed(0)} × ${info.pageHeight.toFixed(0)} pt` });
      }
    }
  } else if (info.kind === "archive") {
    if (info.encrypted) {
      rows.push({ label: "Encrypted", value: "Yes — password required" });
    } else {
      if (info.entryCount != null) rows.push({ label: "Entries", value: String(info.entryCount) });
      if (info.uncompressedSize != null) rows.push({ label: "Uncompressed size", value: formatBytes(info.uncompressedSize) });
      if (info.method) rows.push({ label: "Compression method", value: info.method });
    }
  }

  return rows;
}

export function MetadataInspectorPanel({ onClose }: { onClose: () => void }) {
  const push = useToastStore((s) => s.push);
  const [path, setPath] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<MetadataInfo | null>(null);

  const pickFile = async () => {
    const picked = await open({ multiple: false });
    if (typeof picked !== "string") return;
    setPath(picked);
    setInfo(null);
    setError(null);
    setLoading(true);
    try {
      const result = await invoke<MetadataInfo>("inspect_metadata", { path: picked });
      setInfo(result);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const copyMetadata = () => {
    if (!info) return;
    const text = toRows(info).map((r) => `${r.label}: ${r.value}`).join("\n");
    navigator.clipboard.writeText(text).then(
      () => push("Metadata copied", "success"),
      () => push("Couldn't copy to clipboard", "warning")
    );
  };

  return (
    <ToolModal
      title="Metadata Inspector"
      subtitle="Read-only — nothing about the file is ever changed."
      onClose={onClose}
      footer={
        <>
          <button className="tool-modal__secondary-btn" onClick={onClose}>Close</button>
          {info && (
            <button className="tool-modal__primary-btn" onClick={copyMetadata}>
              <Copy size={14} /> Copy Metadata
            </button>
          )}
        </>
      }
    >
      <Field label="File">
        <div className="tool-modal__row">
          <input type="text" value={path ? pathToFileName(path) : ""} readOnly placeholder="Choose a file to inspect" />
          <button className="tool-modal__secondary-btn" onClick={pickFile}>
            Browse…
          </button>
        </div>
      </Field>

      {loading && (
        <p className="tool-modal__note">
          <Loader2 size={13} className="anim-spin" style={{ verticalAlign: "middle", marginRight: 6 }} />
          Reading metadata…
        </p>
      )}
      {error && <p className="tool-modal__error">{error}</p>}

      {info && (info.kind === "pdf" || info.kind === "archive") && info.encrypted && (
        <p className="tool-modal__note">
          <Lock size={13} style={{ verticalAlign: "middle", marginRight: 6 }} />
          This file is password-protected — only basic file info is available without the password.
        </p>
      )}

      {info && (
        <div>
          <div className="tool-modal__section-title">
            {info.kind === "media" && "Media Info"}
            {info.kind === "image" && "Image Info"}
            {info.kind === "pdf" && "PDF Info"}
            {info.kind === "archive" && "Archive Info"}
            {info.kind === "basic" && "File Info"}
          </div>
          <dl className="tool-modal__info-grid">
            {toRows(info).map((row) => (
              <FragmentRow key={row.label} label={row.label} value={row.value} />
            ))}
          </dl>
          {info.kind === "basic" && (
            <p className="tool-modal__note" style={{ marginTop: 10 }}>
              <Info size={13} style={{ verticalAlign: "middle", marginRight: 6 }} />
              Nexara doesn't have a dedicated inspector for this file type yet — showing basic file info only.
            </p>
          )}
        </div>
      )}
    </ToolModal>
  );
}

function FragmentRow({ label, value }: { label: string; value: string }) {
  return (
    <>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </>
  );
}
