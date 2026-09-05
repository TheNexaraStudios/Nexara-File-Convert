import type { ReactNode } from "react";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { Segmented } from "../../components/common/Segmented";
import "../../styles/screen.css";
import "./SettingsScreen.css";

export function SettingsScreen() {
  const {
    theme,
    setTheme,
    outputLocationMode,
    setOutputLocationMode,
    performanceMode,
    setPerformanceMode,
  } = useSettingsStore();

  return (
    <div className="screen-page">
      <header className="screen-page__header">
        <h1>Settings</h1>
        <p>Nexara runs entirely on your computer — nothing here changes that.</p>
      </header>

      <SettingsSection title="Appearance">
        <SettingsRow label="Theme" description="Match Windows, or choose light or dark yourself.">
          <Segmented
            value={theme}
            onChange={setTheme}
            options={[
              { value: "system", label: "System" },
              { value: "light", label: "Light" },
              { value: "dark", label: "Dark" },
            ]}
          />
        </SettingsRow>
      </SettingsSection>

      <SettingsSection title="Conversions">
        <SettingsRow label="Output location" description="Where converted files are saved by default.">
          <div className="radio-list">
            {(
              [
                { value: "same-folder", label: "Same folder as source" },
                { value: "ask", label: "Ask every time" },
                { value: "custom", label: "Custom default folder" },
              ] as const
            ).map((opt) => (
              <label key={opt.value} className="radio-list__item">
                <input
                  type="radio"
                  name="output-location"
                  checked={outputLocationMode === opt.value}
                  onChange={() => setOutputLocationMode(opt.value)}
                />
                {opt.label}
              </label>
            ))}
          </div>
        </SettingsRow>

        <SettingsRow label="Performance mode" description="Controls how many conversions run at once.">
          <Segmented
            value={performanceMode}
            onChange={setPerformanceMode}
            options={[
              { value: "eco", label: "Eco" },
              { value: "balanced", label: "Balanced" },
              { value: "maximum", label: "Maximum" },
            ]}
          />
        </SettingsRow>
      </SettingsSection>
    </div>
  );
}

function SettingsSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="settings-section">
      <h2 className="settings-section__title">{title}</h2>
      <div className="settings-section__body">{children}</div>
    </section>
  );
}

function SettingsRow({
  label,
  description,
  children,
}: {
  label: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <div className="settings-row">
      <div className="settings-row__text">
        <div className="settings-row__label">{label}</div>
        <div className="settings-row__description">{description}</div>
      </div>
      <div className="settings-row__control">{children}</div>
    </div>
  );
}
