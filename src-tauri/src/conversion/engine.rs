use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EngineAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineInfo {
    pub id: String,
    pub name: String,
    pub binary: String,
    pub availability: EngineAvailability,
    /// Whether Nexara's conversion code for this engine has been wired up yet.
    /// An engine can be installed on the system (`availability: available`)
    /// while conversions through it are still being implemented.
    pub implemented: bool,
    pub description: String,
}

/// Resolves whether a binary exists on PATH without spawning it, so this
/// stays a fast, non-blocking startup check as required by the spec.
fn binary_on_path(binary: &str) -> bool {
    #[cfg(target_os = "windows")]
    let finder = "where";
    #[cfg(not(target_os = "windows"))]
    let finder = "which";

    Command::new(finder)
        .arg(binary)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// LibreOffice's official Windows installer does not add itself to PATH —
/// a well-known quirk — so a plain `where soffice` fails even on a normal
/// install. We fall back to the standard install locations. `soffice.com`
/// is used deliberately over `soffice.exe`: LibreOffice's own scripting
/// guidance calls out that the `.exe` launcher can detach/return before
/// the conversion finishes, while `.com` behaves like a normal console
/// child process.
pub fn resolve_soffice() -> Option<String> {
    for candidate in [
        r"C:\Program Files\LibreOffice\program\soffice.com",
        r"C:\Program Files (x86)\LibreOffice\program\soffice.com",
    ] {
        if Path::new(candidate).is_file() {
            return Some(candidate.to_string());
        }
    }
    if binary_on_path("soffice") {
        return Some("soffice".to_string());
    }
    None
}

/// Nexara bundles its own copy of 7-Zip (see `provisioning`), so this
/// resolves that first — falling back to a system install only if the
/// bundled copy is somehow missing (e.g. provisioning hasn't run yet).
/// 7-Zip's own official installer, like LibreOffice's, does not add itself
/// to PATH by default, hence the Program-Files fallback below.
pub fn resolve_7z() -> Option<String> {
    let resolved = binary_path("7z");
    if resolved != "7z" && Path::new(&resolved).is_file() {
        return Some(resolved);
    }
    for candidate in [r"C:\Program Files\7-Zip\7z.exe", r"C:\Program Files (x86)\7-Zip\7z.exe"] {
        if Path::new(candidate).is_file() {
            return Some(candidate.to_string());
        }
    }
    if binary_on_path("7z") {
        return Some("7z".to_string());
    }
    None
}

/// Nexara bundles its own copy of Inkscape (see `provisioning`), so this
/// resolves that first — falling back to a system install only if the
/// bundled copy is somehow missing. Inkscape's own official installer, like
/// LibreOffice's and 7-Zip's, does not add itself to PATH by default, hence
/// the Program-Files fallback below.
pub fn resolve_inkscape() -> Option<String> {
    let resolved = binary_path("inkscape");
    if resolved != "inkscape" && Path::new(&resolved).is_file() {
        return Some(resolved);
    }
    for candidate in [r"C:\Program Files\Inkscape\bin\inkscape.exe", r"C:\Program Files (x86)\Inkscape\bin\inkscape.exe"] {
        if Path::new(candidate).is_file() {
            return Some(candidate.to_string());
        }
    }
    if binary_on_path("inkscape") {
        return Some("inkscape".to_string());
    }
    None
}

/// FontForge's official Windows installer, like LibreOffice's, 7-Zip's, and
/// Inkscape's, does not add itself to PATH by default. Its installed
/// directory is named "FontForgeBuilds" (not "FontForge") on the current
/// release channel, so both are checked.
pub fn resolve_fontforge() -> Option<String> {
    for candidate in [
        r"C:\Program Files\FontForgeBuilds\bin\fontforge.exe",
        r"C:\Program Files\FontForge\bin\fontforge.exe",
        r"C:\Program Files (x86)\FontForgeBuilds\bin\fontforge.exe",
        r"C:\Program Files (x86)\FontForge\bin\fontforge.exe",
    ] {
        if Path::new(candidate).is_file() {
            return Some(candidate.to_string());
        }
    }
    if binary_on_path("fontforge") {
        return Some("fontforge".to_string());
    }
    None
}

/// Calibre's official installer, like the four above, does not add itself
/// to PATH by default. It installs to a folder named "Calibre2" — a
/// long-standing quirk of Calibre's own installer, not a typo — so that's
/// checked ahead of the plain "Calibre" a user might expect.
pub fn resolve_ebook_convert() -> Option<String> {
    for candidate in [r"C:\Program Files\Calibre2\ebook-convert.exe", r"C:\Program Files (x86)\Calibre2\ebook-convert.exe"] {
        if Path::new(candidate).is_file() {
            return Some(candidate.to_string());
        }
    }
    if binary_on_path("ebook-convert") {
        return Some("ebook-convert".to_string())
    }
    None
}

fn is_binary_available(binary: &str) -> bool {
    let resolved = binary_path(binary);
    if resolved != binary {
        // `binary_path` already found a concrete, resolved location (either
        // a bundled/downloaded app-data path, or one of the Program-Files
        // fallbacks below run ahead of time during startup) — trust it
        // directly instead of re-deriving the same answer.
        return Path::new(&resolved).is_file();
    }
    match binary {
        "soffice" => resolve_soffice().is_some(),
        "7z" => resolve_7z().is_some(),
        "inkscape" => resolve_inkscape().is_some(),
        "fontforge" => resolve_fontforge().is_some(),
        "ebook-convert" => resolve_ebook_convert().is_some(),
        other => binary_on_path(other),
    }
}

/// Whether `name` currently resolves to something real. Used by the
/// provisioning module to decide if an installer-based engine (soffice,
/// ebook-convert, fontforge — the three that need a real OS-level install
/// rather than a plain extract) is already installed, so it never re-runs
/// an installer unnecessarily.
pub fn is_installed(name: &str) -> bool {
    is_binary_available(name)
}

/// Global cache of resolved binary paths, populated once at startup by
/// `init_resolved_binaries` after Nexara's provisioning step has had a
/// chance to extract bundled engines and install/extract downloaded ones.
/// Every conversion module reads its binary path through `binary_path`
/// instead of hardcoding a bare name like `"ffmpeg"`, so a bundled or
/// downloaded engine resolves to its real app-data location instead of
/// depending on the engine happening to already be on system PATH.
static RESOLVED_BINARIES: OnceLock<std::sync::RwLock<HashMap<&'static str, String>>> = OnceLock::new();

pub fn init_resolved_binaries(app: &tauri::AppHandle) {
    let mut map = HashMap::new();

    for name in ["7z", "magick", "inkscape", "ffmpeg", "pandoc", "mutool"] {
        if let Some(path) = crate::provisioning::resolved_exe_path(app, name) {
            map.insert(name, path.to_string_lossy().to_string());
        }
    }
    if let Some(path) = crate::provisioning::resolved_ffprobe_path(app) {
        map.insert("ffprobe", path.to_string_lossy().to_string());
    }

    // Fall back to the existing Program-Files/PATH probing for anything
    // provisioning hasn't placed in app-data yet (a clean run before
    // first-run setup finishes) and for the three engines that install to
    // a real system location rather than app-data.
    if !map.contains_key("7z") {
        if let Some(p) = resolve_7z() {
            map.insert("7z", p);
        }
    }
    if !map.contains_key("inkscape") {
        if let Some(p) = resolve_inkscape() {
            map.insert("inkscape", p);
        }
    }
    if !map.contains_key("magick") && binary_on_path("magick") {
        map.insert("magick", "magick".to_string());
    }
    if !map.contains_key("ffmpeg") && binary_on_path("ffmpeg") {
        map.insert("ffmpeg", "ffmpeg".to_string());
    }
    if !map.contains_key("ffprobe") && binary_on_path("ffprobe") {
        map.insert("ffprobe", "ffprobe".to_string());
    }
    if !map.contains_key("pandoc") && binary_on_path("pandoc") {
        map.insert("pandoc", "pandoc".to_string());
    }
    if !map.contains_key("mutool") && binary_on_path("mutool") {
        map.insert("mutool", "mutool".to_string());
    }

    if let Some(p) = resolve_soffice() {
        map.insert("soffice", p);
    }
    if let Some(p) = resolve_fontforge() {
        map.insert("fontforge", p);
    }
    if let Some(p) = resolve_ebook_convert() {
        map.insert("ebook-convert", p);
    }

    let lock = RESOLVED_BINARIES.get_or_init(|| std::sync::RwLock::new(HashMap::new()));
    if let Ok(mut guard) = lock.write() {
        *guard = map;
    }
}

/// Resolves a binary name to its actual runnable path: a bundled or
/// downloaded engine's app-data location if `init_resolved_binaries` found
/// one, otherwise whatever it found via the existing Program-Files/PATH
/// probing. Falls back to the bare name unchanged if the global cache was
/// never initialized (unit/integration tests, which spawn real installed
/// binaries via plain PATH names exactly as before this existed). Callable
/// repeatedly — `init_resolved_binaries` can re-run after provisioning
/// finishes so a freshly-installed engine is picked up without restarting.
pub fn binary_path(name: &str) -> String {
    RESOLVED_BINARIES.get().and_then(|lock| lock.read().ok()).and_then(|guard| guard.get(name).cloned()).unwrap_or_else(|| name.to_string())
}

fn engine(id: &str, name: &str, binaries: &[&str], implemented: bool, description: &str) -> EngineInfo {
    let availability =
        if binaries.iter().all(|b| is_binary_available(b)) { EngineAvailability::Available } else { EngineAvailability::Unavailable };
    EngineInfo {
        id: id.to_string(),
        name: name.to_string(),
        binary: binaries.join(" + "),
        availability,
        implemented,
        description: description.to_string(),
    }
}

/// Runs a lightweight health check across every engine Nexara knows how to
/// use. Each check is a single `where`/`which` lookup (or, for LibreOffice,
/// a couple of file-existence checks), so this stays fast enough to run on
/// every app launch without blocking the UI.
pub fn detect_engines() -> Vec<EngineInfo> {
    vec![
        engine(
            "ffmpeg",
            "FFmpeg",
            &["ffmpeg", "ffprobe"],
            true,
            "Video and audio conversion, remuxing, and transcoding.",
        ),
        engine(
            "image",
            "Image Engine (ImageMagick)",
            &["magick"],
            true,
            "Raster image conversion and processing, including camera RAW decoding (CR2, CR3, NEF, ARW, RAF, ORF, RW2, DNG) via its built-in decoder — no separate dcraw/LibRaw install needed.",
        ),
        engine("office", "LibreOffice", &["soffice"], true, "Document, spreadsheet, and presentation conversion."),
        engine("pdf", "PDF Engine (MuPDF)", &["mutool"], true, "PDF page rasterization to image."),
        engine("archive", "7-Zip", &["7z"], true, "Archive creation and extraction."),
        engine("ebook", "Calibre", &["ebook-convert"], true, "E-book format conversion."),
        engine(
            "vector",
            "Inkscape",
            &["inkscape"],
            true,
            "Vector graphics conversion, including DXF drawing import (rendered to SVG/PDF).",
        ),
        engine("font", "FontForge", &["fontforge"], true, "Font format conversion (TTF, OTF, WOFF, WOFF2)."),
        engine(
            "cad",
            "CAD Engine (DWG)",
            &["oda-convert"],
            false,
            "AutoCAD DWG drawing conversion. DXF is already supported today via the Vector (Inkscape) engine above — DWG is a separate, proprietary format with real licensing constraints that keep it unimplemented.",
        ),
        engine(
            "text",
            "Pandoc",
            &["pandoc"],
            true,
            "Markup and plain-text document conversion (TXT, HTML, Markdown), including PDF output via a LibreOffice hand-off — Pandoc alone can't write PDF without a separate LaTeX install.",
        ),
    ]
}
