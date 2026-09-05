import {
  Film,
  Music,
  Image as ImageIcon,
  Aperture,
  FileText,
  Table,
  Presentation as PresentationIcon,
  BookOpen,
  Archive as ArchiveIcon,
  PenTool,
  Ruler,
  Type,
  FileCode,
  File as FileIcon,
} from "lucide-react";
import type { FormatCategory } from "../../types/format";

export const CATEGORY_ICONS: Record<FormatCategory, typeof FileIcon> = {
  video: Film,
  audio: Music,
  image: ImageIcon,
  "raw-image": Aperture,
  document: FileText,
  spreadsheet: Table,
  presentation: PresentationIcon,
  ebook: BookOpen,
  archive: ArchiveIcon,
  vector: PenTool,
  cad: Ruler,
  font: Type,
  "text-markup": FileCode,
};

export function iconForCategory(category: FormatCategory | undefined) {
  return (category && CATEGORY_ICONS[category]) || FileIcon;
}
