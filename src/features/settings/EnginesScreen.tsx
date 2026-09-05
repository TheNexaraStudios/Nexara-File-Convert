import { useState } from "react";
import { RefreshCw, CheckCircle2, XCircle, Wrench, Loader2 } from "lucide-react";
import { useRegistryStore } from "../../stores/useRegistryStore";
import { useProvisioningStore } from "../../stores/useProvisioningStore";
import "../../styles/screen.css";
import "./EnginesScreen.css";

export function EnginesScreen() {
  const engines = useRegistryStore((s) => s.engines);
  const load = useRegistryStore((s) => s.load);
  const [refreshing, setRefreshing] = useState(false);
  const provisioningRunning = useProvisioningStore((s) => s.running);
  const runSetup = useProvisioningStore((s) => s.runSetup);
  const provisioningFailures = useProvisioningStore((s) => s.failures);

  const refresh = async () => {
    setRefreshing(true);
    await load();
    setRefreshing(false);
  };

  const repair = async () => {
    await runSetup();
    await load();
  };

  return (
    <div className="screen-page">
      <header className="screen-page__header engines-header">
        <div>
          <h1>Conversion Engines</h1>
          <p>Every engine is bundled with Nexara or fetched automatically from its official source — nothing to install by hand.</p>
        </div>
        <div className="engines-header__actions">
          <button className="engines-refresh" onClick={repair} disabled={provisioningRunning}>
            {provisioningRunning ? <Loader2 size={13} className="anim-spin" /> : <Wrench size={13} />}
            Re-run setup
          </button>
          <button className="engines-refresh" onClick={refresh} disabled={refreshing}>
            <RefreshCw size={13} className={refreshing ? "anim-spin" : ""} />
            Refresh
          </button>
        </div>
      </header>

      {provisioningFailures.length > 0 && (
        <p className="engines-repair-note">
          Setup couldn't reach {provisioningFailures.map((f) => f.id).join(", ")} last time — check your internet connection
          and try "Re-run setup" again.
        </p>
      )}

      <div className="engines-list">
        {engines.map((engine) => (
          <div key={engine.id} className="engine-row">
            <div className={`engine-row__status engine-row__status--${engine.availability}`}>
              {engine.availability === "available" ? <CheckCircle2 size={16} /> : <XCircle size={16} />}
            </div>
            <div className="engine-row__text">
              <div className="engine-row__name">
                {engine.name}
                {!engine.implemented && <span className="engine-row__preview">Preview</span>}
              </div>
              <div className="engine-row__description">{engine.description}</div>
            </div>
            <div className="engine-row__meta">
              <div className="engine-row__binary">{engine.binary}</div>
              <div className={`engine-row__label engine-row__label--${engine.availability}`}>
                {engine.availability === "available" ? "Detected" : "Not found"}
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
