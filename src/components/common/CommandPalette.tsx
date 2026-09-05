import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  Search,
  FilePlus,
  ArrowRightLeft,
  ListChecks,
  History,
  Wrench,
  Settings,
  Cpu,
  Trash2,
} from "lucide-react";
import { useCommandPaletteStore } from "../../stores/useCommandPaletteStore";
import { useNavStore, type Screen } from "../../stores/useNavStore";
import { useJobStore } from "../../stores/useJobStore";
import { resolveFiles } from "../../hooks/useFileDrop";
import { useToastStore } from "../../stores/useToastStore";
import "./CommandPalette.css";

interface Command {
  id: string;
  label: string;
  hint?: string;
  icon: typeof Search;
  run: () => void;
}

export function CommandPalette() {
  const open = useCommandPaletteStore((s) => s.open);
  const setOpen = useCommandPaletteStore((s) => s.setOpen);
  const [query, setQuery] = useState("");
  const go = useNavStore((s) => s.go);
  const addFiles = useJobStore((s) => s.addFiles);
  const clearCompleted = useJobStore((s) => s.clearCompleted);
  const push = useToastStore((s) => s.push);

  useEffect(() => {
    if (!open) setQuery("");
  }, [open]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        useCommandPaletteStore.getState().toggle();
      }
      if (e.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [setOpen]);

  const commands: Command[] = useMemo(() => {
    const navigate = (screen: Screen, label: string, icon: typeof Search) => ({
      id: `nav-${screen}`,
      label,
      icon,
      run: () => go(screen),
    });

    return [
      {
        id: "add-files",
        label: "Add Files",
        hint: "Ctrl+O",
        icon: FilePlus,
        run: async () => {
          const selection = await openDialog({ multiple: true });
          if (!selection) return;
          const paths = Array.isArray(selection) ? selection : [selection];
          const files = await resolveFiles(paths);
          if (files.length > 0) {
            addFiles(files);
            go("convert");
            push(files.length === 1 ? "File added" : `${files.length} files added`, "success");
          }
        },
      },
      navigate("convert", "Go to Convert", ArrowRightLeft),
      navigate("queue", "Go to Queue", ListChecks),
      navigate("history", "Go to History", History),
      navigate("tools", "Go to Tools", Wrench),
      navigate("engines", "Conversion Engines", Cpu),
      navigate("settings", "Settings", Settings),
      {
        id: "clear-completed",
        label: "Clear Completed Jobs",
        icon: Trash2,
        run: clearCompleted,
      },
    ];
  }, [go, addFiles, clearCompleted, push]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter((c) => c.label.toLowerCase().includes(q));
  }, [commands, query]);

  if (!open) return null;

  return (
    <div className="command-palette-overlay anim-fade-in" onMouseDown={() => setOpen(false)}>
      <div
        className="command-palette anim-scale-in"
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="command-palette__search">
          <Search size={15} strokeWidth={2} />
          <input
            autoFocus
            placeholder="Type a command..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && filtered[0]) {
                filtered[0].run();
                setOpen(false);
              }
            }}
          />
          <kbd>Esc</kbd>
        </div>
        <div className="command-palette__list">
          {filtered.length === 0 && <div className="command-palette__empty">No matching commands</div>}
          {filtered.map((cmd) => (
            <button
              key={cmd.id}
              className="command-palette__item"
              onClick={() => {
                cmd.run();
                setOpen(false);
              }}
            >
              <cmd.icon size={15} strokeWidth={1.9} />
              <span>{cmd.label}</span>
              {cmd.hint && <kbd>{cmd.hint}</kbd>}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
