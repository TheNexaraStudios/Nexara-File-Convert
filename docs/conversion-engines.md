# Conversion Engines

Nexara integrates with local command-line tools rather than reimplementing
codecs, but the user never has to find, install, or configure any of
them — see [`../THIRD_PARTY_LICENSES.md`](../THIRD_PARTY_LICENSES.md) for
exactly how each one reaches the user's computer (bundled directly inside
Nexara's own installer, or fetched from its official source and verified
by hash the first time it's needed) and the `src-tauri/src/provisioning/`
module for the mechanism. Settings → Conversion Engines shows exactly what
Nexara found, and offers a "Re-run setup" action if anything didn't
provision successfully (no internet on first launch, a blocked download,
etc).

## How detection works

Every engine name (`ffmpeg`, `magick`, `mutool`, ...) resolves through
`conversion::engine::binary_path`, populated once at startup by
`init_resolved_binaries` (and refreshed after setup runs) in this priority
order:

1. **Bundled or downloaded, in Nexara's own app-data folder** — where
   `provisioning` extracts/installs everything (see above). This is the
   normal case on a clean machine.
2. **A well-known system install location** — LibreOffice, 7-Zip,
   Inkscape, FontForge, and Calibre's official Windows installers don't
   add themselves to `PATH`, so their standard Program Files path is
   checked directly. This matters for silently-installed engines
   (LibreOffice/Calibre/FontForge, which install to a real system
   location rather than app-data) and as a fallback if provisioning
   hasn't run yet.
3. **Plain `PATH` lookup** (`where`) — the original detection method, kept
   as the last resort for every engine.

Every conversion module reads its binary path through this single
resolver rather than hardcoding a bare name, so a bundled/downloaded
engine is used in preference to a coincidentally-already-installed system
copy — the whole point being that Nexara never silently depends on
whatever happens to already be on the machine.

An engine can show as "Detected" while a specific conversion still isn't
implemented (the `PREVIEW` badge) — availability and implementation are
tracked separately, so the UI never implies something works just because
the underlying tool happens to be installed.

## Engines

### FFmpeg — video & audio

Binary: `ffmpeg` + `ffprobe`. Bundled directly (the gyan.dev "full" build).

- Real progress via `-progress pipe:1`, computed from `out_time_ms` ÷
  probed duration.
- Remux-vs-transcode: if the source codecs are already compatible with
  the target container and no non-default quality/resolution/codec
  settings are requested, Nexara stream-copies (`-c copy`) instead of
  re-encoding — faster and lossless.
- Output validated by re-probing with `ffprobe`, not just checking the
  exit code.

### ImageMagick — raster images

Binary: `magick`. Bundled directly (official portable build).

- Quality presets map to `-quality`; resize maps to `-resize`; metadata
  stripping maps to `-strip`.
- JPEG output gets `-background white -flatten` automatically, so
  transparent source images don't turn black.
- Can write PDF directly from any image with no Ghostscript dependency —
  verified directly — but cannot *read* PDF without a Ghostscript
  delegate this build doesn't have, so PDF is an output-only extra
  capability for this engine, never an input.

### LibreOffice — documents, spreadsheets, presentations

Binary: `soffice.com`. Fetched and silently installed on first run (too
large to bundle, no official portable build) — resolved via its standard
Windows install location, since its installer doesn't add itself to
`PATH`.

- Headless `--convert-to`. LibreOffice names its own output file
  (`<input-stem>.<ext>`); Nexara predicts that name and moves it into the
  expected location afterward.
- Every invocation uses a **fresh, per-job profile directory**
  (`-env:UserInstallation=...`) nested inside that job's own temp folder.
  This isn't just isolation — a force-killed `soffice` process (which
  cancellation does) skips LibreOffice's own profile-lock cleanup, and a
  *reused* profile path would then hang every subsequent conversion
  waiting on a lock nobody will ever release. A throwaway profile per job
  sidesteps that entirely.
- Also handles PDF/TXT/HTML output from office documents directly (see
  the routing note below).

### MuPDF (`mutool`) — PDF rasterization

Binary: `mutool`. Fetched (not bundled — see the AGPL note in
`THIRD_PARTY_LICENSES.md`) and hash-verified on first run.

- PDF → PNG, first page only. Nexara's job model produces exactly one
  output file per conversion, so a multi-page PDF can't become "N images"
  in one job — the first page (the common thumbnail/preview case) is what
  is actually offered, rather than silently only ever doing that while
  implying full-document conversion.
- `mutool convert` always substitutes the page number into the output
  filename, even without an explicit pattern (`page.png` → `page1.png`).
  Nexara uses an explicit `page-%d.png` pattern and renames the resulting
  `page-1.png` into place.

### 7-Zip — archives

Binary: `7z`. Bundled directly — also the tool every other bundled
archive gets extracted with.

- Cross-format conversion is extract-then-recompress (7-Zip can't convert
  archive formats directly): list entries, extract, recompress into the
  target format.
