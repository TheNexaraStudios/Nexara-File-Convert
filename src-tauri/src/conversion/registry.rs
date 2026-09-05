use serde::Serialize;
use std::collections::HashMap;

/// The categories shown in the UI's format picker and used to group
/// the capability registry. Kept flat and serializable so the frontend
/// can render section headers directly from this value.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FormatCategory {
    Video,
    Audio,
    Image,
    RawImage,
    Document,
    Spreadsheet,
    Presentation,
    Ebook,
    Archive,
    Vector,
    Cad,
    Font,
    TextMarkup,
}

#[derive(Debug, Clone, Serialize)]
pub struct FormatInfo {
    pub id: String,
    pub extensions: Vec<String>,
    pub name: String,
    pub category: FormatCategory,
    /// Id of the engine that will perform this conversion (see `engine::EngineId`).
    pub engine: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegistryResponse {
    pub formats: Vec<FormatInfo>,
    /// input format id -> ordered list of compatible output format ids
    pub conversions: HashMap<String, Vec<String>>,
}

fn f(
    id: &str,
    extensions: &[&str],
    name: &str,
    category: FormatCategory,
    engine: &str,
    description: &str,
) -> FormatInfo {
    FormatInfo {
        id: id.to_string(),
        extensions: extensions.iter().map(|s| s.to_string()).collect(),
        name: name.to_string(),
        category,
        engine: engine.to_string(),
        description: description.to_string(),
    }
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// The single, authoritative catalog of formats Nexara knows about, and which
/// output formats are considered a meaningful/compatible target for each input.
///
/// This is the ONE capability registry referenced throughout the app: the UI
/// never hard-codes conversion possibilities, it always asks this registry.
pub fn build_registry() -> RegistryResponse {
    use FormatCategory::*;

    let formats = vec![
        // ---- Video ----
        f("mp4", &["mp4", "m4v"], "MPEG-4 Video", Video, "ffmpeg", "The most widely compatible video container."),
        f("mov", &["mov"], "QuickTime Video", Video, "ffmpeg", "Apple's QuickTime container."),
        f("mkv", &["mkv"], "Matroska Video", Video, "ffmpeg", "Open, flexible container supporting many codecs."),
        f("webm", &["webm"], "WebM Video", Video, "ffmpeg", "Open video format optimized for the web."),
        f("avi", &["avi"], "AVI Video", Video, "ffmpeg", "Legacy Windows video container."),
        f("gif", &["gif"], "Animated GIF", Image, "ffmpeg", "Looping animated image, also usable as a short clip export."),

        // ---- Audio ----
        f("mp3", &["mp3"], "MP3 Audio", Audio, "ffmpeg", "Universally compatible lossy audio."),
        f("wav", &["wav"], "WAV Audio", Audio, "ffmpeg", "Uncompressed PCM audio."),
        f("flac", &["flac"], "FLAC Audio", Audio, "ffmpeg", "Lossless compressed audio."),
        f("aac", &["aac", "m4a"], "AAC Audio", Audio, "ffmpeg", "Efficient lossy audio, common on Apple devices."),
        f("ogg", &["ogg"], "Ogg Vorbis Audio", Audio, "ffmpeg", "Open lossy audio format."),

        // ---- Image ----
        f("jpg", &["jpg", "jpeg"], "JPEG Image", Image, "image", "Common lossy photo format."),
        f("png", &["png"], "PNG Image", Image, "image", "Lossless image format with transparency."),
        f("webp", &["webp"], "WebP Image", Image, "image", "Modern format with strong compression."),
        f("avif", &["avif"], "AVIF Image", Image, "image", "Next-gen image format with excellent compression."),
        f("tiff", &["tiff", "tif"], "TIFF Image", Image, "image", "High-fidelity format used in publishing and archiving."),
        f("bmp", &["bmp"], "BMP Image", Image, "image", "Uncompressed Windows bitmap."),
        f("ico", &["ico"], "Icon", Image, "image", "Windows icon format."),
        f("heic", &["heic", "heif"], "HEIC Image", Image, "image", "High-efficiency format used by modern iPhones."),

        // ---- Raw Image ----
        // Decoded by ImageMagick's own built-in RAW support (verified via
        // `magick -list delegate`: none of these formats route through an
        // external dcraw/LibRaw binary, so they ride on the same "image"
        // engine as every other raster format) — see conversion-engines.md.
        f("cr2", &["cr2"], "Canon RAW (CR2)", RawImage, "image", "Canon camera raw sensor data."),
        f("cr3", &["cr3"], "Canon RAW (CR3)", RawImage, "image", "Canon camera raw sensor data."),
        f("nef", &["nef"], "Nikon RAW", RawImage, "image", "Nikon camera raw sensor data."),
        f("arw", &["arw"], "Sony RAW", RawImage, "image", "Sony camera raw sensor data."),
        f("dng", &["dng"], "Digital Negative", RawImage, "image", "Adobe's open raw archival format."),
        f("raf", &["raf"], "Fujifilm RAW", RawImage, "image", "Fujifilm camera raw sensor data."),
        f("orf", &["orf"], "Olympus RAW", RawImage, "image", "Olympus camera raw sensor data."),
        f("rw2", &["rw2"], "Panasonic RAW", RawImage, "image", "Panasonic camera raw sensor data."),

        // ---- Document ----
        f("docx", &["docx"], "Word Document", Document, "office", "Microsoft Word's modern document format."),
        f("doc", &["doc"], "Word 97-2003 Document", Document, "office", "Legacy Microsoft Word format."),
        f("odt", &["odt"], "OpenDocument Text", Document, "office", "Open standard word processing format."),
        f("rtf", &["rtf"], "Rich Text Format", Document, "office", "Portable formatted text document."),
        f("txt", &["txt"], "Plain Text", TextMarkup, "text", "Unformatted text."),
        f("html", &["html", "htm"], "HTML Document", TextMarkup, "text", "Web page markup."),
        f("pdf", &["pdf"], "PDF Document", Document, "pdf", "Portable Document Format."),

        // ---- Spreadsheet ----
        f("xlsx", &["xlsx"], "Excel Workbook", Spreadsheet, "office", "Microsoft Excel's modern spreadsheet format."),
        f("xls", &["xls"], "Excel 97-2003 Workbook", Spreadsheet, "office", "Legacy Microsoft Excel format."),
        f("ods", &["ods"], "OpenDocument Spreadsheet", Spreadsheet, "office", "Open standard spreadsheet format."),
        f("csv", &["csv"], "CSV", Spreadsheet, "office", "Comma-separated values."),

        // ---- Presentation ----
        f("pptx", &["pptx"], "PowerPoint Presentation", Presentation, "office", "Microsoft PowerPoint's modern format."),
        f("ppt", &["ppt"], "PowerPoint 97-2003", Presentation, "office", "Legacy Microsoft PowerPoint format."),
        f("odp", &["odp"], "OpenDocument Presentation", Presentation, "office", "Open standard presentation format."),

        // ---- E-book ----
        f("epub", &["epub"], "EPUB E-book", Ebook, "ebook", "Widely supported e-book format."),
        f("mobi", &["mobi"], "MOBI E-book", Ebook, "ebook", "Legacy Kindle e-book format."),
        f("azw3", &["azw3"], "AZW3 E-book", Ebook, "ebook", "Modern Kindle e-book format."),
        f("fb2", &["fb2"], "FictionBook", Ebook, "ebook", "XML-based e-book format."),

        // ---- Archive ----
        f("zip", &["zip"], "ZIP Archive", Archive, "archive", "Universally compatible archive format."),
        f("7z", &["7z"], "7-Zip Archive", Archive, "archive", "High-ratio compressed archive."),
        f("tar", &["tar"], "TAR Archive", Archive, "archive", "Unix-style archive container."),
        f("gz", &["gz", "tar.gz", "tgz"], "Gzip Archive", Archive, "archive", "Gzip-compressed archive."),
        f("rar", &["rar"], "RAR Archive", Archive, "archive", "Proprietary archive format (extraction only)."),

        // ---- Vector ----
        f("svg", &["svg"], "SVG Vector", Vector, "vector", "Scalable vector graphics for the web."),
        f("eps", &["eps"], "Encapsulated PostScript", Vector, "vector", "Legacy print/vector interchange format."),
        f("ps", &["ps"], "PostScript", Vector, "vector", "Page description language for print."),

        // ---- CAD ----
        // DXF is a plain-text vector interchange format Inkscape already
        // imports natively, so it rides on the "vector" engine rather than
        // a dedicated CAD one — see conversion-engines.md.
        f("dxf", &["dxf"], "Drawing Exchange Format", Cad, "vector", "Open CAD interchange format."),
        f("dwg", &["dwg"], "AutoCAD Drawing", Cad, "cad", "Proprietary AutoCAD format; conversion support is limited by licensing."),

        // ---- Font ----
        f("ttf", &["ttf"], "TrueType Font", Font, "font", "Widely supported outline font format."),
        f("otf", &["otf"], "OpenType Font", Font, "font", "Modern outline font format."),
        f("woff", &["woff"], "Web Open Font Format", Font, "font", "Compressed font format for the web."),
        f("woff2", &["woff2"], "WOFF2 Font", Font, "font", "Higher-compression successor to WOFF."),

        // ---- Markdown ----
        f("md", &["md", "markdown"], "Markdown", TextMarkup, "text", "Lightweight plain-text markup."),
    ];

    let mut conversions: HashMap<String, Vec<String>> = HashMap::new();

    // Video
    conversions.insert("mp4".into(), ids(&["mkv", "webm", "avi", "mov", "gif", "mp3", "wav", "aac"]));
    conversions.insert("mov".into(), ids(&["mp4", "webm", "mkv", "avi", "gif", "mp3", "wav", "aac"]));
    conversions.insert("mkv".into(), ids(&["mp4", "webm", "avi", "mov", "mp3", "wav", "aac"]));
    conversions.insert("webm".into(), ids(&["mp4", "mkv", "avi", "gif", "mp3", "wav"]));
    conversions.insert("avi".into(), ids(&["mp4", "mkv", "webm", "mov", "mp3", "wav"]));
    conversions.insert("gif".into(), ids(&["mp4", "webm", "png", "webp"]));

    // Audio
    conversions.insert("mp3".into(), ids(&["wav", "flac", "aac", "ogg"]));
    conversions.insert("wav".into(), ids(&["mp3", "flac", "aac", "ogg"]));
    conversions.insert("flac".into(), ids(&["mp3", "wav", "aac", "ogg"]));
    conversions.insert("aac".into(), ids(&["mp3", "wav", "flac", "ogg"]));
    conversions.insert("ogg".into(), ids(&["mp3", "wav", "flac", "aac"]));

    // Image
    conversions.insert("jpg".into(), ids(&["png", "webp", "avif", "tiff", "bmp", "ico", "pdf"]));
    conversions.insert("png".into(), ids(&["jpg", "webp", "avif", "tiff", "bmp", "ico", "pdf"]));
    conversions.insert("webp".into(), ids(&["jpg", "png", "avif", "tiff", "bmp"]));
    conversions.insert("avif".into(), ids(&["jpg", "png", "webp", "tiff"]));
    conversions.insert("tiff".into(), ids(&["jpg", "png", "webp", "bmp"]));
    conversions.insert("bmp".into(), ids(&["jpg", "png", "webp", "tiff"]));
    conversions.insert("ico".into(), ids(&["png", "jpg"]));
    conversions.insert("heic".into(), ids(&["jpg", "png", "webp", "avif", "tiff"]));

    // Raw
    // Every RAW format here is read-only in ImageMagick (verified via
    // `magick -list format`, all listed "r--") — including DNG itself, so
    // it can be a *source* for other RAW conversions but never a target.
    for raw in ["cr2", "cr3", "nef", "arw", "dng", "raf", "orf", "rw2"] {
        let outputs: Vec<String> = ["jpg", "png", "tiff"].into_iter().map(String::from).collect();
        conversions.insert(raw.into(), outputs);
    }

    // Document
    conversions.insert("docx".into(), ids(&["pdf", "odt", "rtf", "txt", "html", "epub"]));
    conversions.insert("doc".into(), ids(&["pdf", "docx", "odt", "txt"]));
    conversions.insert("odt".into(), ids(&["pdf", "docx", "rtf", "txt"]));
    conversions.insert("rtf".into(), ids(&["pdf", "docx", "odt", "txt"]));
    conversions.insert("txt".into(), ids(&["pdf", "docx", "html", "md"]));
    conversions.insert("html".into(), ids(&["pdf", "docx", "md", "txt"]));
    conversions.insert("md".into(), ids(&["html", "pdf", "docx", "epub"]));
    // Rasterizing to PNG (first page) is what's actually verified to work
    // without Ghostscript installed; PDF -> DOCX/TXT would need real text
    // extraction/OCR this build doesn't attempt, so it isn't offered.
    conversions.insert("pdf".into(), ids(&["png"]));

    // Spreadsheet
    conversions.insert("xlsx".into(), ids(&["pdf", "csv", "ods", "xls"]));
    conversions.insert("xls".into(), ids(&["xlsx", "pdf", "csv", "ods"]));
    conversions.insert("ods".into(), ids(&["xlsx", "pdf", "csv"]));
    conversions.insert("csv".into(), ids(&["xlsx", "ods", "pdf"]));

    // Presentation
    conversions.insert("pptx".into(), ids(&["pdf", "odp", "ppt"]));
    conversions.insert("ppt".into(), ids(&["pptx", "pdf", "odp"]));
    conversions.insert("odp".into(), ids(&["pptx", "pdf"]));

    // Ebook
    conversions.insert("epub".into(), ids(&["mobi", "azw3", "pdf", "fb2", "txt"]));
    conversions.insert("mobi".into(), ids(&["epub", "azw3", "pdf"]));
    conversions.insert("azw3".into(), ids(&["epub", "mobi", "pdf"]));
    conversions.insert("fb2".into(), ids(&["epub", "mobi", "pdf"]));

    // Archive
    conversions.insert("zip".into(), ids(&["7z", "tar", "gz"]));
    conversions.insert("7z".into(), ids(&["zip", "tar", "gz"]));
    conversions.insert("tar".into(), ids(&["zip", "7z", "gz"]));
    conversions.insert("gz".into(), ids(&["zip", "7z", "tar"]));
    conversions.insert("rar".into(), ids(&["zip", "7z"])); // extraction only, then re-archive

    // Vector
    conversions.insert("svg".into(), ids(&["png", "pdf", "eps"]));
    conversions.insert("eps".into(), ids(&["svg", "png", "pdf"]));
    conversions.insert("ps".into(), ids(&["pdf", "svg"]));

    // CAD
    conversions.insert("dxf".into(), ids(&["svg", "pdf"]));
    conversions.insert("dwg".into(), ids(&[])); // unsupported today, listed for transparency only

    // Font
    conversions.insert("ttf".into(), ids(&["otf", "woff", "woff2"]));
    conversions.insert("otf".into(), ids(&["ttf", "woff", "woff2"]));
    conversions.insert("woff".into(), ids(&["ttf", "otf", "woff2"]));
    conversions.insert("woff2".into(), ids(&["ttf", "otf", "woff"]));

    RegistryResponse { formats, conversions }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_conversion_references_a_registered_format() {
        let reg = build_registry();
        let known_ids: HashSet<&str> = reg.formats.iter().map(|f| f.id.as_str()).collect();

        for (input_id, outputs) in &reg.conversions {
            assert!(known_ids.contains(input_id.as_str()), "conversions key '{input_id}' has no matching format entry");
            for output_id in outputs {
                assert!(
                    known_ids.contains(output_id.as_str()),
                    "conversion target '{output_id}' (from '{input_id}') has no matching format entry"
                );
            }
        }
    }

    #[test]
    fn no_format_lists_itself_as_an_output() {
        let reg = build_registry();
        for (input_id, outputs) in &reg.conversions {
            assert!(!outputs.contains(input_id), "'{input_id}' should not be its own conversion target");
        }
    }

    #[test]
    fn format_ids_are_unique() {
        let reg = build_registry();
        let mut ids: Vec<&str> = reg.formats.iter().map(|f| f.id.as_str()).collect();
        let original_len = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), original_len, "duplicate format id detected in the registry");
    }

