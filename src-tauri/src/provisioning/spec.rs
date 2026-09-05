//! Pinned, hash-verified specs for every third-party engine Nexara ships or
//! fetches. This is the single source of truth for exact versions, official
//! download URLs, and SHA-256 hashes — every hash here was either computed
//! directly from a real download during development, or copied verbatim
//! from the upstream vendor's own published checksum file (noted per entry).
//! Bumping a version means re-verifying and updating the hash here, never
//! just changing the URL.

/// How a fetched payload turns into a usable, resolvable binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadKind {
    /// A loose, already-uncompressed binary shipped directly as a bundle
    /// resource — used only for 7-Zip itself, since it's the tool every
    /// other archive on this list gets extracted with (no chicken-and-egg
    /// problem: it never needs extracting).
    LooseBinary,
    /// A `.7z` archive, extracted with our own bundled 7-Zip into the
    /// engine's app-data directory.
    SevenZipArchive,
    /// A `.zip` archive, extracted the same way (7-Zip reads zip natively).
    ZipArchive,
    /// A Windows Installer package, run silently via `msiexec /qn`.
    MsiInstaller,
    /// An Inno Setup installer, run silently via `/VERYSILENT`.
    InnoInstaller,
}

/// A third-party engine payload: either bundled directly inside Nexara's own
/// installer (small, permissively-licensed, or a hard functional
/// requirement like FFmpeg), or fetched on demand from its official upstream
/// source at install/first-run time. Every field is a `'static` primitive or
/// string slice, so this is cheaply `Copy` — handy for passing into a
/// progress closure that outlives the borrow of the static spec table.
#[derive(Clone, Copy)]
pub struct EngineSpec {
    /// Matches the ids used by `conversion::engine` / the capability
    /// registry (e.g. "ffmpeg", "magick", "mutool").
    pub id: &'static str,
    pub display_name: &'static str,
    pub version: &'static str,
    pub license: &'static str,
    /// True if this payload ships inside Nexara's own installer
    /// (`resources/engines/...`); false if it's fetched from `url` at
    /// install/first-run time instead.
    pub bundled: bool,
    /// Relative path under `resources/engines/` (when bundled) — matches
    /// what `tauri.conf.json` declares as a bundle resource.
    pub resource_relative: Option<&'static str>,
    /// Official upstream download URL (when not bundled).
    pub url: Option<&'static str>,
    pub sha256: &'static str,
    pub kind: PayloadKind,
    /// Path to the resolvable executable, relative to wherever the payload
    /// lands (the extraction root for archives; irrelevant for installers,
    /// which resolve through `conversion::engine`'s existing Program Files
    /// probing instead).
    pub exe_relative: Option<&'static str>,
    /// One-line note on why this hash is trusted — who published it and how
    /// it was obtained. Not read at runtime; feeds directly into
    /// THIRD_PARTY_LICENSES.md as the audit trail for each pinned hash.
    #[allow(dead_code)]
    pub hash_provenance: &'static str,
}

