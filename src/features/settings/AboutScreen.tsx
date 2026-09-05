import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { appLogDir } from "@tauri-apps/api/path";
import { mkdir } from "@tauri-apps/plugin-fs";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { ShieldCheck, FolderOpen } from "lucide-react";
import { useRegistryStore } from "../../stores/useRegistryStore";
import "../../styles/screen.css";
import "./AboutScreen.css";

export function AboutScreen() {
  const [version, setVersion] = useState("");
  const engines = useRegistryStore((s) => s.engines);
  const availableCount = engines.filter((e) => e.availability === "available").length;

  useEffect(() => {
    getVersion().then(setVersion).catch(() => setVersion("0.1.0"));
  }, []);

  const openLogsFolder = async () => {
    const dir = await appLogDir();
    await mkdir(dir, { recursive: true }).catch(() => {});
    await revealItemInDir(dir).catch(() => {});
  };

  return (
    <div className="screen-page">
      <header className="screen-page__header">
        <h1>About</h1>
      </header>

      <div className="about-card">
        <div className="about-card__mark">
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none">
            <path d="M4 12 L12 4 L20 12 L12 20 Z" fill="var(--text-on-accent)" />
          </svg>
        </div>
        <h2>Nexara File Convert</h2>
        <p className="about-card__version">Version {version || "…"}</p>

        <div className="about-card__privacy">
          <ShieldCheck size={15} />
          Processed locally. Your files never leave your computer.
        </div>
      </div>

      <div className="about-row">
        <span>Conversion engines detected</span>
        <span>
          {availableCount} of {engines.length}
        </span>
      </div>

      <button className="about-logs-btn" onClick={openLogsFolder}>
        <FolderOpen size={14} />
        Open Logs Folder
      </button>

      <p className="about-footnote">No account, no telemetry, no cloud upload — Nexara is a local desktop utility.</p>
    </div>
  );
}