    #[test]
    fn ffmpeg_backed_video_formats_have_non_empty_outputs() {
        let reg = build_registry();
        for id in ["mp4", "mov", "mkv", "webm", "avi", "mp3", "wav", "flac", "aac"] {
            let outputs = reg.conversions.get(id).unwrap_or_else(|| panic!("missing conversions entry for '{id}'"));
            assert!(!outputs.is_empty(), "'{id}' should have at least one compatible output");
        }
    }

    #[test]
    fn raw_formats_are_owned_by_the_image_engine() {
        // Verified via `magick -list delegate`: none of these route through
        // an external dcraw/LibRaw binary, so they ride on ImageMagick's own
        // built-in decoder rather than a separate "raw" engine.
        let reg = build_registry();
        for id in ["cr2", "cr3", "nef", "arw", "dng", "raf", "orf", "rw2"] {
            let format = reg.formats.iter().find(|f| f.id == id).unwrap_or_else(|| panic!("missing format entry for '{id}'"));
            assert_eq!(format.engine, "image", "'{id}' should be owned by the image engine");
        }
    }

    #[test]
    fn raw_formats_never_target_dng_as_output() {
        // Regression test: DNG is read-only in ImageMagick ("r--" in
        // `magick -list format`), so no RAW format's output list may
        // include it even though earlier revisions of this registry did.
        let reg = build_registry();
        for id in ["cr2", "cr3", "nef", "arw", "dng", "raf", "orf", "rw2"] {
            let outputs = reg.conversions.get(id).unwrap_or_else(|| panic!("missing conversions entry for '{id}'"));
            assert!(!outputs.contains(&"dng".to_string()), "'{id}' should not list dng as a convertible target");
            assert!(!outputs.is_empty(), "'{id}' should have at least one compatible output");
        }
    }

    #[test]
    fn dxf_is_owned_by_the_vector_engine() {
        // Inkscape imports DXF natively, so it rides on the same engine as
        // SVG/EPS/PS rather than a dedicated CAD engine.
        let reg = build_registry();
        let dxf = reg.formats.iter().find(|f| f.id == "dxf").unwrap();
        assert_eq!(dxf.engine, "vector");
        assert_eq!(reg.conversions.get("dxf").unwrap(), &vec!["svg".to_string(), "pdf".to_string()]);
    }

    #[test]
    fn dwg_is_registered_but_offers_no_conversions() {
        // DWG is proprietary and genuinely unsupported today — listed for
        // transparency, but it must never claim a working conversion path.
        let reg = build_registry();
        assert!(reg.formats.iter().any(|f| f.id == "dwg"));
        assert!(reg.conversions.get("dwg").unwrap().is_empty());
    }
}
