import { useEffect } from "react";
import { useSettingsStore } from "../stores/useSettingsStore";

/**
 * Applies the resolved theme (light/dark) to the document root as
 * `data-theme`, following the user's preference or the OS setting when
 * preference is "system". Keeps the DOM in sync as the OS theme changes.
 */
export function useTheme() {
  const theme = useSettingsStore((s) => s.theme);

  useEffect(() => {
    const root = document.documentElement;
    const media = window.matchMedia("(prefers-color-scheme: dark)");

    const apply = () => {
      if (theme === "system") {
        root.setAttribute("data-theme", "system");
        root.setAttribute("data-system-theme", media.matches ? "dark" : "light");
      } else {
        root.setAttribute("data-theme", theme);
        root.removeAttribute("data-system-theme");
      }
    };

    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
}
