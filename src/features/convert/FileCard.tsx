import { useState } from "react";
import { ArrowRight, Settings2, X, ChevronDown } from "lucide-react";
import type { PendingFile } from "../../stores/useJobStore";
import { useJobStore } from "../../stores/useJobStore";
import { useRegistryStore } from "../../stores/useRegistryStore";
import { formatBytes, formatDuration } from "../../utils/format";
import { iconForCategory } from "./categoryIcons";
import { FormatPicker } from "./FormatPicker";
import { SettingsPanel } from "./SettingsPanel";
import "./FileCard.css";

export function FileCard({ pending }: { pending: PendingFile }) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  const removeFile = useJobStore((s) => s.removeFile);
  const setOutputFormat = useJobStore((s) => s.setOutputFormat);
  const setSettings = useJobStore((s) => s.setSettings);
  const startJob = useJobStore((s) => s.startJob);

  const formatsById = useRegistryStore((s) => s.formatsById);
  const compatibleOutputs = useRegistryStore((s) => s.compatibleOutputs);

  const detected = pending.file.detectedFormatId ? formatsById[pending.file.detectedFormatId] : null;
  const target = pending.outputFormatId ? formatsById[pending.outputFormatId] : null;
  const options = compatibleOutputs(pending.file.detectedFormatId);
  const InputIcon = iconForCategory(detected?.category);

  return (
    <div className="file-card anim-rise-in">
      <div className="file-card__icon">
        <InputIcon size={18} strokeWidth={1.8} />
      </div>

      <div className="file-card__info">
        <div className="file-card__name" title={pending.file.name}>
          {pending.file.name}
        </div>
        <div className="file-card__meta">
          <span>{formatBytes(pending.file.sizeBytes)}</span>
          <span className="file-card__dot" />
          <span>{detected ? detected.name : "Unrecognized format"}</span>
          {pending.probe?.durationSeconds != null && (
            <>
              <span className="file-card__dot" />
              <span>{formatDuration(pending.probe.durationSeconds)}</span>
            </>
          )}
          {pending.probe?.width && pending.probe?.height && (
            <>
              <span className="file-card__dot" />
              <span>
                {pending.probe.width}×{pending.probe.height}
              </span>
            </>
          )}
          {pending.probe?.videoCodec && (
            <>
              <span className="file-card__dot" />
              <span>{pending.probe.videoCodec.toUpperCase()}</span>
            </>
          )}
        </div>
      </div>

      <div className="file-card__conversion">
        <span className="file-card__format-chip">{detected?.id.toUpperCase() ?? pending.file.extension.toUpperCase()}</span>
        <ArrowRight size={14} className="file-card__arrow" />
        <button className="file-card__target" onClick={() => setPickerOpen(true)} disabled={options.length === 0}>
          {target ? target.id.toUpperCase() : "Choose"}
          <ChevronDown size={13} />
        </button>
      </div>

      <div className="file-card__actions">
        <button
          className="file-card__icon-btn"
          onClick={() => setSettingsOpen(true)}
          disabled={!target}
          aria-label="Conversion settings"
          title="Conversion settings"
        >
          <Settings2 size={15} />
        </button>
        <button
          className="file-card__convert-btn"
          disabled={!target}
          onClick={() => startJob(pending.file.id)}
        >
          Convert
        </button>
        <button className="file-card__icon-btn" onClick={() => removeFile(pending.file.id)} aria-label="Remove file">
          <X size={15} />
        </button>
      </div>

      {options.length === 0 && (
        <div className="file-card__unsupported">Nexara doesn't recognize this file type yet — no conversions available.</div>
      )}

      {pickerOpen && (
        <FormatPicker
          options={options}
          selectedFormatId={pending.outputFormatId}
          onSelect={(id) => {
            setOutputFormat(pending.file.id, id);
            setPickerOpen(false);
          }}
          onClose={() => setPickerOpen(false)}
        />
      )}

      {settingsOpen && target && (
        <SettingsPanel
          fileName={pending.file.name}
          outputFormat={target}
          settings={pending.settings}
          onChange={(s) => setSettings(pending.file.id, s)}
          onClose={() => setSettingsOpen(false)}
        />
      )}
    </div>
  );
}
