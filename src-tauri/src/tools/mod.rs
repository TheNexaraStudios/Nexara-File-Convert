pub mod archive_create;
pub mod archive_extract;
pub mod metadata;
pub mod pages;
pub mod pdf_export;
pub mod pdf_merge;
pub mod pdf_split;

use std::path::PathBuf;

/// Outcome of a tool operation that can produce more than one output file
/// (splitting a PDF into per-page files, exporting several pages as
/// images, extracting an archive). The single-output engines already have
/// `ExecuteOutcome`/`ConversionOutcome` for this; multi-output tools need
/// their own list-shaped result instead of forcing a "which one file" answer
/// that doesn't exist.
pub enum MultiOutcome {
    Completed(Vec<PathBuf>),
    Cancelled,
    Failed(String),
}
