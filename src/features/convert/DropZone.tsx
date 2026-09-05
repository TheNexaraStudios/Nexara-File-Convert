import { UploadCloud } from "lucide-react";
import "./DropZone.css";

interface DropZoneProps {
  isDraggingOver: boolean;
  onChooseFiles: () => void;
  compact?: boolean;
}

export function DropZone({ isDraggingOver, onChooseFiles, compact }: DropZoneProps) {
  return (
    <div
      className={`dropzone ${isDraggingOver ? "dropzone--active" : ""} ${compact ? "dropzone--compact" : ""}`}
      onClick={onChooseFiles}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") onChooseFiles();
      }}
    >
      <div className="dropzone__icon">
        <UploadCloud size={compact ? 18 : 26} strokeWidth={1.6} />
      </div>
      {!compact && (
        <>
          <p className="dropzone__title">Drop files here</p>
          <p className="dropzone__helper">or click to choose files from your computer</p>
        </>
      )}
      {compact && <p className="dropzone__title dropzone__title--compact">Drop more files, or click to browse</p>}
    </div>
  );
}
