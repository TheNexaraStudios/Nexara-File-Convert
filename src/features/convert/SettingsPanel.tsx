import { useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { X, ChevronDown } from "lucide-react";
import type { FormatInfo } from "../../types/format";
import type { ConversionSettings, QualityPreset, ResizeMode } from "../../types/job";
import { Segmented } from "../../components/common/Segmented";
import "./SettingsPanel.css";

interface SettingsPanelProps {
  fileName: string;
  outputFormat: FormatInfo;
  settings: ConversionSettings;
  onChange: (settings: ConversionSettings) => void;
  onClose: () => void;
}

const PRESET_OPTIONS: { value: QualityPreset; label: string }[] = [
  { value: "high", label: "High" },
  { value: "balanced", label: "Balanced" },
  { value: "small", label: "Smaller File" },
  { value: "custom", label: "Custom" },
];

export function SettingsPanel({ fileName, outputFormat, settings, onChange, onClose }: SettingsPanelProps) {
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const update = (patch: Partial<ConversionSettings>) => onChange({ ...settings, ...patch });

  return createPortal(
    <div className="settings-panel-overlay anim-fade-in" onMouseDown={onClose}>
      <aside
        className="settings-panel anim-slide-in-right"
        onMouseDown={(e) => e.stopPropagation()}
        aria-label="Conversion settings"
      >
        <div className="settings-panel__header">
          <div>
            <h3>Conversion settings</h3>
            <p className="settings-panel__subtitle">{fileName} → {outputFormat.name}</p>
          </div>
          <button className="settings-panel__close" onClick={onClose} aria-label="Close">
            <X size={16} />
          </button>
        </div>

        <div className="settings-panel__body">
          <Field label="Quality">
            <Segmented value={settings.preset} options={PRESET_OPTIONS} onChange={(preset) => update({ preset })} />
          </Field>

          {outputFormat.category === "video" && (
            <>
              <Field label="Resolution">
                <select
                  value={settings.resolution ?? "original"}
                  onChange={(e) => update({ resolution: e.target.value })}
                >
                  <option value="original">Original resolution</option>
                  <option value="2160p">4K (2160p)</option>
                  <option value="1440p">1440p</option>
                  <option value="1080p">1080p</option>
                  <option value="720p">720p</option>
                  <option value="480p">480p</option>
                </select>
              </Field>
              <Field label="Frame rate">
                <select value={settings.frameRate ?? "original"} onChange={(e) => update({ frameRate: e.target.value })}>
                  <option value="original">Original frame rate</option>
                  <option value="24">24 fps</option>
                  <option value="25">25 fps</option>
                  <option value="30">30 fps</option>
                  <option value="50">50 fps</option>
                  <option value="60">60 fps</option>
                </select>
              </Field>
            </>
          )}

          {outputFormat.category === "audio" && (
            <Field label="Bitrate">
              <select value={settings.audioBitrate ?? "192k"} onChange={(e) => update({ audioBitrate: e.target.value })}>
                <option value="128k">128 kbps</option>
                <option value="192k">192 kbps</option>
                <option value="256k">256 kbps</option>
                <option value="320k">320 kbps</option>
              </select>
            </Field>
          )}

          {(outputFormat.category === "image" || outputFormat.category === "raw-image") && (
            <>
              <ResizeFields settings={settings} update={update} />
              <Field label={`Image quality — ${settings.imageQuality ?? 85}`}>
                <input
                  type="range"
                  min={10}
                  max={100}
                  value={settings.imageQuality ?? 85}
                  onChange={(e) => update({ imageQuality: Number(e.target.value) })}
                />
              </Field>
              <label className="settings-panel__checkbox">
                <input
                  type="checkbox"
                  checked={settings.stripMetadata ?? false}
                  onChange={(e) => update({ stripMetadata: e.target.checked })}
                />
                Strip metadata (EXIF, location, camera info)
              </label>
            </>
          )}

          {["document", "spreadsheet", "presentation", "ebook", "archive", "vector", "cad", "font", "text-markup"].includes(
            outputFormat.category
          ) && (
            <p className="settings-panel__note">
              Nexara will use sensible defaults for {outputFormat.name} — there's nothing else to configure for this
              format yet.
            </p>
          )}

          {outputFormat.category === "video" && (
            <div className="settings-panel__advanced">
              <button className="settings-panel__advanced-toggle" onClick={() => setAdvancedOpen((v) => !v)}>
                <ChevronDown size={14} className={advancedOpen ? "settings-panel__chevron--open" : ""} />
                Advanced
              </button>
              {advancedOpen && (
                <div className="settings-panel__advanced-body anim-rise-in">
                  <Field label="Video codec">
                    <select value={settings.videoCodec ?? "h264"} onChange={(e) => update({ videoCodec: e.target.value })}>
                      <option value="h264">H.264 (most compatible)</option>
                      <option value="h265">H.265 / HEVC (smaller files)</option>
                      <option value="av1">AV1 (best compression)</option>
                    </select>
                  </Field>
                </div>
              )}
            </div>
          )}
        </div>
      </aside>
    </div>,
    document.body
  );
}

type ResizeUiMode = "off" | "percent" | "exact";

const RESIZE_MODE_OPTIONS: { value: ResizeMode; label: string }[] = [
  { value: "fit", label: "Fit (contain)" },
  { value: "fill", label: "Fill (crop)" },
  { value: "stretch", label: "Stretch" },
];

/** Real width/height/percentage/aspect-ratio/fit-fill-stretch resize
 * controls, backed directly by the image engine's `-resize`/`-extent`
 * geometry (see `conversion::image::explicit_resize_args`) — available for
 * any image conversion, not just the Resize Image tool shortcut. */
function ResizeFields({ settings, update }: { settings: ConversionSettings; update: (patch: Partial<ConversionSettings>) => void }) {
  const uiMode: ResizeUiMode = settings.resizePercent != null ? "percent" : settings.resizeWidth != null || settings.resizeHeight != null ? "exact" : "off";
  const lockAspect = uiMode === "exact" && (settings.resizeWidth == null || settings.resizeHeight == null);

  const setMode = (mode: ResizeUiMode) => {
    if (mode === "off") {
      update({ resizePercent: undefined, resizeWidth: undefined, resizeHeight: undefined, resizeMode: undefined });
    } else if (mode === "percent") {
      update({ resizePercent: 100, resizeWidth: undefined, resizeHeight: undefined, resizeMode: undefined });
    } else {
      update({ resizePercent: undefined, resizeWidth: undefined, resizeHeight: undefined, resizeMode: "fit" });
    }
  };

  return (
    <Field label="Resize">
      <Segmented
        value={uiMode}
        options={[
          { value: "off", label: "Original size" },
          { value: "percent", label: "Percentage" },
          { value: "exact", label: "Exact size" },
        ]}
        onChange={setMode}
      />

      {uiMode === "percent" && (
        <div className="settings-panel__resize-block">
          <Field label={`Scale — ${settings.resizePercent ?? 100}%`}>
            <input
              type="range"
              min={5}
              max={300}
              value={settings.resizePercent ?? 100}
              onChange={(e) => update({ resizePercent: Number(e.target.value) })}
            />
          </Field>
        </div>
      )}

      {uiMode === "exact" && (
        <div className="settings-panel__resize-block">
          <div className="settings-panel__resize-dims">
            <Field label="Width (px)">
              <input
                type="number"
                min={1}
                placeholder="auto"
                value={settings.resizeWidth ?? ""}
                onChange={(e) => update({ resizeWidth: e.target.value ? Number(e.target.value) : undefined })}
              />
            </Field>
            <Field label="Height (px)">
              <input
                type="number"
                min={1}
                placeholder="auto"
                value={settings.resizeHeight ?? ""}
                onChange={(e) => update({ resizeHeight: e.target.value ? Number(e.target.value) : undefined })}
              />
            </Field>
          </div>
          <label className="settings-panel__checkbox">
            <input
              type="checkbox"
              checked={lockAspect}
              onChange={(e) => {
                if (e.target.checked) {
                  // Locking aspect means only one dimension drives the
                  // resize — drop whichever one was set second so the
                  // image engine's aspect-preserving single-dimension
                  // geometry (`WxN`/`NxH`) applies.
                  if (settings.resizeWidth != null && settings.resizeHeight != null) {
                    update({ resizeHeight: undefined });
                  }
                } else if (settings.resizeWidth == null) {
                  update({ resizeWidth: settings.resizeHeight ?? 100 });
                } else if (settings.resizeHeight == null) {
                  update({ resizeHeight: settings.resizeWidth });
                }
              }}
            />
            Lock aspect ratio
          </label>
          {!lockAspect && (
            <Field label="When width and height don't match the original aspect ratio">
              <Segmented value={settings.resizeMode ?? "fit"} options={RESIZE_MODE_OPTIONS} onChange={(resizeMode) => update({ resizeMode })} />
            </Field>
          )}
        </div>
      )}
    </Field>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="settings-panel__field">
      <label className="settings-panel__label">{label}</label>
      {children}
    </div>
  );
}
