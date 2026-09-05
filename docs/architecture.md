# Architecture

## The core principle: one capability registry

Nexara never hard-codes "PNG can become JPG" logic into a component. Every
screen that needs to know what a file can become asks one source of truth:
`src-tauri/src/conversion/registry.rs`. It returns two things over the
`get_format_registry` Tauri command:

- `formats`: every format Nexara knows about (id, extensions, display
  name, category, which engine owns it, a short description).
- `conversions`: a map from input format id to the list of output format
  ids that are actually valid targets for it.

The frontend's format picker, file card, and settings panel all just read
from this. If a pair isn't in `conversions`, it never appears as an
option — there's no separate "is this really supported" check scattered
through the UI, because the registry *is* that check.

```
UI: "what can sample.docx become?"
  → registry.conversions["docx"] → ["pdf", "odt", "rtf", "txt", "html", "epub"]
```

## Engines

Each conversion tool Nexara can drive lives in its own module under
`src-tauri/src/conversion/`:

| Module | Engine | What it does |
|---|---|---|
| `ffmpeg.rs` | FFmpeg | Video/audio: build args, stream real `-progress` output, remux-vs-transcode decision |
| `image.rs` | ImageMagick | Raster images: quality/resize/strip-metadata args |
| `office.rs` | LibreOffice | Documents/spreadsheets/presentations, headless, isolated per-job profile |
| `pdf.rs` | MuPDF (`mutool`) | PDF → PNG (first page) |
| `archive.rs` | 7-Zip | Extract → recompress, with Zip Slip validation before extraction |
| `ebook.rs` | Calibre | E-book formats |
| `vector.rs` | Inkscape | SVG/EPS/PS, and DXF (Inkscape imports it natively) |
| `font.rs` | FontForge | TTF/OTF/WOFF/WOFF2 |
| `text.rs` | Pandoc | TXT/HTML/Markdown, plus a LibreOffice hand-off for PDF output (see below) |

Camera RAW formats (CR2, NEF, ARW, DNG, and others) don't get their own
module — ImageMagick decodes them natively, so they're just more input
formats for `image.rs`.

