# Nexara File Convert

A local, private, universal file converter for Windows. Drop in a file,
pick a target format, and Nexara converts it on your own machine — no
upload, no account, no cloud dependency.

Built with [Tauri 2](https://tauri.app/) (Rust) + React + TypeScript.

## What it does

Nexara detects a file's real type, shows only the output formats that
actually make sense for it, and routes the conversion to whichever local
engine handles that pair — FFmpeg for video/audio, ImageMagick for images
(including camera RAW), LibreOffice for documents, MuPDF for PDF
rasterization, 7-Zip for archives, Calibre for e-books, Inkscape for
vector graphics (including DXF), FontForge for fonts, and Pandoc for
plain text/HTML/Markdown. Nexara is fully self-contained: every engine is
either bundled directly inside the installer or fetched automatically
from its official source and hash-verified the first time it's needed —
nothing to install, configure, or add to `PATH` by hand. Settings →
Conversion Engines shows exactly what's ready, with a one-click retry for
anything that couldn't be set up (no internet on first launch, etc). No
conversion is ever reported as successful without validating the real
output file first. See
[THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) for exactly how each
engine is delivered and licensed.

## Requirements

- Windows 10/11
- [Node.js](https://nodejs.org/) 20.19+ (or 22.12+) and npm
- [Rust](https://www.rust-lang.org/tools/install) (stable, MSVC toolchain)
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (Desktop development with C++ workload)
- WebView2 Runtime (preinstalled on most Windows 11 systems)

None of the conversion engines above need to be pre-installed to build or
run Nexara — the app provisions them itself on first launch (bundled ones
are extracted instantly; fetched ones download from their official
source). A format category simply reports itself unavailable if its
engine hasn't finished provisioning yet.

## Development

```bash
npm install
npm run tauri dev
```

This launches a real native window (not a browser tab). Frontend changes
hot-reload; Rust changes trigger a rebuild.

## Building for production

```bash
npm run tauri build
```

Produces a Windows installer (NSIS `.exe` and MSI) under
`src-tauri/target/release/bundle/`, plus the standalone
`nexara-file-convert.exe` under `src-tauri/target/release/`.

See [`docs/build-windows.md`](docs/build-windows.md) for the full
production build and installer verification checklist.

## Architecture

Nexara never hard-codes "format X can become format Y" logic into the UI.
Instead, everything flows through one capability registry
(`src-tauri/src/conversion/registry.rs`) that answers "what can this file
become?" — the React frontend just asks it and renders whatever comes
back. Each engine (`src-tauri/src/conversion/{ffmpeg,image,office,pdf,
archive,ebook,vector,font,text}.rs`) implements building the right
command-line arguments and validating its own output; a shared `process.rs` helper
handles spawning, tracking, and cancelling the child process for engines
that don't need line-by-line progress parsing (FFmpeg is the one
exception, since it streams real percentage progress).

See [`docs/architecture.md`](docs/architecture.md) for the full picture,
and [`docs/conversion-engines.md`](docs/conversion-engines.md) /
[`docs/formats.md`](docs/formats.md) for what's implemented today.

## Adding a new format or engine

1. Register the format in `registry.rs` — extensions, category, display
   name, and which engine owns it.
2. Add its compatible output formats to the `conversions` map. Only list
   pairs you've actually verified work; the registry is what the UI trusts
   to decide what to show, so it should never overclaim.
3. If the engine is new, add a module under `src-tauri/src/conversion/`
   following the shape of `image.rs` or `office.rs` (build args, execute,
   validate output), wire it into `commands/mod.rs`'s engine dispatch, and
   add a health check in `engine.rs`.
4. If an output format's "default" engine can't actually produce it for a
   specific input format handled by a different engine (e.g. LibreOffice
   producing PDF, which is otherwise a dedicated future engine), add that
   pair to `engine_can_also_produce` in `commands/mod.rs` rather than
   changing the format's default engine.
5. Add unit tests for argument-building and output validation, plus a
   smoke test in `src-tauri/tests/` that runs the real engine against a
   small generated fixture — skipped, not failed, when the engine isn't
   installed.

## Testing

```bash
cd src-tauri
cargo test
```

Runs Rust unit tests (pure logic — argument building, output naming,
registry integrity, Zip Slip protection) plus integration tests that
invoke each *installed* engine for real against tiny generated fixtures
in `src-tauri/tests/fixtures/`. A test for an engine that isn't installed
on the machine running the suite is skipped with a message, not failed.

```bash
npx tsc --noEmit
```

Typechecks the frontend.

## Licenses

Nexara's own code, its `package.json`/`Cargo.toml` dependencies, and the
conversion engines it can optionally integrate with are documented in
[`THIRD_PARTY_LICENSES.md`](THIRD_PARTY_LICENSES.md).
