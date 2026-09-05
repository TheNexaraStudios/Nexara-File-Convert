use serde::Serialize;

/// A unified "what is this file" result, shared across engines (ffmpeg for
/// audio/video, ImageMagick for raster images). Fields that don't apply to
/// a given engine are simply left `None`/`false`.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaProbe {
    pub duration_seconds: Option<f64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub has_video: bool,
    pub has_audio: bool,
}
