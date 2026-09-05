pub mod provisioning;
pub mod tools;

use crate::conversion::{archive, ebook, engine, ffmpeg, font, image, jobs, jobs::JobRegistry, office, pdf, registry, text, vector};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn get_format_registry() -> registry::RegistryResponse {
    registry::build_registry()
}

#[tauri::command]
pub fn get_engine_status() -> Vec<engine::EngineInfo> {
    engine::detect_engines()
}

/// Probes a file with whichever engine owns its detected format (ffmpeg for
/// audio/video, ImageMagick for raster images), returning a unified shape
/// the frontend can render regardless of which engine answered.
#[tauri::command]
pub async fn probe_media(path: String) -> Result<ffmpeg::MediaProbe, String> {
    let reg = registry::build_registry();
    let extension = Path::new(&path).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let format = reg.formats.iter().find(|f| f.extensions.contains(&extension));

    match format.map(|f| f.engine.as_str()) {
        Some("image") => image::probe(&path).await,
        _ => ffmpeg::probe(&path).await,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub size_bytes: u64,
    pub is_file: bool,
}

/// Reads size/kind for a file the user explicitly selected (via the OS file
/// picker or native drag-and-drop). We use a plain Rust command rather than
/// the fs plugin here because that plugin's permission scopes are meant for
/// an app's own data directories, not arbitrary user-chosen files — exactly
/// what a file converter needs to read.
#[tauri::command]
pub fn get_file_meta(path: String) -> Result<FileMeta, String> {
    let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    Ok(FileMeta { size_bytes: metadata.len(), is_file: metadata.is_file() })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartConversionRequest {
    pub job_id: String,
    pub input_path: String,
    pub output_format: String,
    pub output_dir: String,
    pub settings: ffmpeg::ConversionSettings,
}

#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub enum ConversionOutcome {
    #[serde(rename_all = "camelCase")]
    Completed { output_path: String, output_size_bytes: u64, remuxed: bool },
    Cancelled,
    Failed { message: String, technical: String },
}

fn failed(message: impl Into<String>, technical: impl Into<String>) -> ConversionOutcome {
    ConversionOutcome::Failed { message: message.into(), technical: technical.into() }
}

/// Avoids ever silently overwriting an existing file: `video.mp4` becomes
/// `video (1).mp4`, `video (2).mp4`, and so on.
pub(crate) fn resolve_output_path(dir: &Path, base_name: &str, ext: &str) -> PathBuf {
    let mut candidate = dir.join(format!("{base_name}.{ext}"));
    let mut n = 1;
    while candidate.exists() {
        candidate = dir.join(format!("{base_name} ({n}).{ext}"));
        n += 1;
    }
    candidate
}

/// Moves a file into place, falling back to copy+delete when the temp
/// directory and destination live on different volumes (a plain rename
/// can't cross drives on Windows).
pub(crate) fn move_into_place(from: &Path, to: &Path) -> std::io::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to)?;
    std::fs::remove_file(from)?;
    Ok(())
}

/// Shared, already-validated context both engine-specific conversion paths
/// need: where the input lives, where the temp working file goes, and where
/// the final output should land once validated.
struct ConversionSetup<'a> {
    req: &'a StartConversionRequest,
    target_format: &'a registry::FormatInfo,
    /// The engine that actually performs this specific (input, output) pair.
    /// Usually equal to `target_format.engine`, but PDF is a special case:
    /// it's registered under its own future "pdf" engine (for PDF-specific
    /// operations like merge/split), while LibreOffice can already produce
    /// PDF directly from office documents — so that pair routes to "office"
    /// instead.
    engine: String,
    temp_output_str: String,
    output_dir: PathBuf,
    base_name: String,
}

