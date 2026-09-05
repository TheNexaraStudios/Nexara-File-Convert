import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { invoke } from "@tauri-apps/api/core";
import { pathToFileName } from "../utils/format";

export interface DroppedFile {
  path: string;
  name: string;
  sizeBytes: number;
}

interface FileMeta {
  sizeBytes: number;
  isFile: boolean;
}

export async function resolveFiles(paths: string[]): Promise<DroppedFile[]> {
  const results = await Promise.all(
    paths.map(async (path) => {
      let sizeBytes = 0;
      try {
        const info = await invoke<FileMeta>("get_file_meta", { path });
        if (info.isFile) sizeBytes = info.sizeBytes;
      } catch {
        // Unreadable path (e.g. a dropped folder) — skip size, keep the entry.
      }
      return { path, name: pathToFileName(path), sizeBytes };
    })
  );
  return results.filter((r) => r.sizeBytes > 0 || !r.name.includes("."));
}

/**
 * Wires up native OS drag-and-drop from Explorer via the Tauri webview,
 * and exposes an `isDraggingOver` flag for the drop-zone's visual state.
 */
export function useFileDrop(onDrop: (files: DroppedFile[]) => void) {
  const [isDraggingOver, setIsDraggingOver] = useState(false);
  const onDropRef = useRef(onDrop);
  onDropRef.current = onDrop;

  useEffect(() => {
    // Outside a real Tauri webview (e.g. previewing the Vite dev server
    // directly in a browser) the __TAURI_INTERNALS__ bridge doesn't exist —
    // skip native drag-and-drop wiring rather than crashing the app.
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
      return;
    }

    let unlisten: (() => void) | undefined;
    let cancelled = false;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "over") {
          setIsDraggingOver(true);
        } else if (event.payload.type === "drop") {
          setIsDraggingOver(false);
          resolveFiles(event.payload.paths).then((files) => {
            if (files.length > 0) onDropRef.current(files);
          });
        } else {
          setIsDraggingOver(false);
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return { isDraggingOver };
}
