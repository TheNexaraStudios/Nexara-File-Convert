use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use std::time::UNIX_EPOCH;

use crate::conversion::engine;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicInfo {
    pub file_name: String,
    pub size_bytes: u64,
    /// Milliseconds since the Unix epoch — left as a plain number rather
    /// than a formatted string so the frontend renders it in the user's
    /// own locale/timezone instead of a fixed Rust-side format.
    pub modified_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamInfo {
    pub codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<String>,
    pub sample_rate: Option<String>,
    pub channels: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MetadataInfo {
    #[serde(rename_all = "camelCase")]
    Media {
        basic: BasicInfo,
        container: Option<String>,
        duration_seconds: Option<f64>,
        bit_rate: Option<i64>,
        video: Option<StreamInfo>,
        audio: Option<StreamInfo>,
        tags: Vec<Tag>,
    },
    #[serde(rename_all = "camelCase")]
    Image {
        basic: BasicInfo,
        width: Option<u32>,
        height: Option<u32>,
        format: Option<String>,
        colorspace: Option<String>,
        bit_depth: Option<String>,
        has_alpha: Option<bool>,
    },
    #[serde(rename_all = "camelCase")]
    Pdf {
        basic: BasicInfo,
        page_count: Option<u32>,
        page_width: Option<f64>,
        page_height: Option<f64>,
        encrypted: bool,
    },
    #[serde(rename_all = "camelCase")]
    Archive {
        basic: BasicInfo,
        entry_count: Option<u32>,
        uncompressed_size: Option<u64>,
        method: Option<String>,
        encrypted: bool,
    },
    #[serde(rename_all = "camelCase")]
    Basic { basic: BasicInfo },
}

fn basic_info(path: &Path) -> Result<BasicInfo, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let modified_at_ms =
        meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64);
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    Ok(BasicInfo { file_name, size_bytes: meta.len(), modified_at_ms })
}

const MEDIA_EXTENSIONS: &[&str] =
    &["mp4", "m4v", "mov", "mkv", "webm", "avi", "gif", "mp3", "wav", "flac", "aac", "m4a", "ogg"];
const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "avif", "tiff", "tif", "bmp", "ico", "heic", "heif", "cr2", "cr3", "nef", "arw",
    "dng", "raf", "orf", "rw2",
];
const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "7z", "tar", "gz", "tgz", "rar"];

/// Reads real, format-appropriate metadata for `path` without modifying it
/// in any way — every command here is a pure read (`ffprobe`, `magick
/// identify`, `mutool info`/`pages`, `7z l`), never a write. Falls back to
/// plain filesystem info for formats none of the installed engines have a
/// dedicated inspector for, rather than guessing or fabricating fields.
pub async fn inspect(path: &str) -> Result<MetadataInfo, String> {
    let p = Path::new(path);
    let basic = basic_info(p)?;
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    if MEDIA_EXTENSIONS.contains(&ext.as_str()) {
        return inspect_media(path, basic).await;
    }
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return inspect_image(path, basic).await;
    }
    if ext == "pdf" {
        return inspect_pdf(path, basic).await;
    }
    if ARCHIVE_EXTENSIONS.contains(&ext.as_str()) {
        return inspect_archive(path, basic).await;
    }
    Ok(MetadataInfo::Basic { basic })
}

