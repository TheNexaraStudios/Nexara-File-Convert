import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { FormatInfo, RegistryResponse } from "../types/format";
import type { EngineInfo } from "../types/engine";

interface RegistryState {
  formats: FormatInfo[];
  formatsById: Record<string, FormatInfo>;
  conversions: Record<string, string[]>;
  engines: EngineInfo[];
  loading: boolean;
  error: string | null;
  load: () => Promise<void>;
  compatibleOutputs: (inputFormatId: string | null) => FormatInfo[];
  findByExtension: (extension: string) => FormatInfo | null;
}

export const useRegistryStore = create<RegistryState>((set, get) => ({
  formats: [],
  formatsById: {},
  conversions: {},
  engines: [],
  loading: true,
  error: null,

  load: async () => {
    set({ loading: true, error: null });
    try {
      const [registry, engines] = await Promise.all([
        invoke<RegistryResponse>("get_format_registry"),
        invoke<EngineInfo[]>("get_engine_status"),
      ]);
      const formatsById: Record<string, FormatInfo> = {};
      for (const fmt of registry.formats) formatsById[fmt.id] = fmt;
      set({
        formats: registry.formats,
        formatsById,
        conversions: registry.conversions,
        engines,
        loading: false,
      });
    } catch (err) {
      set({ loading: false, error: String(err) });
    }
  },

  compatibleOutputs: (inputFormatId) => {
    if (!inputFormatId) return [];
    const { conversions, formatsById } = get();
    const ids = conversions[inputFormatId] ?? [];
    return ids.map((id) => formatsById[id]).filter((f): f is FormatInfo => Boolean(f));
  },

  findByExtension: (extension) => {
    const ext = extension.toLowerCase();
    return get().formats.find((f) => f.extensions.includes(ext)) ?? null;
  },
}));
