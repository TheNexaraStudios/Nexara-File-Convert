import { create } from "zustand";

export type Screen = "convert" | "queue" | "history" | "tools" | "settings" | "engines" | "about";

interface NavState {
  screen: Screen;
  go: (screen: Screen) => void;
}

export const useNavStore = create<NavState>((set) => ({
  screen: "convert",
  go: (screen) => set({ screen }),
}));