impl ConversionSetup<'_> {
    fn temp_output(&self) -> PathBuf {
        PathBuf::from(&self.temp_output_str)
    }

    /// Validates the temp output exists and is non-empty, probes it to make
    /// sure it's real decodable media (never trusting exit code 0 alone),
    /// then moves it into its final, collision-avoided destination.
    async fn finalize(&self, remuxed: bool) -> ConversionOutcome {
        let temp_output = self.temp_output();

        let metadata = match std::fs::metadata(&temp_output) {
            Ok(m) if m.len() > 0 => m,
            Ok(_) => return failed("Conversion produced an empty file", "output file size was 0 bytes"),
            Err(e) => return failed("Conversion did not produce an output file", e.to_string()),
        };

        let validation_result: Result<(), String> = match self.engine.as_str() {
            "image" => image::validate_output(&self.temp_output_str, &self.req.output_format).await,
            "office" => office::validate_output(&temp_output, &self.req.output_format),
            "archive" => archive::validate_output(&temp_output, &self.req.output_format),
            "ebook" => ebook::validate_output(&temp_output, &self.req.output_format),
            "pdf" => pdf::validate_output(&temp_output, &self.req.output_format),
            "vector" => vector::validate_output(&temp_output, &self.req.output_format),
            "font" => font::validate_output(&temp_output, &self.req.output_format),
            "text" => text::validate_output(&temp_output, &self.req.output_format),
            _ => ffmpeg::probe(&self.temp_output_str).await.map(|_| ()),
        };
        if let Err(e) = validation_result {
            return failed("The converted file appears to be corrupted", e);
        }

        let final_path = resolve_output_path(&self.output_dir, &self.base_name, &self.req.output_format);
        if let Err(e) = move_into_place(&temp_output, &final_path) {
            return failed("Couldn't save the converted file", e.to_string());
        }

        ConversionOutcome::Completed {
            output_path: final_path.to_string_lossy().to_string(),
            output_size_bytes: metadata.len(),
            remuxed,
        }
    }
}

async fn convert_with_ffmpeg(app: &AppHandle, registry_state: &JobRegistry, setup: &ConversionSetup<'_>) -> ConversionOutcome {
    let media_probe = match ffmpeg::probe(&setup.req.input_path).await {
        Ok(p) => p,
        Err(e) => return failed("Nexara couldn't read this file", e),
    };

    let (args, remuxed) =
        ffmpeg::build_args(&setup.req.input_path, &setup.temp_output_str, &setup.req.output_format, &setup.req.settings, &media_probe);

    let progress_app = app.clone();
    let progress_job_id = setup.req.job_id.clone();
    let outcome_result = ffmpeg::execute(registry_state, &setup.req.job_id, &args, media_probe.duration_seconds, move |percent| {
        let _ = progress_app.emit("nexara://conversion-progress", serde_json::json!({ "jobId": progress_job_id, "percent": percent }));
    })
    .await;

    let outcome = match outcome_result {
        Ok(o) => o,
        Err(e) => return failed("Nexara couldn't start the conversion", e),
    };

    if outcome.cancelled {
        return ConversionOutcome::Cancelled;
    }
    if !outcome.success {
        return failed(
            format!("Nexara couldn't convert this file to {}", setup.target_format.name),
            if outcome.stderr_tail.trim().is_empty() {
                "ffmpeg exited with an error but produced no diagnostic output.".to_string()
            } else {
                outcome.stderr_tail
            },
        );
    }

    setup.finalize(remuxed).await
}

async fn convert_with_image(registry_state: &JobRegistry, setup: &ConversionSetup<'_>) -> ConversionOutcome {
    let args = image::build_args(&setup.req.input_path, &setup.temp_output_str, &setup.req.settings);

    let outcome_result = image::execute(registry_state, &setup.req.job_id, &args).await;

    let outcome = match outcome_result {
        Ok(o) => o,
        Err(e) => return failed("Nexara couldn't start the conversion", e),
    };

    if outcome.cancelled {
        return ConversionOutcome::Cancelled;
    }
    if !outcome.success {
        return failed(
            format!("Nexara couldn't convert this file to {}", setup.target_format.name),
            if outcome.stderr_tail.trim().is_empty() {
                "ImageMagick exited with an error but produced no diagnostic output.".to_string()
            } else {
                outcome.stderr_tail
            },
        );
    }

    setup.finalize(false).await
}

