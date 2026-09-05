import { useState } from "react";
import {
  Minimize2,
  ImageDown,
  AudioLines,
  Scaling,
  FileStack,
  Scissors,
  Images,
  FileOutput,
  FolderOpen,
  FolderArchive,
  Clapperboard,
  Info,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { resolveFiles } from "../../hooks/useFileDrop";
import { useJobStore } from "../../stores/useJobStore";
import { useNavStore } from "../../stores/useNavStore";
import { useToastStore } from "../../stores/useToastStore";
import type { ConversionSettings } from "../../types/job";
import { MergePdfPanel } from "./MergePdfPanel";
import { SplitPdfPanel } from "./SplitPdfPanel";
import { PdfToImagesPanel } from "./PdfToImagesPanel";
import { ExtractArchivePanel } from "./ExtractArchivePanel";
import { CreateArchivePanel } from "./CreateArchivePanel";
import { MetadataInspectorPanel } from "./MetadataInspectorPanel";
import "../../styles/screen.css";
import "./ToolsScreen.css";

type PanelKind = "mergePdf" | "splitPdf" | "pdfToImages" | "extractArchive" | "createArchive" | "metadata";

interface Tool {
  icon: typeof Minimize2;
  title: string;
  description: string;
  /** Formats that route straight to the Convert screen with this format
   * pre-selected — for tools that are genuinely just a single-file
   * conversion with a well-known target. */
  formatId?: string;
  settings?: Partial<ConversionSettings>;
  /** Tools with real multi-file/multi-output/read-only needs that don't
   * fit the single-conversion model open a dedicated panel instead. */
  panel?: PanelKind;
}

const TOOLS: Tool[] = [
  { icon: Minimize2, title: "Compress Video", description: "Shrink video files while preserving quality.", formatId: "mp4", settings: { preset: "small" } },
  { icon: ImageDown, title: "Compress Image", description: "Reduce image file size with minimal quality loss.", formatId: "jpg", settings: { preset: "small" } },
  { icon: AudioLines, title: "Extract Audio", description: "Pull the audio track out of any video.", formatId: "mp3" },
  { icon: Scaling, title: "Resize Image", description: "Exact width/height, percentage, or fit/fill — via the settings icon after adding a file." },
  { icon: FileStack, title: "Merge PDF", description: "Combine multiple PDFs into one document.", panel: "mergePdf" },
  { icon: Scissors, title: "Split PDF", description: "Extract a page range, or export every page separately.", panel: "splitPdf" },
  { icon: Images, title: "Images to PDF", description: "Combine images into a single PDF.", formatId: "pdf" },
  { icon: FileOutput, title: "PDF to Images", description: "Export any pages as PNG, JPG, or WebP, at any resolution.", panel: "pdfToImages" },
  { icon: FolderOpen, title: "Extract Archive", description: "Unpack ZIP, 7Z, TAR, GZ, or RAR safely.", panel: "extractArchive" },
  { icon: FolderArchive, title: "Create Archive", description: "Compress files and folders into a new archive.", panel: "createArchive" },
  { icon: Clapperboard, title: "Video to GIF", description: "Turn a video clip into an animated GIF.", formatId: "gif" },
  { icon: Info, title: "Metadata Inspector", description: "View real technical details about any file — read-only.", panel: "metadata" },
];

export function ToolsScreen() {
  const push = useToastStore((s) => s.push);
  const addFiles = useJobStore((s) => s.addFiles);
  const go = useNavStore((s) => s.go);
  const [openPanel, setOpenPanel] = useState<PanelKind | null>(null);

  const runTool = async (tool: Tool) => {
    if (tool.panel) {
      setOpenPanel(tool.panel);
      return;
    }
    if (tool.title === "Resize Image") {
      const selection = await open({ multiple: true });
      if (!selection) return;
      const paths = Array.isArray(selection) ? selection : [selection];
      const files = await resolveFiles(paths);
      if (files.length === 0) return;
      addFiles(files);
      go("convert");
      push("Files added — open the settings icon on each to set width, height, or crop mode.", "info");
      return;
    }
    if (!tool.formatId) {
      push(`${tool.title} will be available once its conversion engine is wired up.`, "info");
      return;
    }

    const selection = await open({ multiple: true });
    if (!selection) return;
    const paths = Array.isArray(selection) ? selection : [selection];
    const files = await resolveFiles(paths);
    if (files.length === 0) return;

    addFiles(files, { preferredFormatId: tool.formatId, preferredSettings: tool.settings });
    go("convert");
    push(files.length === 1 ? "File added" : `${files.length} files added`, "success");
  };

  return (
    <div className="screen-page">
      <header className="screen-page__header">
        <h1>Tools</h1>
        <p>Focused, single-purpose conversion tools — powered by the same local engines.</p>
      </header>

      <div className="tools-grid">
        {TOOLS.map((tool) => (
          <button key={tool.title} className="tool-card" onClick={() => runTool(tool)}>
            <div className="tool-card__icon">
              <tool.icon size={18} strokeWidth={1.7} />
            </div>
            <div className="tool-card__title">{tool.title}</div>
            <div className="tool-card__description">{tool.description}</div>
          </button>
        ))}
      </div>

      {openPanel === "mergePdf" && <MergePdfPanel onClose={() => setOpenPanel(null)} />}
      {openPanel === "splitPdf" && <SplitPdfPanel onClose={() => setOpenPanel(null)} />}
      {openPanel === "pdfToImages" && <PdfToImagesPanel onClose={() => setOpenPanel(null)} />}
      {openPanel === "extractArchive" && <ExtractArchivePanel onClose={() => setOpenPanel(null)} />}
      {openPanel === "createArchive" && <CreateArchivePanel onClose={() => setOpenPanel(null)} />}
      {openPanel === "metadata" && <MetadataInspectorPanel onClose={() => setOpenPanel(null)} />}
    </div>
  );
}
