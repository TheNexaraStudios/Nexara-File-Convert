use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::State;

use crate::conversion::archive as archive_engine;
use crate::conversion::jobs::JobRegistry;
use crate::tools::{archive_create, archive_extract, metadata, pages, pdf_export, pdf_merge, pdf_split, MultiOutcome};

use super::{move_into_place, resolve_output_path};

#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub enum ToolOutcome {
    #[serde(rename_all = "camelCase")]
    Completed { output_paths: Vec<String> },
    Cancelled,
    #[serde(rename_all = "camelCase")]
    Failed { message: String, technical: String },
}

fn failed(message: impl Into<String>, technical: impl Into<String>) -> ToolOutcome {
    ToolOutcome::Failed { message: message.into(), technical: technical.into() }
}

fn job_temp_dir(job_id: &str) -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join("NexaraFileConvert").join(job_id);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Moves every file in `temp_paths` into `output_dir`, giving each a
/// collision-avoided final name (never silently overwriting an existing
/// file — the same rule every other conversion in the app follows), and
/// returns the final paths in the same order.
fn place_all(temp_paths: &[PathBuf], output_dir: &Path) -> Result<Vec<String>, String> {
    let mut finals = Vec::with_capacity(temp_paths.len());
    for temp_path in temp_paths {
        let stem = temp_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
        let ext = temp_path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let final_path = resolve_output_path(output_dir, stem, ext);
        move_into_place(temp_path, &final_path).map_err(|e| e.to_string())?;
        finals.push(final_path.to_string_lossy().to_string());
    }
    Ok(finals)
}

#[tauri::command]
pub async fn get_pdf_page_count(path: String) -> Result<u32, String> {
    pages::page_count(&path).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePdfsRequest {
    pub job_id: String,
    pub input_paths: Vec<String>,
    pub output_dir: String,
    pub output_name: String,
}

#[tauri::command]
pub async fn merge_pdfs(registry_state: State<'_, JobRegistry>, req: MergePdfsRequest) -> Result<ToolOutcome, String> {
    if req.input_paths.len() < 2 {
        return Ok(failed("Select at least two PDFs to merge", "input_paths had fewer than 2 entries"));
    }

    let temp_dir = match job_temp_dir(&req.job_id) {
        Ok(d) => d,
        Err(e) => return Ok(failed("Couldn't create a temporary working folder", e.to_string())),
    };
    let temp_output = temp_dir.join("merged.pdf");

    let args = pdf_merge::build_args(&req.input_paths, &temp_output.to_string_lossy());
    let outcome = match pdf_merge::execute(registry_state.inner(), &req.job_id, &args).await {
        Ok(o) => o,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Ok(failed("Nexara couldn't start the merge", e));
        }
    };

    if outcome.cancelled {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Ok(ToolOutcome::Cancelled);
    }
    if !outcome.success {
        let _ = std::fs::remove_dir_all(&temp_dir);
        let technical = if outcome.stderr_tail.trim().is_empty() {
            "mutool exited with an error but produced no diagnostic output.".to_string()
        } else {
            outcome.stderr_tail
        };
        return Ok(failed("Nexara couldn't merge these PDFs", technical));
    }

    if let Err(e) = pdf_merge::validate_output(&temp_output, req.input_paths.len() as u32).await {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Ok(failed("The merged file appears to be corrupted", e));
    }

    let final_path = resolve_output_path(Path::new(&req.output_dir), &req.output_name, "pdf");
    if let Err(e) = move_into_place(&temp_output, &final_path) {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Ok(failed("Couldn't save the merged file", e.to_string()));
    }
    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok(ToolOutcome::Completed { output_paths: vec![final_path.to_string_lossy().to_string()] })
}

#[derive(Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum SplitMode {
    #[serde(rename_all = "camelCase")]
    Range { pages: String, output_name: String },
    #[serde(rename_all = "camelCase")]
    EachPage { base_name: String },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitPdfRequest {
    pub job_id: String,
    pub input_path: String,
    pub output_dir: String,
    #[serde(flatten)]
    pub mode: SplitMode,
}