async fn convert_with_office(registry_state: &JobRegistry, setup: &ConversionSetup<'_>) -> ConversionOutcome {
    let temp_output = setup.temp_output();
    let temp_dir = match temp_output.parent() {
        Some(p) => p,
        None => return failed("Internal error preparing the conversion", "temp output has no parent directory"),
    };
    let temp_dir_str = temp_dir.to_string_lossy().to_string();

    let args = office::build_args(&setup.req.input_path, &temp_dir_str, &setup.req.output_format);

    let outcome_result = office::execute(registry_state, &setup.req.job_id, &args).await;

    let outcome = match outcome_result {
        Ok(o) => o,
        Err(e) => return failed("Nexara couldn't start the conversion", e),
    };

    if outcome.cancelled {
        return ConversionOutcome::Cancelled;
    }
    if !outcome.success {
        return failed(
            format!("Nexara couldn't convert this file to {}", setup.target_format.name),
            if outcome.stderr_tail.trim().is_empty() {
                "LibreOffice exited with an error but produced no diagnostic output.".to_string()
            } else {
                outcome.stderr_tail
            },
        );
    }

    // LibreOffice names its own output file (<input-stem>.<ext>) — normalize
    // it to the shared temp_output convention before the shared finalize step.
    let produced = temp_dir.join(format!("{}.{}", setup.base_name, setup.req.output_format));
    if produced != temp_output {
        if let Err(e) = std::fs::rename(&produced, &temp_output) {
            return failed(
                "LibreOffice did not produce the expected output file",
                format!("expected {} but couldn't find/move it: {e}", produced.display()),
            );
        }
    }

    setup.finalize(false).await
}

async fn convert_with_archive(registry_state: &JobRegistry, setup: &ConversionSetup<'_>) -> ConversionOutcome {
    let temp_output = setup.temp_output();
    let temp_dir = match temp_output.parent() {
        Some(p) => p,
        None => return failed("Internal error preparing the conversion", "temp output has no parent directory"),
    };
    let temp_dir_str = temp_dir.to_string_lossy().to_string();

    let outcome_result =
        archive::convert(registry_state, &setup.req.job_id, &setup.req.input_path, &temp_dir_str, &setup.temp_output_str, &setup.req.output_format)
            .await;

    let outcome = match outcome_result {
        Ok(o) => o,
        Err(e) => return failed("Nexara couldn't convert this archive", e),
    };

    if outcome.cancelled {
        return ConversionOutcome::Cancelled;
    }
    if !outcome.success {
        return failed(
            format!("Nexara couldn't convert this file to {}", setup.target_format.name),
            if outcome.stderr_tail.trim().is_empty() {
                "7-Zip exited with an error but produced no diagnostic output.".to_string()
            } else {
                outcome.stderr_tail
            },
        );
    }

    setup.finalize(false).await
}

async fn convert_with_ebook(registry_state: &JobRegistry, setup: &ConversionSetup<'_>) -> ConversionOutcome {
    let args = ebook::build_args(&setup.req.input_path, &setup.temp_output_str);

    let outcome_result = ebook::execute(registry_state, &setup.req.job_id, &args).await;

    let outcome = match outcome_result {
        Ok(o) => o,
        Err(e) => return failed("Nexara couldn't start the conversion", e),
    };

    if outcome.cancelled {
        return ConversionOutcome::Cancelled;
    }
    if !outcome.success {
        return failed(
            format!("Nexara couldn't convert this file to {}", setup.target_format.name),
            if outcome.stderr_tail.trim().is_empty() {
                "Calibre exited with an error but produced no diagnostic output.".to_string()
            } else {
                outcome.stderr_tail
            },
        );
    }

    setup.finalize(false).await
}

async fn convert_with_pdf(registry_state: &JobRegistry, setup: &ConversionSetup<'_>) -> ConversionOutcome {
    let temp_output = setup.temp_output();
    let temp_dir = match temp_output.parent() {
        Some(p) => p,
        None => return failed("Internal error preparing the conversion", "temp output has no parent directory"),
    };
    let temp_dir_str = temp_dir.to_string_lossy().to_string();

    let args = pdf::build_args(&setup.req.input_path, &temp_dir_str);

    let outcome_result = pdf::execute(registry_state, &setup.req.job_id, &args).await;

    let outcome = match outcome_result {
        Ok(o) => o,
        Err(e) => return failed("Nexara couldn't start the conversion", e),
    };

    if outcome.cancelled {
        return ConversionOutcome::Cancelled;
    }
    if !outcome.success {
        return failed(
            format!("Nexara couldn't convert this file to {}", setup.target_format.name),
            if outcome.stderr_tail.trim().is_empty() {
                "MuPDF exited with an error but produced no diagnostic output.".to_string()
            } else {
                outcome.stderr_tail
            },
        );
    }

    // mutool names its own output (substituting the page number into our
    // pattern) — normalize it to the shared temp_output convention before
    // the shared finalize step.
    let produced = pdf::predicted_first_page_path(&temp_dir_str);
    if produced != temp_output {
        if let Err(e) = std::fs::rename(&produced, &temp_output) {
            return failed(
                "MuPDF did not produce the expected output file",
                format!("expected {} but couldn't find/move it: {e}", produced.display()),
            );
        }
    }

    setup.finalize(false).await
}

