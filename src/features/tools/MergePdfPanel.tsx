import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { ArrowUp, ArrowDown, X, FileStack, Loader2, FolderOpen, ExternalLink } from "lucide-react";
import { ToolModal, Field } from "./ToolModal";
import { pathToFileName, dirname, makeId } from "../../utils/format";
import type { ToolOutcome } from "../../types/tools";
import { useToastStore } from "../../stores/useToastStore";

interface PickedFile {
  path: string;
  name: string;
}

export function MergePdfPanel({ onClose }: { onClose: () => void }) {
  const push = useToastStore((s) => s.push);
  const [files, setFiles] = useState<PickedFile[]>([]);
  const [outputName, setOutputName] = useState("merged");
  const [outputDir, setOutputDir] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [jobId, setJobId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string[] | null>(null);

  const addFiles = async () => {
    const selection = await open({ multiple: true, filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (!selection) return;
    const paths = Array.isArray(selection) ? selection : [selection];
    const additions = paths.map((p) => ({ path: p, name: pathToFileName(p) }));
    setFiles((prev) => [...prev, ...additions]);
    if (!outputDir && paths.length > 0) setOutputDir(dirname(paths[0]));
  };

  const move = (index: number, delta: number) => {
    setFiles((prev) => {
      const next = [...prev];
      const target = index + delta;
      if (target < 0 || target >= next.length) return prev;
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
  };

  const remove = (index: number) => setFiles((prev) => prev.filter((_, i) => i !== index));

  const runMerge = async () => {
    if (files.length < 2 || !outputDir) return;
    setRunning(true);
    setError(null);
    const id = makeId();
    setJobId(id);
    try {
      const outcome = await invoke<ToolOutcome>("merge_pdfs", {
        req: { jobId: id, inputPaths: files.map((f) => f.path), outputDir, outputName: outputName.trim() || "merged" },
      });
      if (outcome.outcome === "completed") {
        setResult(outcome.outputPaths);
        push("PDFs merged", "success");
      } else if (outcome.outcome === "cancelled") {
        push("Merge cancelled", "info");
      } else {
        setError(outcome.message);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setRunning(false);
      setJobId(null);
    }
  };

  const cancel = () => {
    if (jobId) invoke("cancel_conversion", { jobId }).catch(() => {});
  };

  if (result) {
    return (
      <ToolModal title="Merge PDF" onClose={onClose} footer={<button className="tool-modal__secondary-btn" onClick={onClose}>Done</button>}>
        <div className="tool-modal__result">
          <div className="tool-modal__result-title">Merged into 1 PDF</div>
          {result.map((p) => (
            <div key={p} className="tool-modal__file-row">
              <span className="tool-modal__file-row-name">{pathToFileName(p)}</span>
              <button className="tool-modal__file-row-action" onClick={() => openPath(p)} title="Open file">
                <ExternalLink size={13} />
              </button>
              <button className="tool-modal__file-row-action" onClick={() => revealItemInDir(p)} title="Show in folder">
                <FolderOpen size={13} />
              </button>
            </div>
          ))}
        </div>
      </ToolModal>
    );
  }

  return (
    <ToolModal
      title="Merge PDF"
      subtitle="Combine multiple PDFs into one document, in the order below."
      onClose={onClose}
      footer={
        running ? (
          <button className="tool-modal__secondary-btn" onClick={cancel}>Cancel</button>
        ) : (
          <>
            <button className="tool-modal__secondary-btn" onClick={onClose}>Cancel</button>
            <button className="tool-modal__primary-btn" disabled={files.length < 2 || !outputDir} onClick={runMerge}>
              <FileStack size={14} /> Merge {files.length > 0 ? `${files.length} PDFs` : ""}
            </button>
          </>
        )
      }
    >
      <Field label={`Files (${files.length})`}>
        <div className="tool-modal__file-list">
          {files.map((f, i) => (
            <div key={`${f.path}-${i}`} className="tool-modal__file-row">
              <span className="tool-modal__file-row-index">{i + 1}</span>
              <span className="tool-modal__file-row-name" title={f.path}>{f.name}</span>
              <button className="tool-modal__file-row-action" disabled={i === 0 || running} onClick={() => move(i, -1)} title="Move up">
                <ArrowUp size={13} />
              </button>
              <button className="tool-modal__file-row-action" disabled={i === files.length - 1 || running} onClick={() => move(i, 1)} title="Move down">
                <ArrowDown size={13} />
              </button>
              <button className="tool-modal__file-row-action" disabled={running} onClick={() => remove(i)} title="Remove">
                <X size={13} />
              </button>
            </div>
          ))}
          {files.length === 0 && <p className="tool-modal__note">No PDFs selected yet.</p>}
        </div>
        <button className="tool-modal__secondary-btn" disabled={running} onClick={addFiles} style={{ marginTop: 8 }}>
          Add PDFs…
        </button>
      </Field>

      <Field label="Output name">
        <div className="tool-modal__row">
          <input type="text" value={outputName} onChange={(e) => setOutputName(e.target.value)} disabled={running} />
          <span style={{ alignSelf: "center", color: "var(--text-tertiary)", fontSize: 13 }}>.pdf</span>
        </div>
      </Field>

      <Field label="Save to">
        <div className="tool-modal__row">
          <input type="text" value={outputDir ?? ""} readOnly placeholder="Choose a destination folder" />
          <button
            className="tool-modal__secondary-btn"
            disabled={running}
            onClick={async () => {
              const dir = await open({ directory: true });
              if (typeof dir === "string") setOutputDir(dir);
            }}
          >
            Browse…
          </button>
        </div>
      </Field>

      {running && (
        <p className="tool-modal__note">
          <Loader2 size={13} className="anim-spin" style={{ verticalAlign: "middle", marginRight: 6 }} />
          Merging…
        </p>
      )}
      {error && <p className="tool-modal__error">{error}</p>}
    </ToolModal>
  );
}
