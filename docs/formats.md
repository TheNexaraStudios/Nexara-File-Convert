# Supported Formats

This is a snapshot of `src-tauri/src/conversion/registry.rs` — the single
source of truth the app itself reads from (via `get_format_registry`). If
this table and the running app ever disagree, trust the app; update this
file to match.

Status:
- **Available** — the backing engine is implemented and (once provisioned
  — automatic, see `THIRD_PARTY_LICENSES.md`) performs this conversion for
  real today.
- **Awaiting engine** — the format is registered (so the reasoning behind
  what *would* be supported is transparent) but no engine implements it
  yet. It never appears as a choice in the UI.

## Video (FFmpeg — available)

| Input | Can convert to |
|---|---|
| MP4 | MKV, WebM, AVI, MOV, GIF, MP3, WAV, AAC |
| MOV | MP4, WebM, MKV, AVI, GIF, MP3, WAV, AAC |
| MKV | MP4, WebM, AVI, MOV, MP3, WAV, AAC |
| WebM | MP4, MKV, AVI, GIF, MP3, WAV |
| AVI | MP4, MKV, WebM, MOV, MP3, WAV |
| GIF | MP4, WebM, PNG, WebP |

## Audio (FFmpeg — available)

| Input | Can convert to |
|---|---|
| MP3 | WAV, FLAC, AAC, OGG |
| WAV | MP3, FLAC, AAC, OGG |
| FLAC | MP3, WAV, AAC, OGG |
| AAC | MP3, WAV, FLAC, OGG |
| OGG | MP3, WAV, FLAC, AAC |

## Image (ImageMagick — available)

| Input | Can convert to |
|---|---|
| JPG | PNG, WebP, AVIF, TIFF, BMP, ICO, PDF |
| PNG | JPG, WebP, AVIF, TIFF, BMP, ICO, PDF |
| WebP | JPG, PNG, AVIF, TIFF, BMP |
| AVIF | JPG, PNG, WebP, TIFF |
| TIFF | JPG, PNG, WebP, BMP |
| BMP | JPG, PNG, WebP, TIFF |
| ICO | PNG, JPG |
| HEIC | JPG, PNG, WebP, AVIF, TIFF |

PDF output is write-only for this engine (no Ghostscript delegate is
required to *write* PDF, but one would be to *read* it — see
[conversion-engines.md](conversion-engines.md)), so PDF never appears as
an image-engine input.

## Raw Image (ImageMagick — available)

| Input | Can convert to |
|---|---|
| CR2, CR3, NEF, ARW, DNG, RAF, ORF, RW2 | JPG, PNG, TIFF |

Decoded by ImageMagick's own built-in RAW support — no separate
dcraw/LibRaw install needed (see
[conversion-engines.md](conversion-engines.md)). DNG can be a source but
never a target: ImageMagick can only read it, not write it.

## Document (LibreOffice, with PDF/MuPDF cross-engine routing — available)

| Input | Can convert to |
|---|---|
| DOCX | PDF, ODT, RTF, TXT, HTML, EPUB |
| DOC | PDF, DOCX, ODT, TXT |
| ODT | PDF, DOCX, RTF, TXT |
| RTF | PDF, DOCX, ODT, TXT |
| PDF | PNG (first page, via MuPDF) |

DOCX/ODT/RTF → EPUB routes through LibreOffice; PDF → PNG is the one
document conversion MuPDF (not LibreOffice) performs. TXT/HTML/Markdown
are covered separately below (Pandoc).

## Plain text, HTML & Markdown (Pandoc, with a LibreOffice hand-off for PDF — available)

| Input | Can convert to |
|---|---|
| TXT | PDF, DOCX, HTML, MD |
| HTML | PDF, DOCX, MD, TXT |
| MD | HTML, PDF, DOCX, EPUB |

Driven through Pandoc's CLI. PDF output is the one pair that needs a
second engine: Pandoc can't write PDF without a separate LaTeX install, so
the source is normalized to HTML via Pandoc first (skipped if it's
already HTML) and handed to LibreOffice for the actual export — see
[conversion-engines.md](conversion-engines.md) for why (LibreOffice hangs
headless on Markdown input specifically, but not on identical content
saved as `.txt`).

## Spreadsheet (LibreOffice — available)

| Input | Can convert to |
|---|---|
| XLSX | PDF, CSV, ODS, XLS |
| XLS | XLSX, PDF, CSV, ODS |
| ODS | XLSX, PDF, CSV |
| CSV | XLSX, ODS, PDF |

## Presentation (LibreOffice — available)

| Input | Can convert to |
|---|---|
| PPTX | PDF, ODP, PPT |
| PPT | PPTX, PDF, ODP |
| ODP | PPTX, PDF |

## E-book (Calibre, with PDF cross-engine routing — available)

| Input | Can convert to |
|---|---|
| EPUB | MOBI, AZW3, PDF, FB2, TXT |
| MOBI | EPUB, AZW3, PDF |
| AZW3 | EPUB, MOBI, PDF |
| FB2 | EPUB, MOBI, PDF |

## Archive (7-Zip — available)

| Input | Can convert to |
|---|---|
| ZIP | 7Z, TAR, GZ |
| 7Z | ZIP, TAR, GZ |
| TAR | ZIP, 7Z, GZ |
| GZ | ZIP, 7Z, TAR |
| RAR | ZIP, 7Z (extraction only — 7-Zip can read RAR but never write it) |

Every archive is listed and validated for path-traversal (Zip Slip)
entries before extraction; an unsafe entry rejects the whole archive.

## Vector (Inkscape — available)

| Input | Can convert to |
|---|---|
| SVG | PNG, PDF, EPS |
| EPS | SVG, PNG, PDF |
| PS | PDF, SVG |
| DXF | SVG, PDF |

DXF rides on the same Inkscape engine as SVG/EPS/PS rather than a
dedicated CAD engine — Inkscape imports DXF natively.

## CAD

DXF is covered above (Vector/Inkscape — available). DWG is listed with
zero conversion targets — it's a different, proprietary format with real
licensing constraints beyond just "write an engine", so it's surfaced for
transparency without promising a timeline.

## Font (FontForge — available)

| Input | Can convert to |
|---|---|
| TTF | OTF, WOFF, WOFF2 |
| OTF | TTF, WOFF, WOFF2 |
| WOFF | TTF, OTF, WOFF2 |
| WOFF2 | TTF, OTF, WOFF |

Driven through FontForge's non-interactive scripting CLI.

## Why some pairs are missing

The registry only lists a pair once someone has verified the underlying
engine actually produces a correct result for it — see
[conversion-engines.md](conversion-engines.md) for the specific
cross-engine routing table and the quirks that shaped these choices (e.g.
why PDF can become PNG but not the reverse, why RAR is read-only, why
DOCX → PDF works but PDF → DOCX doesn't).