Text-format PDF output is the one conversion that spans two engines:
`commands::convert_text_to_pdf` normalizes the source to HTML via Pandoc
(skipped if it's already HTML), then hands that HTML to LibreOffice for
the actual PDF export — reusing `office::build_args`/`office::execute`
directly rather than duplicating them. Pandoc can't write PDF itself
without a separate LaTeX install, and LibreOffice hangs headless on
Markdown input specifically (both verified directly), so neither engine
alone covers this pair.

Each module exposes the same shape: a pure `build_args` function (easy to
unit test without spawning anything), an `execute` function that runs the
process, and a `validate_output` function that checks the result is
genuinely valid — not just "the process exited 0".

Engines that don't stream progress (everything except FFmpeg) share one
low-level runner, `process::run_and_track`, which spawns the child,
registers it in the job registry for cancellation, drains stdout/stderr
concurrently (required — an unread pipe fills up and deadlocks the child),
and reports whether it completed, failed, or was cancelled.

## Engine provisioning

`src-tauri/src/provisioning/` is what makes Nexara self-contained: every
engine is either bundled inside Nexara's own installer or fetched from its
official source on first run, so the user never installs, configures, or
locates a conversion engine by hand. `spec.rs` is the single pinned table
(exact version, official URL, verified SHA-256, and whether it's bundled
or fetched) every other module reads from — see
[`THIRD_PARTY_LICENSES.md`](../THIRD_PARTY_LICENSES.md) for the license
reasoning behind each engine's tier.

`provision_one` dispatches on the spec's `PayloadKind`:

- **Bundled archives** (ImageMagick, Inkscape, Pandoc, FFmpeg) extract
  straight from Nexara's own resource directory using the one loose
  bundled binary, 7-Zip, as the universal extractor.
- **Downloaded archives** (MuPDF) download first (streamed, retried,
  hash-verified against the pinned value, cached), then extract the same
  way.
- **Downloaded installers** (Calibre, LibreOffice, FontForge) download
  then run fully silently (`msiexec /qn` or Inno's `/VERYSILENT`) to their
  normal default install location — genuinely installed, not just
  extracted, since these three need a real OS-level install.

Every function in `provisioning` is generic over `tauri::Runtime` rather
than the concrete desktop runtime, specifically so the real pipeline
(resource resolution, extraction, download, hash verification) can be
exercised in integration tests without a live window.

`conversion::engine::binary_path(name)` is the one place every conversion
module resolves a binary name to a real path — checked once at startup
(`init_resolved_binaries`, re-run after setup completes) in priority
order: provisioned app-data location, then the existing Program-Files
probing (for the three silently-installed engines, and as a fallback),
then plain `PATH`. No conversion module hardcodes a bare binary name
anymore; they all read through this resolver, so a bundled/downloaded
engine is preferred over a coincidentally pre-installed system copy.

The frontend gates on this at launch: `useProvisioningStore.checkReadiness`
calls `get_engine_readiness`, and if anything isn't ready yet, `App.tsx`
renders `SetupScreen` instead of the normal UI — a small blocking screen
that calls `run_engine_provisioning`, shows live per-engine progress via
the `nexara://provisioning-progress` event stream, and lets the user
continue once everything's ready (or continue anyway if something
couldn't be reached, with a retry available later from Settings →
Conversion Engines).

## Job registry and cancellation

`conversion::jobs::JobRegistry` is a `Mutex<HashMap<job_id, Arc<Mutex<Child>>>>`
managed as Tauri state. A conversion registers its child process there
right after spawning; `cancel_conversion` looks the job up by id and kills
it. The killed job's own `execute()` call detects that its registry entry
is already gone (removed by the cancel handler) and reports the outcome as
`Cancelled` rather than `Failed`, regardless of what exit code a killed
process happens to report.

On Windows, cancellation kills the whole process tree (`taskkill /T /F`),
not just the direct child — necessary because LibreOffice's `soffice.com`
launcher spawns a separate `soffice.bin` backend process, and killing only
the launcher would leave the backend running.

## Multi-engine routing overrides

A format is registered under one "default" engine, but real tools don't
respect that cleanly — LibreOffice can export PDF directly from a Word
doc, even though PDF is conceptually a dedicated future engine (for
merge/split/rasterize operations). `commands::engine_can_also_produce` is
a small, explicit, hand-verified table of these exceptions. It's
deliberately not a blanket "try the input engine for anything" rule:
every entry reflects a specific pair someone actually tested, so the
routing never claims a conversion path that hasn't been verified.

## Conversion lifecycle

1. Frontend calls `start_conversion` with the input path, target format,
   output directory, and per-format settings.
2. `commands::start_conversion` resolves which engine actually owns this
   (input, output) pair, creates a per-job temp directory
   (`%TEMP%\NexaraFileConvert\<job_id>\`), and dispatches to that engine's
   `convert_with_*` function.
3. The engine writes into the temp directory — never directly to the
   user's chosen output location — so a failed or cancelled conversion
   never leaves a partial file where the user would see it.
4. On success, `ConversionSetup::finalize` re-validates the output (magic
   bytes, or a real re-probe via the engine itself), resolves a
   collision-free final filename (`video (1).mp4`, never silently
   overwriting), and moves the file into place.
5. The temp directory is always removed afterward, success or failure.

## Frontend state

Zustand stores, one per concern: `useRegistryStore` (the capability
registry, fetched once at startup), `useProvisioningStore` (engine
readiness and the setup-screen gate), `useJobStore` (pending files and
active/completed jobs), `useSettingsStore` (persisted preferences),
`useNavStore` / `useToastStore` / `useCommandPaletteStore` (UI-only
state). Real-time progress arrives as Tauri events rather than polling:
`nexara://conversion-progress` for conversions, `nexara://provisioning-progress`
for engine setup.
