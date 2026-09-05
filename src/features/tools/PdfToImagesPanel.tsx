import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { Images, Loader2, FolderOpen, ExternalLink } from "lucide-react";
import { ToolModal, Field } from "./ToolModal";
import { Segmented } from "../../components/common/Segmented";
import { pathToFileName, dirname, fileNameParts, makeId } from "../../utils/format";
import type { ToolOutcome } from "../../types/tools";
import { useToastStore } from "../../stores/useToastStore";

type PageSelection = "all" | "range";
type Format = "png" | "jpg" | "webp";

export function PdfToImagesPanel({ onClose }: { onClose: () => void }) {
  const push = useToastStore((s) => s.push);
  const [inputPath, setInputPath] = useState<string | null>(null);
  const [pageCount, setPageCount] = useState<number | null>(null);
  const [pageCountError, setPageCountError] = useState<string | null>(null);
  const [selection, setSelection] = useState<PageSelection>("all");
  const [rangeSpec, setRangeSpec] = useState("");
  const [dpi, setDpi] = useState(150);
  const [format, setFormat] = useState<Format>("png");
  const [baseName, setBaseName] = useState("document");
  const [outputDir, setOutputDir] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [jobId, setJobId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string[] | null>(null);

  const pickFile = async () => {
    const picked = await open({ multiple: false, filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (typeof picked !== "string") return;
    setInputPath(picked);
    setOutputDir(dirname(picked));
    setBaseName(fileNameParts(pathToFileName(picked)).base);
    setPageCount(null);
    setPageCountError(null);
    try {
      setPageCount(await invoke<number>("get_pdf_page_count", { path: picked }));
    } catch (err) {
      setPageCountError(String(err));
    }
  };

  const runExport = async () => {
    if (!inputPath || !outputDir) return;
    setRunning(true);
    setError(null);
    const id = makeId();
    setJobId(id);
    try {
      const outcome = await invoke<ToolOutcome>("export_pdf_pages", {
        req: {
          jobId: id,
          inputPath,
          outputDir,
          pages: selection === "all" ? "all" : rangeSpec.trim(),
          dpi,
          format,
          baseName: baseName.trim() || "document",
        },
      });
      if (outcome.outcome === "completed") {
        setResult(outcome.outputPaths);
        push(`${outcome.outputPaths.length} page${outcome.outputPaths.length === 1 ? "" : "s"} exported`, "success");
      } else if (outcome.outcome === "cancelled") {
        push("Export cancelled", "info");
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

  const canRun = !!inputPath && !!outputDir && (selection === "all" || rangeSpec.trim().length > 0);

  if (result) {
    return (
      <ToolModal title="PDF to Images" onClose={onClose} footer={<button className="tool-modal__secondary-btn" onClick={onClose}>Done</button>}>
        <div className="tool-modal__result">
          <div className="tool-modal__result-title">
            {result.length === 1 ? "Exported 1 image" : `Exported ${result.length} images`}
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
      title="PDF to Images"
      subtitle="Export any page (or every page) as PNG, JPG, or WebP, at any resolution."
      onClose={onClose}
      footer={
        running ? (
          <button className="tool-modal__secondary-btn" onClick={cancel}>Cancel</button>
        ) : (
          <>
            <button className="tool-modal__secondary-btn" onClick={onClose}>Cancel</button>
            <button className="tool-modal__primary-btn" disabled={!canRun} onClick={runExport}>
              <Images size={14} /> Export
            </button>
          </>
        )
      }
    >
      <Field label="PDF to export">
        <div className="tool-modal__row">
          <input type="text" value={inputPath ? pathToFileName(inputPath) : ""} readOnly placeholder="Choose a PDF" />
          <button className="tool-modal__secondary-btn" disabled={running} onClick={pickFile}>
            Browse…
          </button>
        </div>
        {pageCount != null && <p className="tool-modal__note">{pageCount} page{pageCount === 1 ? "" : "s"} in this document.</p>}
        {pageCountError && <p className="tool-modal__error">{pageCountError}</p>}
      </Field>

      <Field label="Pages">
        <Segmented
          value={selection}
          options={[
            { value: "all", label: "Every page" },
            { value: "range", label: "Page range" },
          ]}
          onChange={setSelection}
        />
        {selection === "range" && (
          <input
            type="text"
            value={rangeSpec}
            onChange={(e) => setRangeSpec(e.target.value)}
            placeholder="1-3,5"
            disabled={running}
            style={{ marginTop: 8 }}
          />
        )}
      </Field>

      <div className="tool-modal__row">
        <Field label="Format">
          <Segmented
            value={format}
            options={[
              { value: "png", label: "PNG" },
              { value: "jpg", label: "JPG" },
              { value: "webp", label: "WebP" },
            ]}
            onChange={setFormat}
          />
        </Field>
        <Field label={`Resolution — ${dpi} DPI`}>
          <input type="range" min={72} max={600} step={1} value={dpi} onChange={(e) => setDpi(Number(e.target.value))} disabled={running} />
        </Field>
      </div>

      <Field label="File name prefix">
        <input type="text" value={baseName} onChange={(e) => setBaseName(e.target.value)} disabled={running} />
        <p className="tool-modal__note">
          Produces {baseName || "document"}-page-001.{format}, {baseName || "document"}-page-002.{format}, and so on.
        </p>
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
          Exporting…
        </p>
      )}
      {error && <p className="tool-modal__error">{error}</p>}
    </ToolModal>
  );
}