#[tauri::command]
pub async fn split_pdf(registry_state: State<'_, JobRegistry>, req: SplitPdfRequest) -> Result<ToolOutcome, String> {
    let total_pages = match pages::page_count(&req.input_path).await {
        Ok(n) => n,
        Err(e) => return Ok(failed("Nexara couldn't read this PDF", e)),
    };

    let temp_dir = match job_temp_dir(&req.job_id) {
        Ok(d) => d,
        Err(e) => return Ok(failed("Couldn't create a temporary working folder", e.to_string())),
    };

    match req.mode {
        SplitMode::Range { pages: spec, output_name } => {
            let page_list = match pages::parse_page_ranges(&spec, total_pages) {
                Ok(p) => p,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    return Ok(failed(e, "invalid page range"));
                }
            };

            let temp_output = temp_dir.join("split.pdf");
            let outcome =
                match pdf_split::extract_range(registry_state.inner(), &req.job_id, &req.input_path, &temp_output.to_string_lossy(), &page_list)
                    .await
                {
                    Ok(o) => o,
                    Err(e) => {
                        let _ = std::fs::remove_dir_all(&temp_dir);
                        return Ok(failed("Nexara couldn't start the split", e));
                    }
                };

            if outcome.cancelled {
                let _ = std::fs::remove_dir_all(&temp_dir);
                return Ok(ToolOutcome::Cancelled);
            }
            if !outcome.success {
                let _ = std::fs::remove_dir_all(&temp_dir);
                let technical = if outcome.stderr_tail.trim().is_empty() {
                    "mutool exited with an error but produced no diagnostic output.".to_string()
                } else {
                    outcome.stderr_tail
                };
                return Ok(failed("Nexara couldn't split this PDF", technical));
            }

            if let Err(e) = pdf_merge::validate_output(&temp_output, page_list.len() as u32).await {
                let _ = std::fs::remove_dir_all(&temp_dir);
                return Ok(failed("The split file appears to be corrupted", e));
            }

            let final_path = resolve_output_path(Path::new(&req.output_dir), &output_name, "pdf");
            if let Err(e) = move_into_place(&temp_output, &final_path) {
                let _ = std::fs::remove_dir_all(&temp_dir);
                return Ok(failed("Couldn't save the split file", e.to_string()));
            }
            let _ = std::fs::remove_dir_all(&temp_dir);
            Ok(ToolOutcome::Completed { output_paths: vec![final_path.to_string_lossy().to_string()] })
        }
        SplitMode::EachPage { base_name } => {
            let page_list: Vec<u32> = (1..=total_pages).collect();
            let per_page_dir = temp_dir.join("pages");
            if let Err(e) = std::fs::create_dir_all(&per_page_dir) {
                let _ = std::fs::remove_dir_all(&temp_dir);
                return Ok(failed("Couldn't create a temporary working folder", e.to_string()));
            }

            let result =
                pdf_split::extract_each_page(registry_state.inner(), &req.job_id, &req.input_path, &per_page_dir, &base_name, &page_list).await;

            match result {
                MultiOutcome::Cancelled => {
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    Ok(ToolOutcome::Cancelled)
                }
                MultiOutcome::Failed(e) => {
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    Ok(failed("Nexara couldn't split this PDF", e))
                }
                MultiOutcome::Completed(temp_paths) => {
                    let placed = place_all(&temp_paths, Path::new(&req.output_dir));
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    match placed {
                        Ok(finals) => Ok(ToolOutcome::Completed { output_paths: finals }),
                        Err(e) => Ok(failed("Couldn't save the split pages", e)),
                    }
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPdfPagesRequest {
    pub job_id: String,
    pub input_path: String,
    pub output_dir: String,
    /// `"all"` or a page-range spec like `"1-3,5"`.
    pub pages: String,
    pub dpi: u32,
    pub format: String,
    pub base_name: String,
}

#[tauri::command]
pub async fn export_pdf_pages(registry_state: State<'_, JobRegistry>, req: ExportPdfPagesRequest) -> Result<ToolOutcome, String> {
    let total_pages = match pages::page_count(&req.input_path).await {
        Ok(n) => n,
        Err(e) => return Ok(failed("Nexara couldn't read this PDF", e)),
    };

    let page_list = if req.pages.trim().eq_ignore_ascii_case("all") {
        (1..=total_pages).collect::<Vec<u32>>()
    } else {
        match pages::parse_page_ranges(&req.pages, total_pages) {
            Ok(p) => p,
            Err(e) => return Ok(failed(e, "invalid page range")),
        }
    };

    let temp_dir = match job_temp_dir(&req.job_id) {
        Ok(d) => d,
        Err(e) => return Ok(failed("Couldn't create a temporary working folder", e.to_string())),
    };

    let result = pdf_export::export_pages(
        registry_state.inner(),
        &req.job_id,
        &req.input_path,
        &temp_dir,
        &req.base_name,
        &page_list,
        req.dpi,
        &req.format,
    )
    .await;

    match result {
        MultiOutcome::Cancelled => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            Ok(ToolOutcome::Cancelled)
        }
        MultiOutcome::Failed(e) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            Ok(failed("Nexara couldn't export these pages", e))
        }
        MultiOutcome::Completed(temp_paths) => {
            let placed = place_all(&temp_paths, Path::new(&req.output_dir));
            let _ = std::fs::remove_dir_all(&temp_dir);
            match placed {
                Ok(finals) => Ok(ToolOutcome::Completed { output_paths: finals }),
                Err(e) => Ok(failed("Couldn't save the exported pages", e)),
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractArchiveRequest {
    pub job_id: String,
    pub input_path: String,
    pub dest_dir: String,
    pub password: Option<String>,
}

#[tauri::command]
pub async fn extract_archive(registry_state: State<'_, JobRegistry>, req: ExtractArchiveRequest) -> Result<ToolOutcome, String> {
    let outcome =
        archive_extract::extract_to(registry_state.inner(), &req.job_id, &req.input_path, &req.dest_dir, req.password.as_deref()).await;

    let outcome = match outcome {
        Ok(o) => o,
        Err(e) => return Ok(failed("Nexara couldn't extract this archive", e)),
    };

    if outcome.cancelled {
        return Ok(ToolOutcome::Cancelled);
    }
    if !outcome.success {
        let technical = if outcome.stderr_tail.trim().is_empty() {
            "7-Zip exited with an error but produced no diagnostic output.".to_string()
        } else {
            outcome.stderr_tail
        };
        return Ok(failed("Nexara couldn't extract this archive", technical));
    }

    Ok(ToolOutcome::Completed { output_paths: vec![req.dest_dir] })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateArchiveRequest {
    pub job_id: String,
    pub input_paths: Vec<String>,
    pub output_dir: String,
    pub output_name: String,
    pub format: String,
    pub compression_level: u8,
    pub password: Option<String>,
}

#[tauri::command]
pub async fn create_archive(registry_state: State<'_, JobRegistry>, req: CreateArchiveRequest) -> Result<ToolOutcome, String> {
    if req.input_paths.is_empty() {
        return Ok(failed("Select at least one file or folder to archive", "input_paths was empty"));
    }
    if !archive_create::CREATABLE_FORMATS.contains(&req.format.as_str()) {
        return Ok(failed(format!("Nexara can't create .{} archives", req.format), format!("unsupported creation format '{}'", req.format)));
    }

    let temp_dir = match job_temp_dir(&req.job_id) {
        Ok(d) => d,
        Err(e) => return Ok(failed("Couldn't create a temporary working folder", e.to_string())),
    };
    let temp_output = temp_dir.join(format!("output.{}", req.format));

    let outcome = archive_create::create(
        registry_state.inner(),
        &req.job_id,
        &req.input_paths,
        &temp_output.to_string_lossy(),
        &req.format,
        req.compression_level,
        req.password.as_deref(),
    )
    .await;

    let outcome = match outcome {
        Ok(o) => o,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Ok(failed("Nexara couldn't start creating the archive", e));
        }
    };

    if outcome.cancelled {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Ok(ToolOutcome::Cancelled);
    }
    if !outcome.success {
        let _ = std::fs::remove_dir_all(&temp_dir);
        let technical = if outcome.stderr_tail.trim().is_empty() {
            "7-Zip exited with an error but produced no diagnostic output.".to_string()
        } else {
            outcome.stderr_tail
        };
        return Ok(failed("Nexara couldn't create this archive", technical));
    }

    if let Err(e) = archive_engine::validate_output(&temp_output, &req.format) {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Ok(failed("The created archive appears to be corrupted", e));
    }

    let final_path = resolve_output_path(Path::new(&req.output_dir), &req.output_name, &req.format);
    if let Err(e) = move_into_place(&temp_output, &final_path) {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Ok(failed("Couldn't save the archive", e.to_string()));
    }
    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok(ToolOutcome::Completed { output_paths: vec![final_path.to_string_lossy().to_string()] })
}

#[tauri::command]
pub async fn inspect_metadata(path: String) -> Result<metadata::MetadataInfo, String> {
    metadata::inspect(&path).await
}

/// Lists an archive's entries without extracting anything — lets the
/// frontend detect "this needs a password" (or an unsafe/corrupt archive)
/// *before* the user picks a destination folder and commits to extracting,
/// rather than only finding out after the fact.
#[tauri::command]
pub async fn preview_archive(path: String, password: Option<String>) -> Result<(), String> {
    archive_extract::preview(&path, password.as_deref()).await
}