async fn convert_with_vector(registry_state: &JobRegistry, setup: &ConversionSetup<'_>) -> ConversionOutcome {
    let args = vector::build_args(&setup.req.input_path, &setup.temp_output_str);

    let outcome_result = vector::execute(registry_state, &setup.req.job_id, &args).await;

    let outcome = match outcome_result {
        Ok(o) => o,
        Err(e) => return failed("Nexara couldn't start the conversion", e),
    };

    if outcome.cancelled {
        return ConversionOutcome::Cancelled;
    }
    if !outcome.success {
        return failed(
            format!("Nexara couldn't convert this file to {}", setup.target_format.name),
            if outcome.stderr_tail.trim().is_empty() {
                "Inkscape exited with an error but produced no diagnostic output.".to_string()
            } else {
                outcome.stderr_tail
            },
        );
    }

    setup.finalize(false).await
}

async fn convert_with_font(registry_state: &JobRegistry, setup: &ConversionSetup<'_>) -> ConversionOutcome {
    let args = font::build_args(&setup.req.input_path, &setup.temp_output_str);

    let outcome_result = font::execute(registry_state, &setup.req.job_id, &args).await;

    let outcome = match outcome_result {
        Ok(o) => o,
        Err(e) => return failed("Nexara couldn't start the conversion", e),
    };

    if outcome.cancelled {
        return ConversionOutcome::Cancelled;
    }
    if !outcome.success {
        return failed(
            format!("Nexara couldn't convert this file to {}", setup.target_format.name),
            if outcome.stderr_tail.trim().is_empty() {
                "FontForge exited with an error but produced no diagnostic output.".to_string()
            } else {
                outcome.stderr_tail
            },
        );
    }

    setup.finalize(false).await
}

async fn convert_with_text(registry_state: &JobRegistry, setup: &ConversionSetup<'_>) -> ConversionOutcome {
    if setup.req.output_format == "pdf" {
        return convert_text_to_pdf(registry_state, setup).await;
    }

    let args = text::build_args(&setup.req.input_path, &setup.temp_output_str, &setup.req.output_format);

    let outcome_result = text::execute(registry_state, &setup.req.job_id, &args).await;

    let outcome = match outcome_result {
        Ok(o) => o,
        Err(e) => return failed("Nexara couldn't start the conversion", e),
    };

    if outcome.cancelled {
        return ConversionOutcome::Cancelled;
    }
    if !outcome.success {
        return failed(
            format!("Nexara couldn't convert this file to {}", setup.target_format.name),
            if outcome.stderr_tail.trim().is_empty() {
                "Pandoc exited with an error but produced no diagnostic output.".to_string()
            } else {
                outcome.stderr_tail
            },
        );
    }

    setup.finalize(false).await
}

