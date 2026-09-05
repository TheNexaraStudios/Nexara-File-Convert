# Third-Party Software

Nexara File Convert is self-contained: every conversion engine it uses is
either bundled directly inside Nexara's own installer, or fetched
automatically from its official upstream source the first time it's
needed. Users never install, configure, or locate any conversion engine by
hand. This document records exactly what's bundled, what's fetched, the
exact pinned version and verified SHA-256 hash of each, and the license
obligations that come with redistributing it.

The full pinned version/URL/hash table, kept in sync with what's actually
shipped, lives in source at
[`src-tauri/src/provisioning/spec.rs`](src-tauri/src/provisioning/spec.rs) —
this document is the human-readable explanation of that table, not a
duplicate of it. If the two ever disagree, `spec.rs` is authoritative.

## How each engine reaches the user's computer

| Engine | Version | Delivery | Why |
|---|---|---|---|
| [7-Zip](https://www.7-zip.org/) | 26.03 | **Bundled** — `7z.exe`/`7z.dll` shipped directly, loose (unmodified, extracted from the official installer) | Tiny (~2.4 MB), permissive LGPL, and needed as the bootstrap tool that extracts every other bundled archive |
| [ImageMagick](https://imagemagick.org/) | 7.1.2-31 | **Bundled** — official portable `.7z` | Small (~12 MB), Apache 2.0 (no copyleft obligations at all) |
| [Inkscape](https://inkscape.org/) | 1.4.2 | **Bundled** — official portable `.7z` | Official portable build exists; GPL v3 but "mere aggregation" via subprocess invocation carries no linking obligation |
| [Pandoc](https://pandoc.org/) | 3.11 | **Bundled** — official static-binary `.zip` | Official static build exists; GPL v2+, same aggregation reasoning |
| [FFmpeg](https://ffmpeg.org/) | 9.0.1 (gyan.dev "full" build) | **Bundled** — official `.7z` | The full build (not an LGPL-only build) is bundled deliberately, to preserve real H.264/H.265 encoding via libx264/libx265 rather than dropping to a weaker codec set for a license technicality that doesn't change Nexara's own obligations either way (see note below) |
| [MuPDF](https://mupdf.com/) (`mutool`) | 1.28.0 | **Fetched at install/first-run** from Artifex's official GitHub release, hash-verified, extracted — never embedded in Nexara's own installer | AGPL v3, dual-licensed commercially by Artifex, who actively enforces it — a deliberately more conservative posture than the other engines here, even though embedding would likely also qualify as mere aggregation |
| [Calibre](https://calibre-ebook.com/) (`ebook-convert`) | 9.14.0 | **Fetched at install/first-run** from Calibre's official GitHub release (the same MSI calibre-ebook.com links to), hash-verified, silently installed | GPL v3; too large (~224 MB) to bundle in every copy of Nexara's installer |
| [LibreOffice](https://www.libreoffice.org/) | 26.2.6 | **Fetched at install/first-run** from The Document Foundation's own servers, hash-verified against TDF's own published checksum, silently installed | MPL v2.0; too large (~373 MB) to bundle, and no official portable build exists |
| [FontForge](https://fontforge.org/) | 2025-10-09 | **Fetched at install/first-run** from FontForge's official Windows build (mirrored via SourceForge, linked from fontforge.org), hash-verified, silently installed | GPL v3 (plus some BSD/MIT-licensed component libraries); no clean official portable build exists |

Every fetched download is verified against a pinned SHA-256 hash before
it's used — either computed directly from the exact file during
development (noted per engine below) or copied verbatim from the vendor's
own published checksum. A hash mismatch deletes the download and fails
provisioning rather than silently continuing with an unverified file.
Downloads are cached in Nexara's app-data folder so a repeat run (or a
retry after a failure) doesn't re-fetch anything already verified good.

## Why bundling a GPL/AGPL *binary* and invoking it as a subprocess is fine

Nexara never links against any of these engines' code, statically or
dynamically — it spawns each one as a wholly separate, unmodified process
and talks to it over stdin/stdout/exit codes, exactly as if the user had
installed it themselves and Nexara shelled out to a system binary. GPL,
LGPL, and AGPL all draw the same distinction: work that merely runs
alongside your program as a separate process, communicating over the same
narrow interfaces an unrelated program would use, is "mere aggregation" —
not a combined or derivative work — and does not extend copyleft
obligations onto Nexara's own source. This applies whether that binary
happens to live on the user's system already or is bundled inside Nexara's
own installer; bundling doesn't change the legal relationship, only the
distribution mechanics.

What bundling *does* still require, and what Nexara does for each bundled
GPL/LGPL/AGPL-family engine:

- Include the engine's own license text unmodified (see below).
- Never misrepresent the engine's origin or claim it as Nexara's own work.
- Make no attempt to restrict what the user may do with the bundled binary
  beyond its own license terms.
- For GPL/LGPL binaries specifically, either ship the corresponding source
  or provide a written offer, valid for at least three years, to provide
  it on request — satisfied here by linking directly to each project's own
  public source repository, since none of these builds carry any
  Nexara-specific patch.

## FFmpeg's GPL v3 obligations specifically

Nexara bundles gyan.dev's "full" Windows build of FFmpeg 9.0.1, built with
`--enable-gpl --enable-version3` and including `libx264`/`libx265` — this
makes the *build* GPL v3 licensed (an LGPL-only build exists but drops
real H.264/H.265 encoding, which would be a real functionality regression
just to sidestep a license technicality that doesn't change Nexara's own
license either way, per the aggregation reasoning above). Because this is
a GPL binary Nexara redistributes unmodified:

- **License text**: FFmpeg's GPL v3 license text is included below and
  ships in Nexara's installer alongside the binary.
- **Source availability**: FFmpeg's complete source is public at
  [git.ffmpeg.org](https://ffmpeg.org/download.html#get-sources); the
  exact source corresponding to this build is tagged `n9.0.1` in that
  repository. Nexara adds no patches of its own, so this upstream tag is
  the complete corresponding source.
- **No added restrictions**: Nexara imposes no license terms of its own
  on the bundled FFmpeg binary beyond what FFmpeg's own GPL v3 license
  already requires.

## A note on MuPDF's AGPL license and why it's fetched, not bundled

Artifex Software dual-licenses MuPDF under AGPL v3 or a commercial
license, and is known to actively enforce the AGPL terms against
commercial redistributors. Nexara's use — spawning the unmodified official
`mutool.exe` as a subprocess — is "mere aggregation" under AGPL's own
definition, the same reasoning that covers every other bundled engine
here. Given Artifex's enforcement posture specifically, Nexara takes the
more conservative path anyway: `mutool.exe` is never embedded in Nexara's
own installer. Instead, it's downloaded fresh from Artifex's own official
GitHub release the first time it's needed, with its SHA-256 verified
against the exact hash of the file Nexara's own developers downloaded and
tested against. If Nexara ever moves to linking against `libmupdf`
directly instead of shelling out to the CLI, that would be a combined
work under AGPL and would need Artifex's commercial license instead.

## Pinned versions and verified hashes

See
[`src-tauri/src/provisioning/spec.rs`](src-tauri/src/provisioning/spec.rs)
for the exact, currently-shipping version, download URL, SHA-256 hash, and
a one-line note on how each hash was obtained (self-computed from a real
download vs. copied from the vendor's own published checksum file) for
every engine listed above.

## Per-engine license texts

The full, unmodified license text for every bundled or fetched engine
ships inside Nexara itself (Settings → About → Third-Party Licenses) and
is also linked here for reference:

- **7-Zip** — [GNU LGPL v2.1](https://www.7-zip.org/license.txt) (the
  bundled RAR-unpacking code specifically carries an additional
  unRAR-license restriction against building a competing RAR compressor,
  which does not apply to Nexara's use)
- **ImageMagick** — [Apache License 2.0](https://imagemagick.org/script/license.php)
- **Inkscape** — [GNU GPL v3](https://www.gnu.org/licenses/gpl-3.0.html)
- **Pandoc** — [GNU GPL v2 or later](https://www.gnu.org/licenses/old-licenses/gpl-2.0.html)
- **FFmpeg** — [GNU GPL v3](https://www.gnu.org/licenses/gpl-3.0.html) (this specific build; see above)
- **MuPDF** — [GNU AGPL v3](https://www.gnu.org/licenses/agpl-3.0.html) ([commercial licensing](https://artifex.com/licensing/) available from Artifex)
- **Calibre** — [GNU GPL v3](https://www.gnu.org/licenses/gpl-3.0.html)
- **LibreOffice** — [Mozilla Public License 2.0](https://www.mozilla.org/en-US/MPL/2.0/)
- **FontForge** — [GNU GPL v3](https://www.gnu.org/licenses/gpl-3.0.html) (bundles some BSD/MIT-licensed component libraries — see FontForge's own `COPYING` file)

## Frontend and backend dependencies

Nexara's own source code is built on open-source libraries pulled in via
`npm` (frontend) and `cargo` (backend/Tauri). Their licenses are captured
in the standard package manifests:

- Frontend: see `package.json` and `package-lock.json`. Notable direct
  dependencies: React (MIT), Zustand (MIT), Lucide React icons (ISC),
  Vite (MIT), TypeScript (Apache-2.0).
- Backend: see `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`. Notable
  direct dependencies: Tauri (MIT/Apache-2.0), Tokio (MIT), Serde
  (MIT/Apache-2.0), Reqwest (MIT/Apache-2.0), Sha2 (MIT/Apache-2.0).

Run `cargo license` (from `src-tauri/`) or `npx license-checker` (from the
project root) to generate a complete, exact-version dependency license
report at any time.
