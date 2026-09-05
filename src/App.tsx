import { useEffect } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { AppShell } from "./components/layout/AppShell";
import { ToastHost } from "./components/common/ToastHost";
import { CommandPalette } from "./components/common/CommandPalette";
import { useTheme } from "./hooks/useTheme";
import { useNavStore } from "./stores/useNavStore";
import { useRegistryStore } from "./stores/useRegistryStore";
import { useJobStore } from "./stores/useJobStore";
import { useToastStore } from "./stores/useToastStore";
import { useProvisioningStore } from "./stores/useProvisioningStore";
import { resolveFiles } from "./hooks/useFileDrop";
import { ConvertScreen } from "./features/convert/ConvertScreen";
import { QueueScreen } from "./features/queue/QueueScreen";
import { HistoryScreen } from "./features/history/HistoryScreen";
import { ToolsScreen } from "./features/tools/ToolsScreen";
import { SettingsScreen } from "./features/settings/SettingsScreen";
import { EnginesScreen } from "./features/settings/EnginesScreen";
import { AboutScreen } from "./features/settings/AboutScreen";
import { SetupScreen } from "./features/setup/SetupScreen";

function App() {
  useTheme();

  const screen = useNavStore((s) => s.screen);
  const go = useNavStore((s) => s.go);
  const loadRegistry = useRegistryStore((s) => s.load);
  const addFiles = useJobStore((s) => s.addFiles);
  const push = useToastStore((s) => s.push);
  const checkEngineReadiness = useProvisioningStore((s) => s.checkReadiness);
  const needsSetup = useProvisioningStore((s) => s.needsSetup());

  useEffect(() => {
    checkEngineReadiness();
  }, [checkEngineReadiness]);

  useEffect(() => {
    loadRegistry();
  }, [loadRegistry]);

  useEffect(() => {
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) return;
    const setProgress = useJobStore.getState().setProgress;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen<{ jobId: string; percent: number | null }>("nexara://conversion-progress", (event) => {
      setProgress(event.payload.jobId, event.payload.percent);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    const handler = async (e: KeyboardEvent) => {
      const meta = e.ctrlKey || e.metaKey;
      if (meta && e.key.toLowerCase() === "o") {
        e.preventDefault();
        const selection = await openDialog({ multiple: true });
        if (!selection) return;
        const paths = Array.isArray(selection) ? selection : [selection];
        const files = await resolveFiles(paths);
        if (files.length > 0) {
          addFiles(files);
          go("convert");
          push(files.length === 1 ? "File added" : `${files.length} files added`, "success");
        }
      } else if (meta && e.key === ",") {
        e.preventDefault();
        go("settings");
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [addFiles, go, push]);

  if (needsSetup) {
    return <SetupScreen />;
  }

  return (
    <AppShell>
      {screen === "convert" && <ConvertScreen />}
      {screen === "queue" && <QueueScreen />}
      {screen === "history" && <HistoryScreen />}
      {screen === "tools" && <ToolsScreen />}
      {screen === "settings" && <SettingsScreen />}
      {screen === "engines" && <EnginesScreen />}
      {screen === "about" && <AboutScreen />}
      <ToastHost />
      <CommandPalette />
    </AppShell>
  );
}

export default App;