/// Engines bundled directly inside Nexara's own installer — small enough,
/// and permissively/compatibly licensed enough, to ship every time.
pub const BUNDLED: &[EngineSpec] = &[
    EngineSpec {
        id: "7z",
        display_name: "7-Zip",
        version: "26.03",
        license: "LGPL-2.1 (plus a BSD-style unRAR restriction on the bundled RAR-unpacking code)",
        bundled: true,
        resource_relative: Some("engines/7zip"),
        url: Some("https://github.com/ip7z/7zip/releases/download/26.03/7z2603-x64.exe"),
        sha256: "6ee3c0ed0b27663c1b948ae85a7c0bb073aed1498983182f3f0df1f6a8c30b2f",
        kind: PayloadKind::LooseBinary,
        exe_relative: Some("7z.exe"),
        hash_provenance: "Computed directly from the official 7-Zip installer (7z2603-x64.exe, github.com/ip7z/7zip) — 7z.exe and 7z.dll were extracted from it unmodified (the installer is itself a 7z SFX archive) and are shipped as loose files rather than re-compressed.",
    },
    EngineSpec {
        id: "magick",
        display_name: "ImageMagick",
        version: "7.1.2-31",
        license: "Apache License 2.0",
        bundled: true,
        resource_relative: Some("engines/imagemagick/ImageMagick-7.1.2-31-portable-Q16-HDRI-x64.7z"),
        url: Some("https://github.com/ImageMagick/ImageMagick/releases/download/7.1.2-31/ImageMagick-7.1.2-31-portable-Q16-HDRI-x64.7z"),
        sha256: "a6a83a77a5284a2cae5ca4a81d95e5fad21ecd56cdb647ee99f970e233504fff",
        kind: PayloadKind::SevenZipArchive,
        exe_relative: Some("magick.exe"),
        hash_provenance: "Computed directly from the official portable build published on ImageMagick's own GitHub release (github.com/ImageMagick/ImageMagick).",
    },
    EngineSpec {
        id: "inkscape",
        display_name: "Inkscape",
        version: "1.4.2",
        license: "GNU GPL v3",
        bundled: true,
        resource_relative: Some("engines/inkscape/inkscape-1.4.2-x64.7z"),
        url: Some("https://inkscape.org/gallery/item/56342/inkscape-1.4.2_2025-05-13_f4327f4-x64.7z"),
        sha256: "757e0358512630e65505a4cca89885369087f2908469439bc41b5a346604cbd4",
        kind: PayloadKind::SevenZipArchive,
        exe_relative: Some("inkscape/bin/inkscape.exe"),
        hash_provenance: "Computed directly from the official portable 7z build linked from inkscape.org's own release page.",
    },
    EngineSpec {
        id: "pandoc",
        display_name: "Pandoc",
        version: "3.11",
        license: "GNU GPL v2 or later",
        bundled: true,
        resource_relative: Some("engines/pandoc/pandoc-3.11-windows-x86_64.zip"),
        url: Some("https://github.com/jgm/pandoc/releases/download/3.11/pandoc-3.11-windows-x86_64.zip"),
        sha256: "2ab72baf2399450e148ddf7a2a8689806c42e1bba71862b57e220fd9b8456d3d",
        kind: PayloadKind::ZipArchive,
        exe_relative: Some("pandoc-3.11/pandoc.exe"),
        hash_provenance: "Computed directly from Pandoc's own official GitHub release (github.com/jgm/pandoc).",
    },
    EngineSpec {
        id: "ffmpeg",
        display_name: "FFmpeg",
        version: "9.0.1",
        license: "GNU GPL v3 (full build — includes libx264/libx265; see THIRD_PARTY_LICENSES.md for the required source offer)",
        bundled: true,
        resource_relative: Some("engines/ffmpeg/ffmpeg-9.0.1-full_build.7z"),
        url: Some("https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.1-full_build.7z"),
        sha256: "4b9c814cb07a1f90d05b768ef4eb2abbf89af94bbb924df5b7dbd6e64e1e2b96",
        kind: PayloadKind::SevenZipArchive,
        exe_relative: Some("ffmpeg-9.0.1-full_build/bin/ffmpeg.exe"),
        hash_provenance: "Copied verbatim from gyan.dev's own published `.sha256` file for this exact build, then confirmed by computing the hash of the downloaded archive ourselves — the two matched exactly.",
    },
];

/// The matching ffprobe entry lives alongside ffmpeg in the same archive —
/// kept as a constant here since it shares the ffmpeg spec's archive/hash
/// but resolves to a different executable.
pub const FFPROBE_EXE_RELATIVE: &str = "ffmpeg-9.0.1-full_build/bin/ffprobe.exe";

