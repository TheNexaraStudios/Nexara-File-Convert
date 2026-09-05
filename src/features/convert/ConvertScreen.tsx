import { open } from "@tauri-apps/plugin-dialog";
import { useFileDrop, resolveFiles } from "../../hooks/useFileDrop";
import { useJobStore } from "../../stores/useJobStore";
import { useToastStore } from "../../stores/useToastStore";
import { DropZone } from "./DropZone";
import { FileCard } from "./FileCard";
import "./ConvertScreen.css";

const QUICK_ACTIONS = [
  { label: "Compress a video", hint: "Video → smaller MP4" },
  { label: "Extract audio", hint: "Video → MP3" },
  { label: "Convert images to PDF", hint: "Images → PDF" },
  { label: "Convert to PDF", hint: "Document → PDF" },
];

export function ConvertScreen() {
  const pending = useJobStore((s) => s.pending);
  const addFiles = useJobStore((s) => s.addFiles);
  const startAll = useJobStore((s) => s.startAll);
  const clearPending = useJobStore((s) => s.clearPending);
  const push = useToastStore((s) => s.push);

  const { isDraggingOver } = useFileDrop((files) => {
    addFiles(files);
    push(files.length === 1 ? "File added" : `${files.length} files added`, "success");
  });

  const chooseFiles = async () => {
    const selection = await open({ multiple: true });
    if (!selection) return;
    const paths = Array.isArray(selection) ? selection : [selection];
    const files = await resolveFiles(paths);
    if (files.length > 0) {
      addFiles(files);
      push(files.length === 1 ? "File added" : `${files.length} files added`, "success");
    }
  };

  const readyCount = pending.filter((p) => p.outputFormatId).length;

  if (pending.length === 0) {
    return (
      <div className="convert-screen convert-screen--empty anim-fade-in">
        <div className="convert-screen__hero">
          <h1>Convert Files</h1>
          <p>Fast, private file conversion directly on your computer.</p>
        </div>

        <div className="convert-screen__dropzone-wrap">
          <DropZone isDraggingOver={isDraggingOver} onChooseFiles={chooseFiles} />
        </div>

        <div className="convert-screen__quick-actions">
          {QUICK_ACTIONS.map((action) => (
            <button key={action.label} className="quick-action" onClick={chooseFiles}>
              <span className="quick-action__label">{action.label}</span>
              <span className="quick-action__hint">{action.hint}</span>
            </button>
          ))}
        </div>

        <p className="convert-screen__privacy">Processed locally. Your files never leave your computer.</p>
      </div>
    );
  }

  return (
    <div className="convert-screen anim-fade-in">
      <div className="convert-screen__workspace-header">
        <div>
          <h1>Convert Files</h1>
          <p>
            {pending.length} file{pending.length === 1 ? "" : "s"} ready
          </p>
        </div>
        <button className="convert-screen__clear" onClick={clearPending}>
          Clear all
        </button>
      </div>

      <DropZone isDraggingOver={isDraggingOver} onChooseFiles={chooseFiles} compact />

      <div className="convert-screen__list">
        {pending.map((p) => (
          <FileCard key={p.file.id} pending={p} />
        ))}
      </div>

      <div className="convert-screen__footer">
        <span className="convert-screen__footer-hint">
          {readyCount} of {pending.length} ready to convert
        </span>
        <button className="convert-screen__convert-all" disabled={readyCount === 0} onClick={startAll}>
          Convert All ({readyCount})
        </button>
      </div>
    </div>
  );
}
