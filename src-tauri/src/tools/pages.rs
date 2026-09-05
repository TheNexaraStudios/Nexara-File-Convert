use std::process::Stdio;

use crate::conversion::engine;

/// Parses a page-range spec like `"1-3,5,8-10"` into an ordered,
/// deduplicated list of 1-based page numbers, validated against
/// `total_pages`. Never silently drops or clamps an invalid entry — an
/// out-of-range, zero, reversed, or malformed piece is a hard error naming
/// exactly the part that's wrong, so the caller can show it back to the
/// user rather than guessing what was meant.
pub fn parse_page_ranges(spec: &str, total_pages: u32) -> Result<Vec<u32>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("Enter at least one page or page range.".to_string());
    }
    if total_pages == 0 {
        return Err("This document has no pages.".to_string());
    }

    let mut pages: Vec<u32> = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(format!("'{spec}' has an empty entry between commas."));
        }
        if let Some((start_str, end_str)) = part.split_once('-') {
            let start: u32 = start_str.trim().parse().map_err(|_| format!("'{part}' isn't a valid page range."))?;
            let end: u32 = end_str.trim().parse().map_err(|_| format!("'{part}' isn't a valid page range."))?;
            if start == 0 || end == 0 {
                return Err(format!("'{part}': page numbers start at 1."));
            }
            if start > end {
                return Err(format!("'{part}': the range is backwards — {start} comes after {end}."));
            }
            if end > total_pages {
                return Err(format!("'{part}': this document only has {total_pages} page(s)."));
            }
            pages.extend(start..=end);
        } else {
            let page: u32 = part.parse().map_err(|_| format!("'{part}' isn't a valid page number."))?;
            if page == 0 {
                return Err(format!("'{part}': page numbers start at 1."));
            }
            if page > total_pages {
                return Err(format!("'{part}': this document only has {total_pages} page(s)."));
            }
            pages.push(page);
        }
    }
    pages.sort_unstable();
    pages.dedup();
    Ok(pages)
}

/// Formats a page list back into mutool's own selector syntax — consecutive
/// runs collapsed into `"N-M"` — so a page list built from user input (or
/// from "every page") can be passed straight through to `mutool`.
pub fn to_mutool_range(pages: &[u32]) -> String {
    if pages.is_empty() {
        return String::new();
    }
    let mut parts = Vec::new();
    let mut start = pages[0];
    let mut prev = pages[0];
    for &p in &pages[1..] {
        if p == prev + 1 {
            prev = p;
            continue;
        }
        parts.push(if start == prev { start.to_string() } else { format!("{start}-{prev}") });
        start = p;
        prev = p;
    }
    parts.push(if start == prev { start.to_string() } else { format!("{start}-{prev}") });
    parts.join(",")
}

/// Reads a PDF's page count via `mutool info`. Used to validate a page-range
/// request *before* spawning any per-page conversion work.
pub async fn page_count(path: &str) -> Result<u32, String> {
    let output = tokio::process::Command::new(engine::binary_path("mutool"))
        .args(["info", path])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Could not run MuPDF: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Nexara couldn't read this PDF: {}", stderr.trim()));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Pages: ") {
            return rest.trim().parse::<u32>().map_err(|_| "Could not read the page count.".to_string());
        }
    }
    Err("Could not determine the page count.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mixed_ranges_and_singles_in_order() {
        assert_eq!(parse_page_ranges("1-3,5,8-10", 10).unwrap(), vec![1, 2, 3, 5, 8, 9, 10]);
    }

    #[test]
    fn sorts_and_dedupes_out_of_order_input() {
        assert_eq!(parse_page_ranges("5,1-3,2", 10).unwrap(), vec![1, 2, 3, 5]);
    }

    #[test]
    fn single_page_works() {
        assert_eq!(parse_page_ranges("4", 10).unwrap(), vec![4]);
    }

    #[test]
    fn rejects_empty_spec() {
        assert!(parse_page_ranges("", 10).is_err());
        assert!(parse_page_ranges("   ", 10).is_err());
    }

    #[test]
    fn rejects_page_zero() {
        assert!(parse_page_ranges("0", 10).is_err());
        assert!(parse_page_ranges("0-3", 10).is_err());
    }

    #[test]
    fn rejects_out_of_range_page() {
        let err = parse_page_ranges("1-3,15", 10).unwrap_err();
        assert!(err.contains("15"), "error should name the offending page, got: {err}");
    }

    #[test]
    fn rejects_reversed_range() {
        let err = parse_page_ranges("5-2", 10).unwrap_err();
        assert!(err.contains("backwards"), "got: {err}");
    }

    #[test]
    fn rejects_non_numeric_entries() {
        assert!(parse_page_ranges("abc", 10).is_err());
        assert!(parse_page_ranges("1-abc", 10).is_err());
    }

    #[test]
    fn rejects_empty_entry_between_commas() {
        assert!(parse_page_ranges("1,,3", 10).is_err());
    }

    #[test]
    fn rejects_when_document_has_no_pages() {
        assert!(parse_page_ranges("1", 0).is_err());
    }

    #[test]
    fn to_mutool_range_collapses_consecutive_runs() {
        assert_eq!(to_mutool_range(&[1, 2, 3, 5, 8, 9, 10]), "1-3,5,8-10");
    }

    #[test]
    fn to_mutool_range_handles_all_singles() {
        assert_eq!(to_mutool_range(&[1, 3, 5]), "1,3,5");
    }

    #[test]
    fn to_mutool_range_handles_single_page() {
        assert_eq!(to_mutool_range(&[7]), "7");
    }

    #[test]
    fn to_mutool_range_handles_empty() {
        assert_eq!(to_mutool_range(&[]), "");
    }
}
