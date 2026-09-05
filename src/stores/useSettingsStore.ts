import { create } from "zustand";
import { persist } from "zustand/middleware";

export type ThemePreference = "system" | "light" | "dark";
export type OutputLocationMode = "same-folder" | "ask" | "custom";
export type PerformanceMode = "eco" | "balanced" | "maximum";

interface SettingsState {
  theme: ThemePreference;
  outputLocationMode: OutputLocationMode;
  customOutputDir: string | null;
  performanceMode: PerformanceMode;
  hasCompletedFirstRun: boolean;
  setTheme: (theme: ThemePreference) => void;
  setOutputLocationMode: (mode: OutputLocationMode) => void;
  setCustomOutputDir: (dir: string | null) => void;
  setPerformanceMode: (mode: PerformanceMode) => void;
  markFirstRunSeen: () => void;
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      theme: "system",
      outputLocationMode: "same-folder",
      customOutputDir: null,
      performanceMode: "balanced",
      hasCompletedFirstRun: false,
      setTheme: (theme) => set({ theme }),
      setOutputLocationMode: (outputLocationMode) => set({ outputLocationMode }),
      setCustomOutputDir: (customOutputDir) => set({ customOutputDir }),
      setPerformanceMode: (performanceMode) => set({ performanceMode }),
      markFirstRunSeen: () => set({ hasCompletedFirstRun: true }),
    }),
    { name: "nexara-settings" }
  )
);