/// Pandoc alone can't write PDF without a separate LaTeX install (verified
/// directly: `pandoc in.md -o out.pdf` fails with "'pdflatex' not found").
/// LibreOffice can export PDF directly from HTML, but hangs headless on
/// Markdown/plain-text input specifically (verified directly — identical
/// content saved as `.txt` converts instantly, the same content as `.md`
/// hangs indefinitely). So: normalize non-HTML sources to HTML via Pandoc
/// first, then hand that off to LibreOffice for the actual PDF export.
async fn convert_text_to_pdf(registry_state: &JobRegistry, setup: &ConversionSetup<'_>) -> ConversionOutcome {
    let temp_output = setup.temp_output();
    let temp_dir = match temp_output.parent() {
        Some(p) => p,
        None => return failed("Internal error preparing the conversion", "temp output has no parent directory"),
    };
    let temp_dir_str = temp_dir.to_string_lossy().to_string();

    let input_path = Path::new(&setup.req.input_path);
    let input_extension = input_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    let html_source: PathBuf = if matches!(input_extension.as_str(), "html" | "htm") {
        input_path.to_path_buf()
    } else {
        let intermediate = temp_dir.join("pandoc-intermediate.html");
        let pandoc_args = text::build_args(&setup.req.input_path, &intermediate.to_string_lossy(), "html");

        let outcome_result = text::execute(registry_state, &setup.req.job_id, &pandoc_args).await;
        let outcome = match outcome_result {
            Ok(o) => o,
            Err(e) => return failed("Nexara couldn't start the conversion", e),
        };
        if outcome.cancelled {
            return ConversionOutcome::Cancelled;
        }
        if !outcome.success {
            return failed(
                format!("Nexara couldn't convert this file to {}", setup.target_format.name),
                if outcome.stderr_tail.trim().is_empty() {
                    "Pandoc exited with an error but produced no diagnostic output.".to_string()
                } else {
                    outcome.stderr_tail
                },
            );
        }
        intermediate
    };

    let office_args = office::build_args(&html_source.to_string_lossy(), &temp_dir_str, "pdf");
    let outcome_result = office::execute(registry_state, &setup.req.job_id, &office_args).await;
    let outcome = match outcome_result {
        Ok(o) => o,
        Err(e) => return failed("Nexara couldn't start the conversion", e),
    };
    if outcome.cancelled {
        return ConversionOutcome::Cancelled;
    }
    if !outcome.success {
        return failed(
            format!("Nexara couldn't convert this file to {}", setup.target_format.name),
            if outcome.stderr_tail.trim().is_empty() {
                "LibreOffice exited with an error but produced no diagnostic output.".to_string()
            } else {
                outcome.stderr_tail
            },
        );
    }

    // LibreOffice names its own output after the HTML source's stem (which,
    // for a normalized Markdown/plain-text source, is "pandoc-intermediate"
    // rather than the original input's name) — normalize to the shared
    // temp_output convention before the shared finalize step.
    let html_stem = html_source.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();
    let produced = temp_dir.join(format!("{html_stem}.pdf"));
    if produced != temp_output {
        if let Err(e) = std::fs::rename(&produced, &temp_output) {
            return failed(
                "LibreOffice did not produce the expected output file",
                format!("expected {} but couldn't find/move it: {e}", produced.display()),
            );
        }
    }

    setup.finalize(false).await
}

/// Known cases where an engine can produce an output format beyond the one
/// that format is registered under by default. Kept as an explicit,
/// narrow list rather than a blanket "try the input's engine for anything"
/// rule, so Nexara only ever claims a conversion path it has actually
/// verified works — never an assumption.
fn engine_can_also_produce(engine: &str, output_format: &str) -> bool {
    match engine {
        "office" => matches!(output_format, "pdf" | "txt" | "html"),
        "ebook" => matches!(output_format, "pdf" | "txt"),
        // ImageMagick can write PDF directly from any raster image without
        // needing Ghostscript (verified) — but it can only *read* PDF via a
        // Ghostscript delegate we don't bundle, so this only ever grants
        // the image engine the extra PDF *output*, never PDF as input.
        "image" => output_format == "pdf",
        // PNG is registered under the "image" engine by default (correct
        // for every other input format), but when the *input* is a PDF,
        // ImageMagick can't read it without Ghostscript — MuPDF has to
        // handle that specific pair instead.
        "pdf" => output_format == "png",
        // PNG and PDF are each registered under a different default engine
        // (image, pdf), but when the *input* is SVG/EPS/PS, Inkscape is the
        // one that actually renders it correctly — route those pairs
        // through vector instead of falling through to an engine that
        // either can't read the input at all or renders it poorly.
        "vector" => matches!(output_format, "png" | "pdf"),
        // TXT/HTML/Markdown are all owned by "text" (Pandoc) by default, so
        // most of their pairs already match trivially without needing an
        // entry here. This override only matters for the pairs where the
        // *target* format's own default engine differs and would otherwise
        // be wrong: DOCX defaults to "office" (but LibreOffice can't export
        // DOCX from HTML — verified directly, "no export filter" — so this
        // has to stay on Pandoc), EPUB defaults to "ebook" (untested via
        // Calibre; Pandoc is the verified path), and PDF defaults to "pdf"
        // (MuPDF, which can't read plain text/HTML at all — see
        // `convert_text_to_pdf` for the real PDF path).
        "text" => matches!(output_format, "docx" | "epub" | "pdf"),
        _ => false,
    }
}

