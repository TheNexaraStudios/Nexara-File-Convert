//! Smoke tests that exercise real ffmpeg/ffprobe against tiny generated
//! fixtures in `tests/fixtures/`. These are skipped (not failed) when
//! ffmpeg/ffprobe aren't available on PATH, matching the project's rule
//! that optional-engine tests must not fail the whole suite.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn ffmpeg_available() -> bool {
    let ffmpeg = Command::new("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false);
    let ffprobe = Command::new("ffprobe").arg("-version").output().map(|o| o.status.success()).unwrap_or(false);
    ffmpeg && ffprobe
}

fn probe_duration(path: &Path) -> f64 {
    let output = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", path.to_str().unwrap()])
        .output()
        .expect("ffprobe should run");
    String::from_utf8_lossy(&output.stdout).trim().parse().unwrap_or(0.0)
}

fn run_case(fixture_name: &str, args_tail: &[&str], out_ext: &str, min_duration: f64) {
    if !ffmpeg_available() {
        eprintln!("skipping {fixture_name} -> .{out_ext}: ffmpeg/ffprobe not found on PATH");
        return;
    }

    let input = fixture(fixture_name);
    assert!(input.is_file(), "fixture {fixture_name} is missing");

    let work_dir = std::env::temp_dir().join("nexara-smoke-test").join(format!("{fixture_name}-{out_ext}"));
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).unwrap();
    let output = work_dir.join(format!("out.{out_ext}"));

    let mut args: Vec<&str> = vec!["-y", "-i", input.to_str().unwrap()];
    args.extend_from_slice(args_tail);
    let output_str = output.to_str().unwrap().to_string();
    args.push(&output_str);

    let status = Command::new("ffmpeg").args(&args).status().expect("ffmpeg should run");
    assert!(status.success(), "ffmpeg exited with a failure converting {fixture_name} -> .{out_ext}");

    let metadata = std::fs::metadata(&output).expect("output file should exist");
    assert!(metadata.len() > 0, "output file for {fixture_name} -> .{out_ext} is empty");

    let duration = probe_duration(&output);
    assert!(duration >= min_duration, "expected duration >= {min_duration}s, got {duration}s");

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn mp4_to_mp3_produces_valid_audio() {
    run_case("sample.mp4", &["-vn", "-c:a", "libmp3lame", "-b:a", "128k"], "mp3", 0.5);
}

#[test]
fn mp4_to_wav_produces_valid_audio() {
    run_case("sample.mp4", &["-vn", "-c:a", "pcm_s16le"], "wav", 0.5);
}

#[test]
fn wav_to_mp3_produces_valid_audio() {
    run_case("sample.wav", &["-c:a", "libmp3lame", "-b:a", "128k"], "mp3", 0.5);
}

#[test]
fn mp4_to_webm_transcodes_video() {
    run_case("sample.mp4", &["-c:v", "libvpx-vp9", "-crf", "30", "-b:v", "0", "-c:a", "libopus"], "webm", 0.5);
}

#[test]
fn mkv_to_mp4_remux_stream_copy() {
    run_case("sample.mkv", &["-c", "copy"], "mp4", 0.5);
}
