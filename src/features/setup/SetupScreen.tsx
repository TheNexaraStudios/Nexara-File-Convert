import { useEffect, useMemo } from "react";
import { CheckCircle2, XCircle, Loader2, Circle, DownloadCloud, ShieldCheck, PackageOpen, Settings2 } from "lucide-react";
import { useProvisioningStore } from "../../stores/useProvisioningStore";
import type { ProvisionPhase } from "../../types/provisioning";
import "./SetupScreen.css";

function phaseLabel(phase: ProvisionPhase): string {
  switch (phase) {
    case "pending":
      return "Waiting…";
    case "downloading":
      return "Downloading…";
    case "verifying":
      return "Verifying…";
    case "extracting":
      return "Extracting…";
    case "installing":
      return "Installing…";
    case "ready":
      return "Ready";
    case "failed":
      return "Failed";
  }
}

function phaseIcon(phase: ProvisionPhase | undefined, ready: boolean) {
  if (ready || phase === "ready") return <CheckCircle2 size={16} />;
  if (phase === "failed") return <XCircle size={16} />;
  if (phase === "downloading") return <DownloadCloud size={15} />;
  if (phase === "verifying") return <ShieldCheck size={15} />;
  if (phase === "extracting" || phase === "installing") return <PackageOpen size={15} />;
  if (phase === "pending") return <Loader2 size={15} className="anim-spin" />;
  return <Circle size={14} />;
}

function formatBytes(n: number | null): string {
  if (n == null) return "";
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

export function SetupScreen() {
  const readiness = useProvisioningStore((s) => s.readiness);
  const running = useProvisioningStore((s) => s.running);
  const attempted = useProvisioningStore((s) => s.attempted);
  const progressById = useProvisioningStore((s) => s.progressById);
  const failures = useProvisioningStore((s) => s.failures);
  const runSetup = useProvisioningStore((s) => s.runSetup);
  const dismiss = useProvisioningStore((s) => s.dismiss);

  useEffect(() => {
    if (!attempted) {
      void runSetup();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const allReady = useMemo(() => readiness.length > 0 && readiness.every((e) => e.ready), [readiness]);
  const doneAttempting = !running && attempted;

  return (
    <div className="setup-screen">
      <div className="setup-card">
        <div className="setup-card__icon">
          <Settings2 size={22} strokeWidth={1.7} />
        </div>
        <h1>Setting up Nexara File Convert</h1>
        <p className="setup-card__subtitle">
          Nexara is preparing every conversion engine it needs — nothing to configure, nothing to install by hand. This only
          happens once.
        </p>

        <div className="setup-list">
          {readiness.map((engine) => {
            const progress = progressById[engine.id];
            const phase = progress?.phase;
            const showBar = phase === "downloading" && progress?.bytesTotal;
            const percent = showBar ? Math.min(100, Math.round(((progress!.bytesDownloaded ?? 0) / progress!.bytesTotal!) * 100)) : null;
            return (
              <div key={engine.id} className={`setup-row setup-row--${engine.ready || phase === "ready" ? "ready" : phase === "failed" ? "failed" : "pending"}`}>
                <div className="setup-row__icon">{phaseIcon(phase, engine.ready)}</div>
                <div className="setup-row__text">
                  <div className="setup-row__name">{engine.displayName}</div>
                  {phase && !engine.ready && (
                    <div className="setup-row__status">
                      {phaseLabel(phase)}
                      {percent != null && ` ${percent}% (${formatBytes(progress!.bytesDownloaded)} / ${formatBytes(progress!.bytesTotal)})`}
                      {phase === "failed" && progress?.message ? ` — ${progress.message}` : ""}
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>

        {doneAttempting && failures.length > 0 && (
          <div className="setup-warning">
            {failures.length === 1 ? "One engine" : `${failures.length} engines`} couldn't be set up automatically. You can
            retry now, or continue — the rest of Nexara will still work, and you can retry later from Conversion Engines in
            Settings.
          </div>
        )}

        <div className="setup-actions">
          {doneAttempting && !allReady && (
            <button className="setup-btn setup-btn--secondary" onClick={() => void runSetup()}>
              Retry
            </button>
          )}
          {doneAttempting && (
            <button className="setup-btn setup-btn--primary" onClick={dismiss}>
              {allReady ? "Continue" : "Continue anyway"}
            </button>
          )}
          {!doneAttempting && (
            <button className="setup-btn setup-btn--primary" disabled>
              <Loader2 size={14} className="anim-spin" /> Setting up…
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
