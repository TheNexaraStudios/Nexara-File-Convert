# Building and Verifying the Windows Production Build

## Prerequisites

Same as [development](../README.md#requirements): Node.js, Rust (MSVC
toolchain), and the Microsoft C++ Build Tools. Nothing else — Tauri
downloads and hash-verifies its own NSIS/WiX toolchains the first time
you build.

## Building

```bash
npm run tauri build
```

This runs the frontend production build (`vite build`), compiles the
Rust binary in release mode, and produces:

- `src-tauri/target/release/nexara-file-convert.exe` — the standalone
  binary.
- `src-tauri/target/release/bundle/nsis/Nexara File Convert_<version>_x64-setup.exe`
  — the NSIS installer (per-user install, no admin prompt — see
  `tauri.conf.json`'s `bundle.windows.nsis.installMode`).
- `src-tauri/target/release/bundle/msi/Nexara File Convert_<version>_x64_en-US.msi`
  — the WiX-based MSI, for environments that require MSI specifically
  (e.g. group policy deployment).

A full build takes a few minutes; the Rust compile dominates.

## Verification checklist

Run through this after every build whose changes could plausibly affect
packaging, path handling, or engine detection — not just before a
release.

1. **Install runs cleanly.** `& "...\<setup>.exe" /S` for a silent
   install, or double-click for the interactive flow. Confirm it lands
   under `%LOCALAPPDATA%\Nexara File Convert\` (per-user install mode —
   no UAC prompt).
2. **The installed app launches and looks right.** Correct window title,
   custom diamond-mark icon in the title bar and taskbar (not the Tauri
   default), sidebar navigation intact.
3. **First-run setup provisions every engine from the installed build**,
   not just `tauri dev`. On first launch, the Setup screen should appear
   and walk through every engine — bundled ones (7-Zip, ImageMagick,
   Inkscape, Pandoc, FFmpeg) finish almost instantly since they're just
   extracted from the installer's own resources; downloaded ones (MuPDF,
   Calibre, LibreOffice, FontForge) need real internet access and take
   longer. Once it finishes, Settings → Conversion Engines should show
   every engine "Detected" with no manual installation of anything.
   - **The one genuinely clean-machine-only test**: this checklist can't
     fully substitute for testing on a real clean Windows VM/PC with none
     of these engines preinstalled — a dev machine that already has, say,
     Calibre or LibreOffice installed will resolve those via the existing
     Program-Files fallback without ever exercising the real silent-install
     code path. Run the installed build in an actual clean VM at least
     once before a release to confirm the `msiexec`/Inno silent-install
     flows genuinely work end to end, not just the bundled-extraction ones.
   - If a tool you know is installed shows "Not found" outside of a clean
     first run, check `PATH` from a **freshly opened** shell first —
     Windows only broadcasts `PATH` changes to new processes, so a
     shell/terminal that was open before something installed won't see it
     either. This is an environment artifact, not an app bug, if a
     brand-new terminal resolves the tool fine.
4. **A real conversion succeeds end-to-end.** Pick one engine you have
   installed, convert a small file, and check the output on disk (not
   just "no error shown") — verify it opens or probes as the format it
   claims to be.
5. **Unicode and spaces in paths work correctly**, both directions. This
   is an explicit product requirement, not an edge case: users will have
   files under paths like
   `C:\Users\Test User\Desktop\Dosyalar\örnek video.mp4`. Verify with a
   real file:
   ```bash
   mkdir -p "/c/Users/<you>/Desktop/Test Klasörü İçin Dosyalar"
   ```
   Put a source file in there (e.g. a small PNG or MP4), convert it
   through the installed app, and confirm the output file lands in the
   same directory with the extension changed and the rest of the
   Unicode/space-containing name preserved byte-for-byte. This has been
   verified for the image (ImageMagick) and video (FFmpeg) engines
   against a folder named `Test Klasörü İçin Dosyalar` containing files
   named `örnek görüntü.png` and `örnek video.mp4` — both converted
   cleanly with the Turkish characters (ö, ü, İ, ç) intact in both the
   input and output filenames. This works because Tauri/Rust pass
   structured argument arrays to each engine's `Command`/`Child` (never
   a raw shell string), so `OsString`/`PathBuf` carry Unicode through
   untouched — but it's still verified empirically here rather than
   assumed, since this exact class of bug is exactly what naive
   shell-string construction would get wrong.
6. **Uninstall works.** Run `uninstall.exe` from the install directory
   (or via Windows Settings → Apps) and confirm the install directory is
   removed.

## A note on the two installed processes during manual testing

If you `Start-Process` the installed exe yourself while the NSIS
installer's own "launch after install" option was also left checked,
you'll end up with two independent windows — harmless (Nexara doesn't
use a single-instance lock), but don't mistake it for a bug if you see
two `nexara-file-convert.exe` processes after a silent install.
