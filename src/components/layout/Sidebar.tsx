import {
  ArrowRightLeft,
  ListChecks,
  History,
  Wrench,
  Settings,
  Cpu,
  Info,
} from "lucide-react";
import { useNavStore, type Screen } from "../../stores/useNavStore";
import { useJobStore } from "../../stores/useJobStore";
import "./Sidebar.css";

const PRIMARY_ITEMS: { screen: Screen; label: string; icon: typeof ArrowRightLeft }[] = [
  { screen: "convert", label: "Convert", icon: ArrowRightLeft },
  { screen: "queue", label: "Queue", icon: ListChecks },
  { screen: "history", label: "History", icon: History },
  { screen: "tools", label: "Tools", icon: Wrench },
];

const SECONDARY_ITEMS: { screen: Screen; label: string; icon: typeof ArrowRightLeft }[] = [
  { screen: "engines", label: "Conversion Engines", icon: Cpu },
  { screen: "settings", label: "Settings", icon: Settings },
  { screen: "about", label: "About", icon: Info },
];

export function Sidebar() {
  const screen = useNavStore((s) => s.screen);
  const go = useNavStore((s) => s.go);
  const activeCount = useJobStore((s) => s.jobs.filter((j) => j.status === "converting" || j.status === "queued").length);

  return (
    <aside className="sidebar">
      <div className="sidebar__brand">
        <div className="sidebar__mark" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none">
            <path d="M4 12 L12 4 L20 12 L12 20 Z" fill="var(--text-on-accent)" />
          </svg>
        </div>
        <span className="sidebar__wordmark">Nexara</span>
      </div>

      <nav className="sidebar__nav" aria-label="Primary">
        {PRIMARY_ITEMS.map(({ screen: target, label, icon: Icon }) => (
          <button
            key={target}
            className={`sidebar__item ${screen === target ? "sidebar__item--active" : ""}`}
            onClick={() => go(target)}
            aria-current={screen === target ? "page" : undefined}
          >
            <Icon size={17} strokeWidth={1.9} />
            <span>{label}</span>
            {target === "queue" && activeCount > 0 && <span className="sidebar__badge">{activeCount}</span>}
          </button>
        ))}
      </nav>

      <div className="sidebar__spacer" />

      <nav className="sidebar__nav sidebar__nav--secondary" aria-label="Secondary">
        {SECONDARY_ITEMS.map(({ screen: target, label, icon: Icon }) => (
          <button
            key={target}
            className={`sidebar__item ${screen === target ? "sidebar__item--active" : ""}`}
            onClick={() => go(target)}
            aria-current={screen === target ? "page" : undefined}
          >
            <Icon size={16} strokeWidth={1.9} />
            <span>{label}</span>
          </button>
        ))}
      </nav>
    </aside>
  );
}
