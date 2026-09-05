use std::path::Path;

use crate::conversion::jobs::JobRegistry;
use crate::conversion::process;

use super::pages::to_mutool_range;
use super::MultiOutcome;

/// Builds `mutool convert -o <output> -F pdf <input> <pages>` — extracting
/// a page selection into one new PDF. Verified directly: selecting pages
/// "1-2" out of a 3-page source produced a genuine 2-page result.
pub fn range_args(input: &str, output_path: &str, pages: &[u32]) -> Vec<String> {
    vec!["convert".to_string(), "-o".to_string(), output_path.to_string(), "-F".to_string(), "pdf".to_string(), input.to_string(), to_mutool_range(pages)]
}

/// Extracts `pages` from `input` into a single new PDF at `output_path`.
pub async fn extract_range(
    registry: &JobRegistry,
    job_id: &str,
    input: &str,
    output_path: &str,
    pages: &[u32],
) -> Result<process::ExecuteOutcome, String> {
    let args = range_args(input, output_path, pages);
    process::run_and_track(registry, job_id, "mutool", &args).await
}

/// `mutool convert`'s vector (PDF/SVG) output never substitutes `%d` into
/// its filename — verified directly: `-o "page-%d.pdf"` with no page
/// selector wrote one literal file named `page-%d.pdf`, not one file per
/// page — unlike its raster output, which does substitute the page index.
/// So "export every page as its own PDF" has to be one `mutool convert`
/// call per page rather than a single call with a `%d` pattern.
pub async fn extract_each_page(
    registry: &JobRegistry,
    job_id: &str,
    input: &str,
    output_dir: &Path,
    base_name: &str,
    pages: &[u32],
) -> MultiOutcome {
    let mut outputs = Vec::with_capacity(pages.len());
    for &page in pages {
        let output_path = output_dir.join(format!("{base_name}-page-{page:03}.pdf"));
        let args = range_args(input, &output_path.to_string_lossy(), &[page]);
        let outcome = match process::run_and_track(registry, job_id, "mutool", &args).await {
            Ok(o) => o,
            Err(e) => return MultiOutcome::Failed(e),
        };
        if outcome.cancelled {
            return MultiOutcome::Cancelled;
        }
        if !outcome.success {
            let detail = if outcome.stderr_tail.trim().is_empty() {
                format!("mutool exited with an error extracting page {page}")
            } else {
                outcome.stderr_tail
            };
            return MultiOutcome::Failed(detail);
        }
        outputs.push(output_path);
    }
    MultiOutcome::Completed(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_args_uses_pdf_output_format_and_collapsed_range() {
        let args = range_args(r"C:\in.pdf", r"C:\out.pdf", &[1, 2, 3, 5]);
        assert_eq!(args, vec!["convert", "-o", r"C:\out.pdf", "-F", "pdf", r"C:\in.pdf", "1-3,5"]);
    }

    #[test]
    fn range_args_single_page() {
        let args = range_args(r"C:\in.pdf", r"C:\out.pdf", &[7]);
        assert_eq!(args.last().unwrap(), "7");
    }
}