#[tauri::command]
pub async fn start_conversion(
    app: tauri::AppHandle,
    registry_state: State<'_, JobRegistry>,
    req: StartConversionRequest,
) -> Result<ConversionOutcome, String> {
    let reg = registry::build_registry();
    let Some(target_format) = reg.formats.iter().find(|f| f.id == req.output_format) else {
        return Ok(failed("Unknown output format", format!("No format registered for id '{}'", req.output_format)));
    };

    let input_path = Path::new(&req.input_path);
    if !input_path.is_file() {
        return Ok(failed("This file can no longer be found", format!("input path does not exist: {}", req.input_path)));
    }

    let input_extension = input_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let input_format = reg.formats.iter().find(|f| f.extensions.contains(&input_extension));

    // Several output formats are registered under a single "default" engine
    // (e.g. PDF under a future dedicated "pdf" engine, TXT/HTML under a
    // future "text"/Pandoc engine), but the engine that handles a specific
    // *input* format can often already produce that same output directly —
    // LibreOffice exports PDF/TXT/HTML from office documents, and Calibre
    // exports PDF/TXT from e-books. Route those known pairs through the
    // input's own engine instead of falsely reporting "not wired up yet".
    let engine = input_format
        .map(|f| f.engine.clone())
        .filter(|input_engine| engine_can_also_produce(input_engine, &req.output_format))
        .unwrap_or_else(|| target_format.engine.clone());

    if !matches!(engine.as_str(), "ffmpeg" | "image" | "office" | "archive" | "ebook" | "pdf" | "vector" | "font" | "text") {
        return Ok(failed(
            format!("The {engine} engine isn't wired up in this build yet"),
            format!("output format '{}' requires engine '{engine}'", req.output_format),
        ));
    }

    let base_name = input_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output").to_string();

    let output_dir = PathBuf::from(&req.output_dir);
    if let Err(e) = std::fs::create_dir_all(&output_dir) {
        return Ok(failed("Couldn't access the output folder", e.to_string()));
    }

    let temp_dir = std::env::temp_dir().join("NexaraFileConvert").join(&req.job_id);
    if let Err(e) = std::fs::create_dir_all(&temp_dir) {
        return Ok(failed("Couldn't create a temporary working folder", e.to_string()));
    }
    let temp_output_str = temp_dir.join(format!("output.{}", req.output_format)).to_string_lossy().to_string();

    let setup = ConversionSetup { req: &req, target_format, engine: engine.clone(), temp_output_str, output_dir, base_name };

    let outcome = match engine.as_str() {
        "ffmpeg" => convert_with_ffmpeg(&app, registry_state.inner(), &setup).await,
        "image" => convert_with_image(registry_state.inner(), &setup).await,
        "office" => convert_with_office(registry_state.inner(), &setup).await,
        "archive" => convert_with_archive(registry_state.inner(), &setup).await,
        "ebook" => convert_with_ebook(registry_state.inner(), &setup).await,
        "pdf" => convert_with_pdf(registry_state.inner(), &setup).await,
        "vector" => convert_with_vector(registry_state.inner(), &setup).await,
        "font" => convert_with_font(registry_state.inner(), &setup).await,
        "text" => convert_with_text(registry_state.inner(), &setup).await,
        _ => unreachable!("engine already validated above"),
    };

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(outcome)
}

