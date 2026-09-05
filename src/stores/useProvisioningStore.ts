import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { EngineReadiness, ProvisionEvent, ProvisionFailure } from "../types/provisioning";

interface ProvisioningState {
  checked: boolean;
  readiness: EngineReadiness[];
  running: boolean;
  /** True once `runSetup` has been invoked at least once — tracked
   * explicitly rather than derived from `progressById`, since an IPC
   * failure before any progress event arrives would otherwise leave the
   * setup screen stuck showing "Setting up…" with no way to retry. */
  attempted: boolean;
  progressById: Record<string, ProvisionEvent>;
  failures: ProvisionFailure[];
  /** Once true, the blocking setup screen never reappears this session —
   * set after the user explicitly continues past it, even if some engine
   * never became ready (they can retry later from Settings). */
  dismissed: boolean;
  needsSetup: () => boolean;
  checkReadiness: () => Promise<void>;
  runSetup: () => Promise<void>;
  dismiss: () => void;
}

let unlistenProgress: (() => void) | undefined;

export const useProvisioningStore = create<ProvisioningState>((set, get) => ({
  checked: false,
  readiness: [],
  running: false,
  attempted: false,
  progressById: {},
  failures: [],
  dismissed: false,

  needsSetup: () => {
    const { checked, dismissed, readiness } = get();
    if (!checked || dismissed) return false;
    return readiness.some((e) => !e.ready);
  },

  checkReadiness: async () => {
    try {
      const readiness = await invoke<EngineReadiness[]>("get_engine_readiness");
      set({ readiness, checked: true });
    } catch {
      // If the check itself fails, don't block the app on a broken gate —
      // treat it as "already ready" and let the existing Conversion Engines
      // screen surface whatever is actually wrong.
      set({ checked: true, dismissed: true });
    }
  },

  runSetup: async () => {
    if (get().running) return;
    set({ running: true, attempted: true, progressById: {}, failures: [] });

    if (!unlistenProgress) {
      const fn = await listen<ProvisionEvent>("nexara://provisioning-progress", (event) => {
        set((state) => ({ progressById: { ...state.progressById, [event.payload.id]: event.payload } }));
      });
      unlistenProgress = fn;
    }

    try {
      const failures = await invoke<ProvisionFailure[]>("run_engine_provisioning");
      const readiness = await invoke<EngineReadiness[]>("get_engine_readiness");
      set({ failures, readiness, running: false });
    } catch (err) {
      set({ running: false, failures: [{ id: "unknown", message: String(err) }] });
    }
  },

  dismiss: () => set({ dismissed: true }),
}));
