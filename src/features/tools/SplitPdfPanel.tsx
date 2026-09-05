import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { Scissors, Loader2, FolderOpen, ExternalLink } from "lucide-react";
import { ToolModal, Field } from "./ToolModal";
import { Segmented } from "../../components/common/Segmented";
import { pathToFileName, dirname, fileNameParts, makeId } from "../../utils/format";
import type { ToolOutcome } from "../../types/tools";
import { useToastStore } from "../../stores/useToastStore";

type Mode = "range" | "eachPage";

export function SplitPdfPanel({ onClose }: { onClose: () => void }) {
  const push = useToastStore((s) => s.push);
  const [inputPath, setInputPath] = useState<string | null>(null);
  const [pageCount, setPageCount] = useState<number | null>(null);
  const [pageCountError, setPageCountError] = useState<string | null>(null);
  const [mode, setMode] = useState<Mode>("range");
  const [rangeSpec, setRangeSpec] = useState("");
  const [outputName, setOutputName] = useState("extract");
  const [baseName, setBaseName] = useState("document");
  const [outputDir, setOutputDir] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [jobId, setJobId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string[] | null>(null);

  const pickFile = async () => {
    const selection = await open({ multiple: false, filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (typeof selection !== "string") return;
    setInputPath(selection);
    setOutputDir(dirname(selection));
    setBaseName(fileNameParts(pathToFileName(selection)).base);
    setPageCount(null);
    setPageCountError(null);
    try {
      const count = await invoke<number>("get_pdf_page_count", { path: selection });
      setPageCount(count);
    } catch (err) {
      setPageCountError(String(err));
    }
  };

  const runSplit = async () => {
    if (!inputPath || !outputDir) return;
    setRunning(true);
    setError(null);
    const id = makeId();
    setJobId(id);
    try {
      const req =
        mode === "range"
          ? { jobId: id, inputPath, outputDir, mode: "range", pages: rangeSpec.trim(), outputName: outputName.trim() || "extract" }
          : { jobId: id, inputPath, outputDir, mode: "eachPage", baseName: baseName.trim() || "document" };
      const outcome = await invoke<ToolOutcome>("split_pdf", { req });
      if (outcome.outcome === "completed") {
        setResult(outcome.outputPaths);
        push(mode === "range" ? "PDF split" : `${outcome.outputPaths.length} pages exported`, "success");
      } else if (outcome.outcome === "cancelled") {
        push("Split cancelled", "info");
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

  const canRun = !!inputPath && !!outputDir && (mode === "eachPage" || rangeSpec.trim().length > 0);

  if (result) {
    return (
      <ToolModal title="Split PDF" onClose={onClose} footer={<button className="tool-modal__secondary-btn" onClick={onClose}>Done</button>}>
        <div className="tool-modal__result">
          <div className="tool-modal__result-title">
            {result.length === 1 ? "Created 1 file" : `Created ${result.length} files`}
          </div>
          <div className="tool-modal__file-list">
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
        </div>
      </ToolModal>
    );
  }

  return (
    <ToolModal
      title="Split PDF"
      subtitle="Extract a page range into a new PDF, or export every page separately."
      onClose={onClose}
      footer={
        running ? (
          <button className="tool-modal__secondary-btn" onClick={cancel}>Cancel</button>
        ) : (
          <>
            <button className="tool-modal__secondary-btn" onClick={onClose}>Cancel</button>
            <button className="tool-modal__primary-btn" disabled={!canRun} onClick={runSplit}>
              <Scissors size={14} /> Split
            </button>
          </>
        )
      }
    >
      <Field label="PDF to split">
        <div className="tool-modal__row">
          <input type="text" value={inputPath ? pathToFileName(inputPath) : ""} readOnly placeholder="Choose a PDF" />
          <button className="tool-modal__secondary-btn" disabled={running} onClick={pickFile}>
            Browse…
          </button>
        </div>
        {pageCount != null && <p className="tool-modal__note">{pageCount} page{pageCount === 1 ? "" : "s"} in this document.</p>}
        {pageCountError && <p className="tool-modal__error">{pageCountError}</p>}
      </Field>

      <Field label="Mode">
        <Segmented
          value={mode}
          options={[
            { value: "range", label: "Extract page range" },
            { value: "eachPage", label: "Every page separately" },
          ]}
          onChange={setMode}
        />
      </Field>

      {mode === "range" ? (
        <>
          <Field label="Pages (e.g. 1-3,5,8-10)">
            <input type="text" value={rangeSpec} onChange={(e) => setRangeSpec(e.target.value)} placeholder="1-3,5" disabled={running} />
          </Field>
          <Field label="Output name">
            <div className="tool-modal__row">
              <input type="text" value={outputName} onChange={(e) => setOutputName(e.target.value)} disabled={running} />
              <span style={{ alignSelf: "center", color: "var(--text-tertiary)", fontSize: 13 }}>.pdf</span>
            </div>
          </Field>
        </>
      ) : (
        <Field label="File name prefix">
          <input type="text" value={baseName} onChange={(e) => setBaseName(e.target.value)} disabled={running} />
          <p className="tool-modal__note">
            Produces {baseName || "document"}-page-001.pdf, {baseName || "document"}-page-002.pdf, and so on for every page.
          </p>
        </Field>
      )}

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
          Splitting…
        </p>
      )}
      {error && <p className="tool-modal__error">{error}</p>}
    </ToolModal>
  );
}