#[tauri::command]
pub async fn cancel_conversion(registry_state: State<'_, JobRegistry>, job_id: String) -> Result<(), String> {
    jobs::cancel(registry_state.inner(), &job_id).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn office_can_also_produce_pdf_txt_html_but_nothing_else() {
        assert!(engine_can_also_produce("office", "pdf"));
        assert!(engine_can_also_produce("office", "txt"));
        assert!(engine_can_also_produce("office", "html"));
        assert!(!engine_can_also_produce("office", "epub"));
        assert!(!engine_can_also_produce("office", "mp3"));
    }

    #[test]
    fn ebook_can_also_produce_pdf_and_txt_but_nothing_else() {
        assert!(engine_can_also_produce("ebook", "pdf"));
        assert!(engine_can_also_produce("ebook", "txt"));
        assert!(!engine_can_also_produce("ebook", "html"));
        assert!(!engine_can_also_produce("ebook", "docx"));
    }

    #[test]
    fn unrelated_engines_have_no_overrides() {
        assert!(!engine_can_also_produce("ffmpeg", "pdf"));
        assert!(!engine_can_also_produce("archive", "pdf"));
    }

    #[test]
    fn image_can_write_pdf_but_not_read_it() {
        // ImageMagick can write PDF from any raster image without
        // Ghostscript (verified), but reading PDF needs a Ghostscript
        // delegate this build doesn't have — so the override only ever
        // grants the extra *output*, never routes PDF input through image.
        assert!(engine_can_also_produce("image", "pdf"));
        assert!(!engine_can_also_produce("image", "png"));
    }

    #[test]
    fn pdf_engine_can_only_additionally_produce_png() {
        assert!(engine_can_also_produce("pdf", "png"));
        assert!(!engine_can_also_produce("pdf", "jpg"));
        assert!(!engine_can_also_produce("pdf", "docx"));
    }

    #[test]
    fn vector_engine_can_additionally_produce_png_and_pdf() {
        assert!(engine_can_also_produce("vector", "png"));
        assert!(engine_can_also_produce("vector", "pdf"));
        assert!(!engine_can_also_produce("vector", "svg"));
        assert!(!engine_can_also_produce("vector", "jpg"));
    }

    #[test]
    fn text_engine_can_additionally_produce_docx_epub_and_pdf() {
        assert!(engine_can_also_produce("text", "docx"));
        assert!(engine_can_also_produce("text", "epub"));
        assert!(engine_can_also_produce("text", "pdf"));
        assert!(!engine_can_also_produce("text", "html"));
        assert!(!engine_can_also_produce("text", "md"));
        assert!(!engine_can_also_produce("text", "txt"));
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("nexara-test-output-naming").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolve_output_path_uses_plain_name_when_free() {
        let dir = temp_test_dir("free");
        let path = resolve_output_path(&dir, "video", "mp4");
        assert_eq!(path.file_name().unwrap().to_str().unwrap(), "video.mp4");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_output_path_never_overwrites_existing_files() {
        let dir = temp_test_dir("collision");
        std::fs::write(dir.join("video.mp4"), b"existing").unwrap();
        std::fs::write(dir.join("video (1).mp4"), b"existing too").unwrap();

        let path = resolve_output_path(&dir, "video", "mp4");
        assert_eq!(path.file_name().unwrap().to_str().unwrap(), "video (2).mp4");
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn conversion_outcome_serializes_with_camel_case_fields() {
        // Regression test: an internally-tagged enum's `rename_all` does NOT
        // cascade into struct-variant field names in this serde version, so
        // each variant needs its own `rename_all` — verified against the
        // exact wire format the frontend's TS types expect.
        let outcome = ConversionOutcome::Completed { output_path: "C:/out.mp3".into(), output_size_bytes: 1234, remuxed: false };
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("\"outcome\":\"completed\""), "got: {json}");
        assert!(json.contains("\"outputPath\":\"C:/out.mp3\""), "got: {json}");
        assert!(json.contains("\"outputSizeBytes\":1234"), "got: {json}");
    }

    #[test]
    fn move_into_place_relocates_temp_file() {
        let dir = temp_test_dir("move");
        let from = dir.join("temp-output.mp3");
        let to = dir.join("final.mp3");
        std::fs::write(&from, b"fake audio bytes").unwrap();

        move_into_place(&from, &to).unwrap();

        assert!(!from.exists(), "temp file should be gone after moving");
        assert!(to.exists(), "final file should exist at the destination");
        assert_eq!(std::fs::read(&to).unwrap(), b"fake audio bytes");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