- Every archive is **listed and validated before extraction**. Any entry
  with a `..` path segment or an absolute path (including a bare leading
  `/` or `\`, which Windows' own `Path::is_absolute()` doesn't flag) gets
  the whole archive rejected outright — Zip Slip protection, not
  best-effort sanitization.
- `.tar.gz`/`.tgz` input is unwrapped in two passes (gzip, then tar);
  `.tar.gz` output is built the same way in reverse, since gzip alone
  only wraps a single stream.
- RAR is read-only (7-Zip can extract it but never create it), matching
  the format registry — RAR never appears as a conversion target.

### Calibre (`ebook-convert`) — e-books

Binary: `ebook-convert`. Fetched (official MSI) and silently installed on
first run — too large to bundle.

- Straightforward `ebook-convert <input> <output>` — Calibre infers the
  target format from the output filename's extension.
- Also handles PDF/TXT output from e-books directly.

### Inkscape — vector graphics (and DXF)

Binary: `inkscape`. Bundled directly (official portable build).

- `inkscape <input> -o <output>`, same direct-output-naming pattern as
  Calibre.
- SVG/EPS/PS routing to PNG/PDF goes through Inkscape specifically rather
  than the image or PDF engines' defaults, which either can't read SVG at
  all or render it inconsistently.
- DXF is registered under this engine too, not a separate CAD engine.
  Inkscape imports DXF natively — verified directly with a hand-written
  fixture (a line and a circle): `inkscape sample.dxf -o out.svg` runs
  headlessly with no blocking dialog and renders real geometry, and the
  same command works for `-o out.pdf`. DWG (AutoCAD's proprietary format)
  is a different problem — see "Not yet implemented" below.

### FontForge — fonts

Binary: `fontforge`. Fetched (official installer) and silently installed
on first run — installs to a `FontForgeBuilds` directory rather than
`FontForge`, checked accordingly.

- Driven through FontForge's non-interactive scripting mode:
  `fontforge -lang=ff -c 'Open($1); Generate($2)' <input> <output>`. Like
  Inkscape and Calibre, `Generate()` infers the target format from the
  output path's extension, so no filename prediction/rename dance is
  needed afterwards.
- Verified directly against a minimal single-glyph TTF authored via
  FontForge's own scripting (so the test fixture carries no font-license
  baggage): TTF → OTF produces a real `OTTO`-tagged CFF font, TTF → WOFF
  produces a real `wOFF`-tagged font, and TTF → WOFF2 produces a real
  `wOF2`-tagged font with genuine compression.

### Pandoc — plain text, HTML, and Markdown

Binary: `pandoc`. Bundled directly (official static-binary build).

- `pandoc <input> [-t <writer>] -o <output>`. Pandoc infers both input and
  output format from file extensions correctly for HTML, DOCX, and EPUB —
  verified directly for every pair TXT/HTML/Markdown are registered for.
  Plain-text *output* is the one exception: with no explicit `-t`, a
  `.txt` target silently gets Pandoc's Markdown writer instead — verified
  directly (headings/bold/links came through as literal `#`/`**`/`[...]`
  syntax, not flattened prose) — so `.txt` targets always pass `-t plain`
  explicitly. `.md` targets pass `-t markdown` too, mostly for clarity
  since it already matches Pandoc's own inference.
- Pandoc alone can't produce PDF — verified directly:
  `pandoc in.md -o out.pdf` fails with `'pdflatex' not found`, since
  Pandoc's PDF writer shells out to a LaTeX engine this build doesn't
  have. PDF output instead hands off to LibreOffice (`convert_text_to_pdf`
  in `commands/mod.rs`): a non-HTML source is first normalized to HTML via
  Pandoc, then LibreOffice exports that HTML to PDF — LibreOffice can
  already do straight from HTML (verified). LibreOffice can *not* be
  handed Markdown directly, though: verified directly that headless
  `soffice` hangs indefinitely on `.md` input specifically (the identical
  content saved as `.txt` instead converts instantly), so the Pandoc
  normalization step is load-bearing, not just a convenience.
- This is the one conversion in the app that spans two engines for a
  single job. It still runs through the same job registry and
  cancellation path as every other conversion — each step registers its
  own child process in turn, so a cancel mid-pipeline stops whichever step
  is currently running.

## RAW images ride on the Image engine — not a separate one

CR2, CR3, NEF, ARW, DNG, RAF, ORF, and RW2 are registered under the
`image` engine (ImageMagick), not a dedicated `raw`/`dcraw` engine.
`magick -list delegate` shows no external dcraw/LibRaw process for any of
these formats — the decoder is compiled directly into ImageMagick's own
Windows build — so RAW input needs nothing beyond what the image engine
already does; `image.rs` required zero changes. One correction this
uncovered: DNG is read-only in ImageMagick (`magick -list format` lists
it, like every other RAW format here, as `r--`), so no RAW format may
list `dng` as a *convertible-to* target even though an earlier revision
of the registry did — fixed alongside wiring RAW up for real.

## Multi-engine routing

Some output formats are registered under one "default" engine but can
also be produced by whichever engine owns the *input* format:

| Input engine | Can also produce |
|---|---|
| `office` (LibreOffice) | `pdf`, `txt`, `html` |
| `ebook` (Calibre) | `pdf`, `txt` |
| `image` (ImageMagick) | `pdf` (write-only, see above) |
| `pdf` (MuPDF) | `png` (the PDF-input case, overriding image's normal PNG ownership) |
| `vector` (Inkscape) | `png`, `pdf` |

This list (`engine_can_also_produce` in `commands/mod.rs`) is intentionally
narrow and hand-verified — it exists to route *known-working* pairs
correctly, not to assume any engine can produce any format. RAW → image
and DXF → SVG don't need an entry here: both already share the same
engine as their outputs directly (`image` and `vector` respectively), so
the default routing already lands correctly. DXF → PDF is the one case
that does need the override, since PDF's own default engine is `pdf`
(MuPDF) — it's covered by the existing `vector` → `pdf` entry above.

## Not yet implemented

Ghostscript-dependent PDF operations (merge/split/optimize/page
extraction) and DWG (AutoCAD's proprietary format — real licensing
constraints, not just missing code) are represented in the registry and
engine list, correctly reported as unavailable, and simply not offered as
conversions yet.

With Pandoc wired up, every engine in the registry is now real except
DWG — 9 of the original 10 planned engines are implemented; DWG remains
out of scope for the reason above.
