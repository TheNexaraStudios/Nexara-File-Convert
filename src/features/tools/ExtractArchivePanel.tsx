import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { FolderOpen, Loader2, PackageOpen } from "lucide-react";
import { ToolModal, Field } from "./ToolModal";
import { pathToFileName, makeId } from "../../utils/format";
import type { ToolOutcome } from "../../types/tools";
import { useToastStore } from "../../stores/useToastStore";

export function ExtractArchivePanel({ onClose }: { onClose: () => void }) {
  const push = useToastStore((s) => s.push);
  const [inputPath, setInputPath] = useState<string | null>(null);
  const [destDir, setDestDir] = useState<string | null>(null);
  const [needsPassword, setNeedsPassword] = useState(false);
  const [password, setPassword] = useState("");
  const [checking, setChecking] = useState(false);
  const [running, setRunning] = useState(false);
  const [jobId, setJobId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);

  const checkArchive = async (path: string, pw?: string) => {
    setChecking(true);
    setError(null);
    try {
      await invoke("preview_archive", { path, password: pw || null });
      setNeedsPassword(false);
      return true;
    } catch (err) {
      const message = String(err);
      if (message.toLowerCase().includes("password")) {
        setNeedsPassword(true);
      } else {
        setError(message);
      }
      return false;
    } finally {
      setChecking(false);
    }
  };

  const pickFile = async () => {
    const picked = await open({ multiple: false, filters: [{ name: "Archives", extensions: ["zip", "7z", "tar", "gz", "tgz", "rar"] }] });
    if (typeof picked !== "string") return;
    setInputPath(picked);
    setPassword("");
    setNeedsPassword(false);
    setError(null);
    setDone(null);
    await checkArchive(picked);
  };

  const runExtract = async () => {
    if (!inputPath || !destDir) return;
    setRunning(true);
    setError(null);
    const id = makeId();
    setJobId(id);
    try {
      const outcome = await invoke<ToolOutcome>("extract_archive", {
        req: { jobId: id, inputPath, destDir, password: needsPassword ? password : null },
      });
      if (outcome.outcome === "completed") {
        setDone(destDir);
        push("Archive extracted", "success");
      } else if (outcome.outcome === "cancelled") {
        push("Extraction cancelled", "info");
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

  const canRun = !!inputPath && !!destDir && !checking && (!needsPassword || password.length > 0);

  if (done) {
    return (
      <ToolModal title="Extract Archive" onClose={onClose} footer={<button className="tool-modal__secondary-btn" onClick={onClose}>Done</button>}>
        <div className="tool-modal__result">
          <div className="tool-modal__result-title">Extracted to {pathToFileName(done)}</div>
          <button className="tool-modal__secondary-btn" onClick={() => revealItemInDir(done)} style={{ alignSelf: "flex-start" }}>
            <FolderOpen size={14} style={{ verticalAlign: "middle", marginRight: 6 }} />
            Show in folder
          </button>
        </div>
      </ToolModal>
    );
  }

  return (
    <ToolModal
      title="Extract Archive"
      subtitle="Unpack ZIP, 7Z, TAR, GZ, or RAR — with the same path-safety checks used everywhere else in Nexara."
      onClose={onClose}
      footer={
        running ? (
          <button className="tool-modal__secondary-btn" onClick={cancel}>Cancel</button>
        ) : (
          <>
            <button className="tool-modal__secondary-btn" onClick={onClose}>Cancel</button>
            <button className="tool-modal__primary-btn" disabled={!canRun} onClick={runExtract}>
              <PackageOpen size={14} /> Extract
            </button>
          </>
        )
      }
    >
      <Field label="Archive to extract">
        <div className="tool-modal__row">
          <input type="text" value={inputPath ? pathToFileName(inputPath) : ""} readOnly placeholder="Choose an archive" />
          <button className="tool-modal__secondary-btn" disabled={running} onClick={pickFile}>
            Browse…
          </button>
        </div>
        {checking && (
          <p className="tool-modal__note">
            <Loader2 size={13} className="anim-spin" style={{ verticalAlign: "middle", marginRight: 6 }} />
            Checking archive…
          </p>
        )}
      </Field>

      {needsPassword && (
        <Field label="Password">
          <div className="tool-modal__row">
            <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder="Archive password" disabled={running} />
            <button
              className="tool-modal__secondary-btn"
              disabled={running || checking || !password}
              onClick={() => inputPath && checkArchive(inputPath, password)}
            >
              Check
            </button>
          </div>
          <p className="tool-modal__note">This archive is password-protected.</p>
        </Field>
      )}

      <Field label="Extract to">
        <div className="tool-modal__row">
          <input type="text" value={destDir ?? ""} readOnly placeholder="Choose a destination folder" />
          <button
            className="tool-modal__secondary-btn"
            disabled={running}
            onClick={async () => {
              const dir = await open({ directory: true });
              if (typeof dir === "string") setDestDir(dir);
            }}
          >
            Browse…
          </button>
        </div>
      </Field>

      {running && (
        <p className="tool-modal__note">
          <Loader2 size={13} className="anim-spin" style={{ verticalAlign: "middle", marginRight: 6 }} />
          Extracting…
        </p>
      )}
      {error && <p className="tool-modal__error">{error}</p>}
    </ToolModal>
  );
}