async fn inspect_media(path: &str, basic: BasicInfo) -> Result<MetadataInfo, String> {
    let output = tokio::process::Command::new(engine::binary_path("ffprobe"))
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", path])
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Could not run ffprobe: {e}"))?;

    if !output.status.success() {
        return Err(format!("Nexara couldn't read this file: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }

    let json: Value = serde_json::from_slice(&output.stdout).map_err(|e| format!("Could not parse media info: {e}"))?;
    Ok(media_info_from_json(&json, basic))
}

fn media_info_from_json(json: &Value, basic: BasicInfo) -> MetadataInfo {
    let format = &json["format"];
    let container = format["format_long_name"].as_str().or_else(|| format["format_name"].as_str()).map(str::to_string);
    let duration_seconds = format["duration"].as_str().and_then(|s| s.parse::<f64>().ok());
    let bit_rate = format["bit_rate"].as_str().and_then(|s| s.parse::<i64>().ok());

    let mut video = None;
    let mut audio = None;
    if let Some(streams) = json["streams"].as_array() {
        for stream in streams {
            let codec_type = stream["codec_type"].as_str().unwrap_or("");
            if codec_type == "video" && video.is_none() {
                video = Some(StreamInfo {
                    codec: stream["codec_name"].as_str().map(str::to_string),
                    width: stream["width"].as_u64().map(|v| v as u32),
                    height: stream["height"].as_u64().map(|v| v as u32),
                    frame_rate: stream["r_frame_rate"].as_str().map(str::to_string),
                    sample_rate: None,
                    channels: None,
                });
            } else if codec_type == "audio" && audio.is_none() {
                audio = Some(StreamInfo {
                    codec: stream["codec_name"].as_str().map(str::to_string),
                    width: None,
                    height: None,
                    frame_rate: None,
                    sample_rate: stream["sample_rate"].as_str().map(str::to_string),
                    channels: stream["channels"].as_u64().map(|v| v as u32),
                });
            }
        }
    }

    let mut tags = Vec::new();
    if let Some(tag_obj) = format["tags"].as_object() {
        for (key, value) in tag_obj {
            if let Some(v) = value.as_str() {
                tags.push(Tag { key: key.clone(), value: v.to_string() });
            }
        }
    }
    tags.sort_by(|a, b| a.key.cmp(&b.key));

    MetadataInfo::Media { basic, container, duration_seconds, bit_rate, video, audio, tags }
}

async fn inspect_image(path: &str, basic: BasicInfo) -> Result<MetadataInfo, String> {
    let target = format!("{path}[0]");
    let output = tokio::process::Command::new(engine::binary_path("magick"))
        .args(["identify", "-format", "%w|%h|%m|%[colorspace]|%z|%A", &target])
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Could not run ImageMagick: {e}"))?;

    if !output.status.success() {
        return Err(format!("Nexara couldn't read this image: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(image_info_from_identify_output(&text, basic))
}

fn image_info_from_identify_output(text: &str, basic: BasicInfo) -> MetadataInfo {
    let mut parts = text.trim().split('|');
    let width = parts.next().and_then(|s| s.parse::<u32>().ok());
    let height = parts.next().and_then(|s| s.parse::<u32>().ok());
    let format = parts.next().filter(|s| !s.is_empty()).map(str::to_string);
    let colorspace = parts.next().filter(|s| !s.is_empty()).map(str::to_string);
    let bit_depth = parts.next().filter(|s| !s.is_empty()).map(|s| format!("{s}-bit"));
    let has_alpha = parts.next().map(|s| s.eq_ignore_ascii_case("true"));

    MetadataInfo::Image { basic, width, height, format, colorspace, bit_depth, has_alpha }
}

async fn inspect_pdf(path: &str, basic: BasicInfo) -> Result<MetadataInfo, String> {
    let info_output = tokio::process::Command::new(engine::binary_path("mutool"))
        .args(["info", "-M", path])
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Could not run MuPDF: {e}"))?;

    let stdout = String::from_utf8_lossy(&info_output.stdout);
    let stderr = String::from_utf8_lossy(&info_output.stderr);

    if !info_output.status.success() {
        if stderr.to_lowercase().contains("password") {
            return Ok(MetadataInfo::Pdf { basic, page_count: None, page_width: None, page_height: None, encrypted: true });
        }
        return Err(format!("Nexara couldn't read this PDF: {}", stderr.trim()));
    }

    let (page_count, page_width, page_height) = pdf_info_from_mutool_output(&stdout);
    Ok(MetadataInfo::Pdf { basic, page_count, page_width, page_height, encrypted: false })
}

/// Parses the fields we need from `mutool info -M`'s text output: the
/// `Pages: N` summary line and the first page's `[ l b r t ]` MediaBox
/// (converted to plain width/height), rather than the fonts/images/etc.
/// detail this command can also print.
fn pdf_info_from_mutool_output(text: &str) -> (Option<u32>, Option<f64>, Option<f64>) {
    let page_count = text.lines().find_map(|l| l.strip_prefix("Pages: ")).and_then(|s| s.trim().parse::<u32>().ok());

    let mediabox_line = text.lines().find(|l| l.contains('[') && l.contains(']') && l.trim_start().starts_with(char::is_numeric));
    let dims = mediabox_line.and_then(|line| {
        let inside = line.split('[').nth(1)?.split(']').next()?;
        let nums: Vec<f64> = inside.split_whitespace().filter_map(|n| n.parse::<f64>().ok()).collect();
        if nums.len() == 4 {
            Some((nums[2] - nums[0], nums[3] - nums[1]))
        } else {
            None
        }
    });

    (page_count, dims.map(|(w, _)| w), dims.map(|(_, h)| h))
}

async fn inspect_archive(path: &str, basic: BasicInfo) -> Result<MetadataInfo, String> {
    let output = tokio::process::Command::new(engine::binary_path("7z"))
        .args(["l", "-slt", path, crate::conversion::archive::UTF8_CONSOLE])
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Could not run 7-Zip: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined_lower = format!("{stdout}{}", String::from_utf8_lossy(&output.stderr)).to_lowercase();

    if !output.status.success() {
        if combined_lower.contains("wrong password") || stdout.to_lowercase().contains("enter password") {
            return Ok(MetadataInfo::Archive { basic, entry_count: None, uncompressed_size: None, method: None, encrypted: true });
        }
        return Err(format!("Nexara couldn't read this archive: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }

    let (entry_count, uncompressed_size, method) = archive_info_from_listing(&stdout);
    Ok(MetadataInfo::Archive { basic, entry_count, uncompressed_size, method, encrypted: false })
}

/// Parses `7z l -slt` output for a summary: how many real entries (past the
/// archive's own header block — see the identical distinction
/// `conversion::archive::find_unsafe_entry` makes and why), their total
/// uncompressed size, and the first entry's compression method.
fn archive_info_from_listing(text: &str) -> (Option<u32>, Option<u64>, Option<String>) {
    let mut past_header = false;
    let mut count: u32 = 0;
    let mut total_size: u64 = 0;
    let mut method: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if !past_header {
            if trimmed.len() >= 5 && trimmed.chars().all(|c| c == '-') {
                past_header = true;
            }
            continue;
        }
        if trimmed.starts_with("Path = ") {
            count += 1;
        } else if let Some(size_str) = trimmed.strip_prefix("Size = ") {
            total_size += size_str.trim().parse::<u64>().unwrap_or(0);
        } else if let Some(m) = trimmed.strip_prefix("Method = ") {
            if method.is_none() {
                method = Some(m.trim().to_string());
            }
        }
    }

    if count == 0 { (None, None, None) } else { (Some(count), Some(total_size), method) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_basic() -> BasicInfo {
        BasicInfo { file_name: "test.file".to_string(), size_bytes: 1234, modified_at_ms: Some(0) }
    }

    #[test]
    fn image_info_parses_all_fields() {
        let info = image_info_from_identify_output("300|200|PNG|sRGB|8|True\n", fake_basic());
        match info {
            MetadataInfo::Image { width, height, format, colorspace, bit_depth, has_alpha, .. } => {
                assert_eq!(width, Some(300));
                assert_eq!(height, Some(200));
                assert_eq!(format.as_deref(), Some("PNG"));
                assert_eq!(colorspace.as_deref(), Some("sRGB"));
                assert_eq!(bit_depth.as_deref(), Some("8-bit"));
                assert_eq!(has_alpha, Some(true));
            }
            _ => panic!("expected Image variant"),
        }
    }

    #[test]
    fn media_info_extracts_streams_and_tags() {
        let json: Value = serde_json::from_str(
            r#"{
                "format": { "format_long_name": "QuickTime / MOV", "duration": "12.5", "bit_rate": "500000", "tags": { "title": "My Video" } },
                "streams": [
                    { "codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080, "r_frame_rate": "30/1" },
                    { "codec_type": "audio", "codec_name": "aac", "sample_rate": "48000", "channels": 2 }
                ]
            }"#,
        )
        .unwrap();
        let info = media_info_from_json(&json, fake_basic());
        match info {
            MetadataInfo::Media { container, duration_seconds, bit_rate, video, audio, tags, .. } => {
                assert_eq!(container.as_deref(), Some("QuickTime / MOV"));
                assert_eq!(duration_seconds, Some(12.5));
                assert_eq!(bit_rate, Some(500000));
                let v = video.unwrap();
                assert_eq!(v.codec.as_deref(), Some("h264"));
                assert_eq!(v.width, Some(1920));
                let a = audio.unwrap();
                assert_eq!(a.channels, Some(2));
                assert_eq!(tags.len(), 1);
                assert_eq!(tags[0].key, "title");
                assert_eq!(tags[0].value, "My Video");
            }
            _ => panic!("expected Media variant"),
        }
    }

    #[test]
    fn pdf_info_parses_page_count_and_dimensions() {
        // Reproduced from a real `mutool info -M` run against a generated
        // 3-page fixture.
        let text = "\
multipage.pdf:

PDF-1.3
Pages: 3

Retrieving info from pages 1-3...
Mediaboxes (1):
\t1\t(3 0 R):\t[ 0 0 300 200 ]
";
        let (pages, w, h) = pdf_info_from_mutool_output(text);
        assert_eq!(pages, Some(3));
        assert_eq!(w, Some(300.0));
        assert_eq!(h, Some(200.0));
    }

    #[test]
    fn archive_info_ignores_the_archives_own_header_block() {
        // Same shape as the fixtures in conversion::archive's tests: a
        // short "--" separator before the archive's own "Path = " line
        // (which must NOT be counted as an entry), then "----------"
        // before the real per-entry blocks.
        let text = "\
--
Path = C:\\archives\\sample.zip
Type = zip
Physical Size = 294

----------
Path = folder/file1.txt
Folder = -
Size = 6
Method = Copy

Path = folder/sub/file2.txt
Folder = -
Size = 8
Method = Copy
";
        let (count, size, method) = archive_info_from_listing(text);
        assert_eq!(count, Some(2));
        assert_eq!(size, Some(14));
        assert_eq!(method.as_deref(), Some("Copy"));
    }

    #[test]
    fn archive_info_returns_none_for_empty_listing() {
        let (count, size, method) = archive_info_from_listing("no entries here");
        assert_eq!(count, None);
        assert_eq!(size, None);
        assert_eq!(method, None);
    }
}
