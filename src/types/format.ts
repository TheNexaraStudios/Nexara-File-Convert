export type FormatCategory =
  | "video"
  | "audio"
  | "image"
  | "raw-image"
  | "document"
  | "spreadsheet"
  | "presentation"
  | "ebook"
  | "archive"
  | "vector"
  | "cad"
  | "font"
  | "text-markup";

export interface FormatInfo {
  id: string;
  extensions: string[];
  name: string;
  category: FormatCategory;
  engine: string;
  description: string;
}

export interface RegistryResponse {
  formats: FormatInfo[];
  conversions: Record<string, string[]>;
}

export const CATEGORY_LABELS: Record<FormatCategory, string> = {
  video: "Video",
  audio: "Audio",
  image: "Image",
  "raw-image": "RAW Image",
  document: "Document",
  spreadsheet: "Spreadsheet",
  presentation: "Presentation",
  ebook: "E-book",
  archive: "Archive",
  vector: "Vector",
  cad: "CAD",
  font: "Font",
  "text-markup": "Text / Markup",
};