/// Engines fetched from their official upstream source at install time or
/// first run — too large to embed in every copy of Nexara's own installer,
/// and/or requiring a real OS-level install (registry, Start Menu entry)
/// rather than a simple extract.
pub const DOWNLOADED: &[EngineSpec] = &[
    EngineSpec {
        id: "mutool",
        display_name: "MuPDF",
        version: "1.28.0",
        license: "GNU AGPL v3 (also available under a commercial license from Artifex Software) — Nexara invokes the unmodified official binary as a separate process and never links against it, so this is \"mere aggregation\" under AGPL's own definition, not a combined work",
        bundled: false,
        resource_relative: None,
        url: Some("https://github.com/ArtifexSoftware/mupdf-downloads/releases/download/1.28.0/mupdf-1.28.0-windows.zip"),
        sha256: "a0ee869bf38ee66b19cd30c281d04e91215f812f0cb20177cd1f69c74c33eb22",
        kind: PayloadKind::ZipArchive,
        exe_relative: Some("mupdf-1.28.0-windows/mutool.exe"),
        hash_provenance: "Computed directly from Artifex's own official GitHub release (github.com/ArtifexSoftware/mupdf-downloads). Deliberately NOT embedded in Nexara's installer, given Artifex's AGPL enforcement posture — fetched fresh and hash-verified instead.",
    },
    EngineSpec {
        id: "ebook-convert",
        display_name: "Calibre",
        version: "9.14.0",
        license: "GNU GPL v3",
        bundled: false,
        resource_relative: None,
        url: Some("https://github.com/kovidgoyal/calibre/releases/download/v9.14.0/calibre-64bit-9.14.0.msi"),
        sha256: "4ccaf2a49a0069b5e78291ee7248dcd8967896d316d6432ddf657b6feae8f32d",
        kind: PayloadKind::MsiInstaller,
        exe_relative: None,
        hash_provenance: "Computed directly from Calibre's own official GitHub release (github.com/kovidgoyal/calibre) — the same MSI calibre-ebook.com links to.",
    },
    EngineSpec {
        id: "soffice",
        display_name: "LibreOffice",
        version: "26.2.6",
        license: "Mozilla Public License 2.0",
        bundled: false,
        resource_relative: None,
        url: Some("https://download.documentfoundation.org/libreoffice/stable/26.2.6/win/x86_64/LibreOffice_26.2.6_Win_x86-64.msi"),
        sha256: "f9877032fd908beb9c0ddf06df4af5c2e85f419c42e14876c4cce5aae5fb2660",
        kind: PayloadKind::MsiInstaller,
        exe_relative: None,
        hash_provenance: "Copied verbatim from The Document Foundation's own published `.sha256` file served alongside this exact MSI at download.documentfoundation.org.",
    },
    EngineSpec {
        id: "fontforge",
        display_name: "FontForge",
        version: "2025-10-09",
        license: "GNU GPL v3 (bundles some BSD/MIT-licensed component libraries)",
        bundled: false,
        resource_relative: None,
        url: Some("https://sourceforge.net/projects/fontforge.mirror/files/20251009/FontForge-2025-10-09-Windows-x64.exe/download"),
        sha256: "548523f08834e344bda69abb759e30c0f84a1a5ef9a5e965eb946d86a11118a3",
        kind: PayloadKind::InnoInstaller,
        exe_relative: None,
        hash_provenance: "Computed directly from the official FontForge Windows build linked from fontforge.org's own downloads page (mirrored via SourceForge, FontForge's official release host for Windows builds).",
    },
];

pub fn find(id: &str) -> Option<&'static EngineSpec> {
    BUNDLED.iter().find(|s| s.id == id).or_else(|| DOWNLOADED.iter().find(|s| s.id == id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spec_has_a_64_char_lowercase_hex_sha256() {
        for spec in BUNDLED.iter().chain(DOWNLOADED.iter()) {
            assert_eq!(spec.sha256.len(), 64, "{} hash is not 64 chars", spec.id);
            assert!(spec.sha256.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{} hash is not lowercase hex", spec.id);
        }
    }

    #[test]
    fn bundled_specs_have_a_resource_path_and_downloaded_specs_have_a_url() {
        for spec in BUNDLED {
            assert!(spec.bundled);
            assert!(spec.resource_relative.is_some(), "{} is bundled but has no resource path", spec.id);
        }
        for spec in DOWNLOADED {
            assert!(!spec.bundled);
            assert!(spec.url.is_some(), "{} is download-tier but has no URL", spec.id);
        }
    }

    #[test]
    fn archive_kinds_declare_an_exe_relative_path() {
        for spec in BUNDLED.iter().chain(DOWNLOADED.iter()) {
            if matches!(spec.kind, PayloadKind::SevenZipArchive | PayloadKind::ZipArchive) {
                assert!(spec.exe_relative.is_some(), "{} is an archive but has no exe_relative path", spec.id);
            }
        }
    }

    #[test]
    fn find_locates_both_bundled_and_downloaded_ids() {
        assert!(find("ffmpeg").is_some());
        assert!(find("mutool").is_some());
        assert!(find("does-not-exist").is_none());
    }

    #[test]
    fn ids_are_unique_across_both_tiers() {
        let mut ids: Vec<&str> = BUNDLED.iter().chain(DOWNLOADED.iter()).map(|s| s.id).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "duplicate engine id across BUNDLED/DOWNLOADED");
    }
}
