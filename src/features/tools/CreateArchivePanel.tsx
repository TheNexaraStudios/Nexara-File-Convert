import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { X, FolderArchive, Loader2, FolderOpen, ExternalLink } from "lucide-react";
import { ToolModal, Field } from "./ToolModal";
import { Segmented } from "../../components/common/Segmented";
import { pathToFileName, dirname, makeId } from "../../utils/format";
import type { ToolOutcome } from "../../types/tools";
import { useToastStore } from "../../stores/useToastStore";

type Format = "zip" | "7z" | "tar" | "gz";

const PASSWORD_CAPABLE: Format[] = ["zip", "7z"];

export function CreateArchivePanel({ onClose }: { onClose: () => void }) {
  const push = useToastStore((s) => s.push);
  const [paths, setPaths] = useState<string[]>([]);
  const [format, setFormat] = useState<Format>("zip");
  const [compressionLevel, setCompressionLevel] = useState(6);
  const [password, setPassword] = useState("");
  const [outputName, setOutputName] = useState("archive");
  const [outputDir, setOutputDir] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [jobId, setJobId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);

  const addPaths = (newPaths: string[]) => {
    setPaths((prev) => [...prev, ...newPaths.filter((p) => !prev.includes(p))]);
    if (!outputDir && newPaths.length > 0) setOutputDir(dirname(newPaths[0]));
  };

  const addFiles = async () => {
    const selection = await open({ multiple: true });
    if (!selection) return;
    addPaths(Array.isArray(selection) ? selection : [selection]);
  };

  const addFolder = async () => {
    const selection = await open({ multiple: true, directory: true });
    if (!selection) return;
    addPaths(Array.isArray(selection) ? selection : [selection]);
  };

  const remove = (path: string) => setPaths((prev) => prev.filter((p) => p !== path));

  const runCreate = async () => {
    if (paths.length === 0 || !outputDir) return;
    setRunning(true);
    setError(null);
    const id = makeId();
    setJobId(id);
    try {
      const outcome = await invoke<ToolOutcome>("create_archive", {
        req: {
          jobId: id,
          inputPaths: paths,
          outputDir,
          outputName: outputName.trim() || "archive",
          format,
          compressionLevel,
          password: PASSWORD_CAPABLE.includes(format) && password ? password : null,
        },
      });
      if (outcome.outcome === "completed") {
        setResult(outcome.outputPaths[0] ?? null);
        push("Archive created", "success");
      } else if (outcome.outcome === "cancelled") {
        push("Archive creation cancelled", "info");
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

  const canRun = paths.length > 0 && !!outputDir;

  if (result) {
    return (
      <ToolModal title="Create Archive" onClose={onClose} footer={<button className="tool-modal__secondary-btn" onClick={onClose}>Done</button>}>
        <div className="tool-modal__result">
          <div className="tool-modal__result-title">Archive created</div>
          <div className="tool-modal__file-row">
            <span className="tool-modal__file-row-name">{pathToFileName(result)}</span>
            <button className="tool-modal__file-row-action" onClick={() => openPath(result)} title="Open file">
              <ExternalLink size={13} />
            </button>
            <button className="tool-modal__file-row-action" onClick={() => revealItemInDir(result)} title="Show in folder">
              <FolderOpen size={13} />
            </button>
          </div>
        </div>
      </ToolModal>
    );
  }

  return (
    <ToolModal
      title="Create Archive"
      subtitle="Compress files and folders into a new ZIP, 7Z, TAR, or GZ archive."
      onClose={onClose}
      footer={
        running ? (
          <button className="tool-modal__secondary-btn" onClick={cancel}>Cancel</button>
        ) : (
          <>
            <button className="tool-modal__secondary-btn" onClick={onClose}>Cancel</button>
            <button className="tool-modal__primary-btn" disabled={!canRun} onClick={runCreate}>
              <FolderArchive size={14} /> Create Archive
            </button>
          </>
        )
      }
    >
      <Field label={`Files & folders (${paths.length})`}>
        <div className="tool-modal__file-list">
          {paths.map((p) => (
            <div key={p} className="tool-modal__file-row">
              <span className="tool-modal__file-row-name" title={p}>{pathToFileName(p)}</span>
              <button className="tool-modal__file-row-action" disabled={running} onClick={() => remove(p)} title="Remove">
                <X size={13} />
              </button>
            </div>
          ))}
          {paths.length === 0 && <p className="tool-modal__note">Nothing selected yet.</p>}
        </div>
        <div className="tool-modal__row" style={{ marginTop: 8 }}>
          <button className="tool-modal__secondary-btn" disabled={running} onClick={addFiles}>
            Add files…
          </button>
          <button className="tool-modal__secondary-btn" disabled={running} onClick={addFolder}>
            Add folder…
          </button>
        </div>
      </Field>

      <div className="tool-modal__row">
        <Field label="Format">
          <Segmented
            value={format}
            options={[
              { value: "zip", label: "ZIP" },
              { value: "7z", label: "7Z" },
              { value: "tar", label: "TAR" },
              { value: "gz", label: "TAR.GZ" },
            ]}
            onChange={setFormat}
          />
        </Field>
        <Field label={`Compression — ${compressionLevel === 0 ? "Store" : compressionLevel}`}>
          <input
            type="range"
            min={0}
            max={9}
            value={compressionLevel}
            onChange={(e) => setCompressionLevel(Number(e.target.value))}
            disabled={running || format === "tar"}
          />
        </Field>
      </div>

      {PASSWORD_CAPABLE.includes(format) && (
        <Field label="Password (optional — AES-256 encryption)">
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="Leave blank for no encryption"
            disabled={running}
          />
        </Field>
      )}

      <Field label="Output name">
        <div className="tool-modal__row">
          <input type="text" value={outputName} onChange={(e) => setOutputName(e.target.value)} disabled={running} />
          <span style={{ alignSelf: "center", color: "var(--text-tertiary)", fontSize: 13 }}>.{format}</span>
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
          Creating archive…
        </p>
      )}
      {error && <p className="tool-modal__error">{error}</p>}
    </ToolModal>
  );
}
