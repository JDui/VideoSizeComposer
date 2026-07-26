use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use filetime::{set_file_mtime, FileTime};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    ffi::OsString,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager};
use uuid::Uuid;
use walkdir::WalkDir;

const VIDEO_EXTENSIONS: &[&str] = &[
    "3g2", "3gp", "avi", "flv", "m2ts", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "mts", "mxf",
    "ogv", "ts", "webm", "wmv",
];
const SEQUENCE_EXTENSIONS: &[&str] = &[
    "bmp", "dpx", "exr", "jpeg", "jpg", "png", "tga", "tif", "tiff", "webp",
];
const CANCELLED_ERROR: &str = "__VSC_CANCELLED__";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlatformInfo {
    os: String,
    accelerators: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolStatus {
    ffmpeg: String,
    ffprobe: String,
    encoders: Vec<String>,
    ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Preset {
    id: String,
    name: String,
    codec: String,
    resolution_mode: String,
    #[serde(default = "default_short_edge")]
    short_edge: u32,
    #[serde(default = "default_scale_percent")]
    scale_percent: u32,
    #[serde(default = "default_custom_width")]
    custom_width: u32,
    #[serde(default = "default_custom_height")]
    custom_height: u32,
    bitrate_mode: String,
    bitrate_multiplier: f64,
    target_bitrate_mbps: f64,
    hardware: String,
    output_mode: String,
    output_dir: String,
    #[serde(default = "default_output_container")]
    output_container: String,
    naming_mode: String,
    prefix: String,
    suffix: String,
    keep_times: bool,
    keep_panorama: bool,
    #[serde(default = "default_color_space")]
    color_space: String,
    #[serde(default = "default_hdr_mode")]
    hdr_mode: String,
    #[serde(
        default = "default_preset_bit_depth",
        deserialize_with = "deserialize_bit_depth"
    )]
    bit_depth: String,
    #[serde(default = "default_preset_chroma")]
    chroma: String,
    #[serde(default)]
    lut_enabled: bool,
    #[serde(default)]
    lut_name: String,
    #[serde(default = "default_lut_intensity")]
    lut_intensity: u8,
    #[serde(default = "default_true")]
    cpu_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueItem {
    id: String,
    source: String,
    file_name: String,
    codec: String,
    width: u32,
    height: u32,
    fps: String,
    bitrate: u64,
    duration: f64,
    size_bytes: u64,
    #[serde(default, skip_deserializing)]
    thumbnail: String,
    is_panorama: bool,
    #[serde(default)]
    panorama_tagged: bool,
    #[serde(default = "default_bit_depth_u32")]
    bit_depth: u32,
    #[serde(default = "default_chroma")]
    chroma: String,
    #[serde(default = "default_color_space")]
    color_space: String,
    #[serde(default)]
    color_transfer: String,
    #[serde(default = "default_hdr_mode")]
    hdr_mode: String,
    #[serde(default)]
    audio_tracks: u32,
    #[serde(default)]
    subtitle_tracks: u32,
    preset_id: String,
    selected: bool,
    output: String,
    status: String,
    progress: u8,
    #[serde(default = "default_media_kind")]
    media_kind: String,
    #[serde(default)]
    sequence_pattern: String,
    #[serde(default)]
    sequence_start_number: u64,
    #[serde(default)]
    sequence_frame_count: u32,
    #[serde(default = "default_sequence_fps")]
    sequence_fps: f64,
    #[serde(default = "default_pixel_aspect")]
    sequence_pixel_aspect: f64,
    #[serde(default)]
    sequence_frames: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EncodeJob {
    item: QueueItem,
    preset: Preset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EncodeProgress {
    item_id: String,
    progress: u8,
    status: String,
    output: Option<String>,
    ok: Option<bool>,
    message: Option<String>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_platform_info,
            get_tool_status,
            load_presets,
            save_preset,
            delete_preset,
            import_lut_files,
            probe_paths,
            probe_sequence_paths,
            start_encode,
            cancel_encode
        ])
        .setup(|app| {
            let path = presets_path(app.handle())?;
            if !path.exists() {
                write_presets(&path, &default_presets())?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running VideoSize Composer");
}

#[tauri::command]
fn get_platform_info() -> PlatformInfo {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    };

    let accelerators = if cfg!(target_os = "windows") {
        vec!["auto", "cuda", "cpu"]
    } else if cfg!(target_os = "macos") {
        vec!["auto", "metal", "cpu"]
    } else {
        vec!["auto", "cpu"]
    };

    PlatformInfo {
        os: os.to_string(),
        accelerators: accelerators.into_iter().map(str::to_string).collect(),
    }
}

#[tauri::command]
fn get_tool_status() -> ToolStatus {
    let ffmpeg = resolve_tool("ffmpeg").to_string_lossy().to_string();
    let ffprobe = resolve_tool("ffprobe").to_string_lossy().to_string();
    let encoders = detected_encoders();
    let ffmpeg_ok = PathBuf::from(&ffmpeg).exists() || command_exists("ffmpeg");
    let ffprobe_ok = PathBuf::from(&ffprobe).exists() || command_exists("ffprobe");
    ToolStatus {
        ffmpeg,
        ffprobe,
        encoders,
        ok: ffmpeg_ok && ffprobe_ok,
    }
}

fn detected_encoders() -> Vec<String> {
    static ENCODERS: OnceLock<Vec<String>> = OnceLock::new();
    ENCODERS
        .get_or_init(|| {
            let mut encoders = Vec::new();
            let output = command_with_hidden_window(resolve_tool("ffmpeg"))
                .args(["-hide_banner", "-encoders"])
                .output();
            if let Ok(output) = output {
                let text = String::from_utf8_lossy(&output.stdout);
                for encoder in [
                    "libx264",
                    "libx265",
                    "libaom-av1",
                    "av1_nvenc",
                    "prores_ks",
                    "h264_nvenc",
                    "hevc_nvenc",
                    "h264_videotoolbox",
                    "hevc_videotoolbox",
                ] {
                    if text.contains(encoder) {
                        encoders.push(encoder.to_string());
                    }
                }
            }
            encoders
        })
        .clone()
}

#[tauri::command]
fn load_presets(app: tauri::AppHandle) -> Result<Vec<Preset>, String> {
    let path = presets_path(&app).map_err(|error| error.to_string())?;
    if !path.exists() {
        write_presets(&path, &default_presets()).map_err(|error| error.to_string())?;
    }
    read_presets(&path).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_preset(app: tauri::AppHandle, preset: Preset) -> Result<Vec<Preset>, String> {
    let path = presets_path(&app).map_err(|error| error.to_string())?;
    let mut presets = read_presets(&path).unwrap_or_else(|_| default_presets());
    if let Some(existing) = presets.iter_mut().find(|item| item.id == preset.id) {
        *existing = preset;
    } else {
        presets.push(preset);
    }
    write_presets(&path, &presets).map_err(|error| error.to_string())?;
    Ok(presets)
}

#[tauri::command]
fn delete_preset(app: tauri::AppHandle, id: String) -> Result<Vec<Preset>, String> {
    let path = presets_path(&app).map_err(|error| error.to_string())?;
    let mut presets = read_presets(&path).unwrap_or_else(|_| default_presets());
    presets.retain(|preset| preset.id != id);
    write_presets(&path, &presets).map_err(|error| error.to_string())?;
    Ok(presets)
}

#[tauri::command]
fn import_lut_files(app: tauri::AppHandle, paths: Vec<String>) -> Result<Vec<String>, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?
        .join("luts");
    fs::create_dir_all(&base).map_err(|error| error.to_string())?;
    let mut imported = Vec::new();

    for raw in paths {
        let source = PathBuf::from(&raw);
        if !source.is_file() || !is_lut(&source) {
            continue;
        }
        let Some(file_name) = source.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let target = unique_child_path(&base, file_name);
        fs::copy(&source, &target)
            .map_err(|error| format!("无法导入 LUT {}: {error}", source.display()))?;
        imported.push(target.to_string_lossy().to_string());
    }

    Ok(imported)
}

#[tauri::command]
async fn probe_paths(paths: Vec<String>, preset_id: String) -> Result<Vec<QueueItem>, String> {
    tauri::async_runtime::spawn_blocking(move || probe_paths_impl(paths, preset_id))
        .await
        .map_err(|error| format!("媒体探测任务异常：{error}"))?
}

fn probe_paths_impl(paths: Vec<String>, preset_id: String) -> Result<Vec<QueueItem>, String> {
    let video_paths = collect_video_files(paths);
    if video_paths.is_empty() {
        return Err("没有找到支持的视频文件".into());
    }
    let mut items = Vec::new();
    let mut errors = Vec::new();
    for path in video_paths {
        match probe_video(&path, &preset_id) {
            Ok(item) => items.push(item),
            Err(error) => errors.push(format!("{}: {error}", path.display())),
        }
    }
    if items.is_empty() {
        return Err(format!("无法读取视频信息：\n{}", errors.join("\n")));
    }
    for error in errors {
        eprintln!("probe failed: {error}");
    }
    Ok(items)
}

#[tauri::command]
async fn probe_sequence_paths(
    paths: Vec<String>,
    preset_id: String,
    default_fps: f64,
) -> Result<Vec<QueueItem>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let groups = collect_sequence_groups(paths);
        if groups.is_empty() {
            return Err("没有找到支持的序列帧（PNG/JPEG/TIFF/BMP/WebP/EXR/DPX/TGA）".into());
        }
        let fps = if default_fps.is_finite() && default_fps > 0.0 {
            default_fps
        } else {
            default_sequence_fps()
        };
        let mut items = Vec::new();
        let mut errors = Vec::new();
        for frames in groups {
            match probe_sequence(&frames, &preset_id, fps) {
                Ok(item) => items.push(item),
                Err(error) => errors.push(error),
            }
        }
        if items.is_empty() {
            return Err(format!("无法读取序列帧信息：\n{}", errors.join("\n")));
        }
        Ok(items)
    })
    .await
    .map_err(|error| format!("序列探测任务异常：{error}"))?
}

fn collect_sequence_groups(paths: Vec<String>) -> Vec<Vec<PathBuf>> {
    let mut groups: HashMap<String, Vec<(u64, PathBuf)>> = HashMap::new();
    let mut requested_keys = Vec::new();
    let has_directory = paths.iter().any(|raw| Path::new(raw).is_dir());

    for raw in paths {
        let path = PathBuf::from(raw);
        if path.is_dir() {
            for entry in WalkDir::new(&path).into_iter().filter_map(Result::ok) {
                let candidate = entry.path();
                if candidate.is_file() && is_sequence_frame(candidate) {
                    if let Some((key, frame)) = sequence_key(candidate) {
                        groups
                            .entry(key)
                            .or_default()
                            .push((frame, candidate.to_path_buf()));
                    }
                }
            }
        } else if path.is_file() && is_sequence_frame(&path) {
            if let Some((key, _)) = sequence_key(&path) {
                requested_keys.push(key.clone());
                if let Some(parent) = path.parent() {
                    if let Ok(entries) = fs::read_dir(parent) {
                        for entry in entries.filter_map(Result::ok) {
                            let candidate = entry.path();
                            if candidate.is_file() && is_sequence_frame(&candidate) {
                                if let Some((candidate_key, frame)) = sequence_key(&candidate) {
                                    if candidate_key == key {
                                        groups
                                            .entry(key.clone())
                                            .or_default()
                                            .push((frame, candidate));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if !has_directory && !requested_keys.is_empty() {
        groups.retain(|key, _| requested_keys.contains(key));
    }
    let mut result: Vec<Vec<PathBuf>> = groups
        .into_values()
        .filter_map(|mut entries| {
            entries.sort_by(|(left_number, left_path), (right_number, right_path)| {
                left_number
                    .cmp(right_number)
                    .then_with(|| left_path.cmp(right_path))
            });
            entries.dedup_by(|left, right| left.1 == right.1);
            (!entries.is_empty()).then(|| entries.into_iter().map(|(_, path)| path).collect())
        })
        .collect();
    result.sort_by(|left, right| left.first().cmp(&right.first()));
    result
}

fn sequence_key(path: &Path) -> Option<(String, u64)> {
    let stem = path.file_stem()?.to_str()?;
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    let (prefix, suffix, frame) = if let Some((prefix, digits, suffix)) = split_sequence_stem(stem)
    {
        (prefix, suffix, digits.parse::<u64>().unwrap_or(0))
    } else {
        (stem, "", 0)
    };
    let parent = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_string_lossy();
    Some((
        format!(
            "{}|{}|{}|{}",
            parent.to_lowercase(),
            prefix.to_lowercase(),
            suffix.to_lowercase(),
            extension
        ),
        frame,
    ))
}

fn split_sequence_stem(stem: &str) -> Option<(&str, &str, &str)> {
    let chars: Vec<(usize, char)> = stem.char_indices().collect();
    let last_index = chars
        .iter()
        .rposition(|(_, value)| value.is_ascii_digit())?;
    let mut first_index = last_index;
    while first_index > 0 && chars[first_index - 1].1.is_ascii_digit() {
        first_index -= 1;
    }
    let start = chars[first_index].0;
    let end = chars[last_index].0 + chars[last_index].1.len_utf8();
    Some((&stem[..start], &stem[start..end], &stem[end..]))
}

fn sequence_base_name(stem: &str) -> String {
    let (prefix, _, suffix) = split_sequence_stem(stem).unwrap_or((stem, "", ""));
    let prefix = prefix.trim_end_matches(['_', '-', '.', ' ']);
    let suffix = suffix.trim_start_matches(['_', '-', '.', ' ']);
    match (prefix.is_empty(), suffix.is_empty()) {
        (false, false) => format!("{prefix}_{suffix}"),
        (false, true) => prefix.to_string(),
        (true, false) => suffix.to_string(),
        (true, true) => "sequence".into(),
    }
}

fn is_sequence_frame(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| SEQUENCE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn probe_sequence(frames: &[PathBuf], preset_id: &str, fps: f64) -> Result<QueueItem, String> {
    let first = frames.first().ok_or_else(|| "空序列".to_string())?;
    let mut item = probe_video(first, preset_id)?;
    let size_bytes = frames
        .iter()
        .filter_map(|path| path.metadata().ok())
        .map(|metadata| metadata.len())
        .sum();
    let (start_number, end_number) = (
        sequence_key(first).map(|(_, frame)| frame).unwrap_or(0),
        frames
            .last()
            .and_then(|path| sequence_key(path))
            .map(|(_, frame)| frame)
            .unwrap_or(0),
    );
    let extension = first
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("image")
        .to_ascii_uppercase();
    let stem = first
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("sequence");
    let display_stem = sequence_base_name(stem);
    item.file_name = if frames.len() > 1 {
        format!(
            "{display_stem} [{start_number}–{end_number}].{}",
            extension.to_ascii_lowercase()
        )
    } else {
        first
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("sequence")
            .to_string()
    };
    item.codec = format!("{extension} 序列");
    item.fps = format_fps(fps);
    item.bitrate = if frames.len() > 0 {
        ((size_bytes as f64 * 8.0) / (frames.len() as f64 / fps)).max(0.0) as u64
    } else {
        0
    };
    item.duration = frames.len() as f64 / fps;
    item.size_bytes = size_bytes;
    item.media_kind = "sequence".into();
    item.sequence_pattern = sequence_display_pattern(first);
    item.sequence_start_number = start_number;
    item.sequence_frame_count = frames.len() as u32;
    item.sequence_fps = fps;
    item.sequence_pixel_aspect = default_pixel_aspect();
    item.sequence_frames = frames
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    item.audio_tracks = 0;
    item.subtitle_tracks = 0;
    item.is_panorama = false;
    item.panorama_tagged = false;
    Ok(item)
}

fn sequence_display_pattern(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("sequence");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let Some((prefix, digits, suffix)) = split_sequence_stem(stem) else {
        return format!("{stem}.{extension}");
    };
    format!("{prefix}%0{}d{suffix}.{extension}", digits.len())
}

fn format_fps(fps: f64) -> String {
    if (fps.fract()).abs() < 0.001 {
        format!("{fps:.0} fps")
    } else {
        format!(
            "{} fps",
            format!("{fps:.3}")
                .trim_end_matches('0')
                .trim_end_matches('.')
        )
    }
}

#[tauri::command]
async fn start_encode(app: tauri::AppHandle, jobs: Vec<EncodeJob>) -> Result<String, String> {
    if jobs.is_empty() {
        return Err("没有可编码的任务".into());
    }
    preflight_jobs(&jobs)?;
    let session_id = Uuid::new_v4().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    encode_sessions()
        .lock()
        .map_err(|_| "无法创建编码会话".to_string())?
        .insert(session_id.clone(), cancel.clone());
    let thread_app = app.clone();
    let thread_session_id = session_id.clone();
    thread::spawn(move || {
        for job in jobs {
            if cancel.load(Ordering::Relaxed) {
                emit_cancelled(&thread_app, job.item.id);
                continue;
            }
            if let Err((item_id, message)) = encode_job(&thread_app, job, &cancel) {
                let _ = thread_app.emit(
                    "encode-progress",
                    EncodeProgress {
                        item_id,
                        progress: 0,
                        status: "失败".into(),
                        output: None,
                        ok: Some(false),
                        message: Some(message),
                    },
                );
            }
        }
        if let Ok(mut sessions) = encode_sessions().lock() {
            sessions.remove(&thread_session_id);
        }
    });
    Ok(session_id)
}

fn preflight_jobs(jobs: &[EncodeJob]) -> Result<(), String> {
    let mut required_by_directory: HashMap<PathBuf, u64> = HashMap::new();
    for job in jobs {
        validate_job(job)?;
        let source = Path::new(&job.item.source);
        if !source.is_file() {
            return Err(format!("源媒体不存在：{}", source.display()));
        }
        if job.item.media_kind == "sequence"
            && (job.item.sequence_frames.is_empty()
                || job
                    .item
                    .sequence_frames
                    .iter()
                    .any(|frame| !Path::new(frame).is_file()))
        {
            return Err(format!(
                "序列“{}”包含缺失帧，请重新载入",
                job.item.file_name
            ));
        }
        let output = build_output_path(&job.item, &job.preset);
        let directory = output
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        fs::create_dir_all(&directory)
            .map_err(|error| format!("无法创建输出目录 {}：{error}", directory.display()))?;
        let probe = directory.join(format!(".vsc-write-test-{}.tmp", Uuid::new_v4()));
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
            .map_err(|error| format!("输出目录不可写 {}：{error}", directory.display()))?;
        let _ = fs::remove_file(&probe);
        let required = estimated_output_bytes(&job.item, &job.preset);
        required_by_directory
            .entry(directory)
            .and_modify(|total| *total = total.saturating_add(required))
            .or_insert(required);
    }

    for (directory, required) in required_by_directory {
        let available = fs2::available_space(&directory).map_err(|error| {
            format!("无法读取输出目录剩余空间 {}：{error}", directory.display())
        })?;
        let reserve = 64 * 1024 * 1024;
        if available < required.saturating_add(reserve) {
            return Err(format!(
                "输出空间不足：{} 需要约 {} MB，可用 {} MB",
                directory.display(),
                required / 1024 / 1024,
                available / 1024 / 1024
            ));
        }
    }
    Ok(())
}

fn validate_preset(preset: &Preset) -> Result<(), String> {
    if !matches!(preset.codec.as_str(), "h264" | "h265" | "av1" | "prores") {
        return Err(format!("不支持的编码格式：{}", preset.codec));
    }
    if !matches!(preset.color_space.as_str(), "source" | "rec709" | "rec2020") {
        return Err(format!("不支持的色彩空间：{}", preset.color_space));
    }
    if !matches!(
        preset.hdr_mode.as_str(),
        "source" | "sdr" | "hlg" | "hdr10" | "dolby_vision"
    ) {
        return Err(format!("不支持的 HDR 模式：{}", preset.hdr_mode));
    }
    if !matches!(preset.bit_depth.as_str(), "source" | "8" | "10") {
        return Err(format!("不支持的色深：{}", preset.bit_depth));
    }
    if !matches!(preset.chroma.as_str(), "source" | "420" | "422") {
        return Err(format!("不支持的色度采样：{}", preset.chroma));
    }
    if !matches!(
        preset.output_container.as_str(),
        "source" | "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" | "m4a"
    ) {
        return Err(format!("不支持的封装格式：{}", preset.output_container));
    }
    if preset.lut_enabled && !Path::new(&preset.lut_name).is_file() {
        return Err(format!("LUT 文件不存在：{}", preset.lut_name));
    }
    if preset.lut_enabled && !filter_available("lut3d") {
        return Err("当前 FFmpeg 不包含 lut3d 滤镜，无法套用 LUT".into());
    }
    if (preset.color_space != "source"
        || !matches!(preset.hdr_mode.as_str(), "source" | "dolby_vision"))
        && !filter_available("zscale")
    {
        return Err("当前 FFmpeg 不包含 zscale 滤镜，无法执行 HDR/色彩转换".into());
    }
    if matches!(preset.hdr_mode.as_str(), "hlg" | "hdr10")
        && (!matches!(preset.codec.as_str(), "h265" | "av1" | "prores")
            || preset_bit_depth(preset).is_some_and(|depth| depth != 10))
    {
        return Err("HLG/HDR10 输出需要 H.265、AV1 或 ProRes，并使用 10-bit".into());
    }
    Ok(())
}

fn validate_job(job: &EncodeJob) -> Result<(), String> {
    validate_preset(&job.preset)?;
    let container = output_extension(&job.item, &job.preset);
    if container == "m4a" && job.item.audio_tracks == 0 {
        return Err(format!("“{}”没有音轨，不能导出为 M4A", job.item.file_name));
    }
    if container == "webm" && job.preset.codec != "av1" {
        return Err(format!(
            "WebM 封装当前仅支持 AV1 输出；“{}”请改用 AV1 或选择 MP4/MOV/MKV",
            job.item.file_name
        ));
    }
    if matches!(job.preset.hdr_mode.as_str(), "hlg" | "hdr10")
        && resolved_bit_depth(&job.item, &job.preset) != 10
    {
        return Err(format!(
            "{} 的原视频不是 10-bit，HLG/HDR10 输出请明确选择 10-bit",
            job.item.file_name
        ));
    }
    if job.preset.hdr_mode == "dolby_vision" {
        if job.item.hdr_mode != "dolby_vision" {
            return Err(format!(
                "{} 不是 Dolby Vision 源视频，无法执行杜比视界保留导出",
                job.item.file_name
            ));
        }
        if job.preset.codec != "h265"
            || job.preset.resolution_mode != "source"
            || job.preset.lut_enabled
        {
            return Err("杜比视界保留导出必须使用 H.265、保持原分辨率且不套用 LUT；该模式会无损复制原视频流以保留 RPU 元数据".into());
        }
    }
    Ok(())
}

fn estimated_output_bytes(item: &QueueItem, preset: &Preset) -> u64 {
    if preset.hdr_mode == "dolby_vision" {
        return item.size_bytes;
    }
    if preset.codec == "prores" {
        return item.size_bytes.saturating_mul(2).max(64 * 1024 * 1024);
    }
    let bits_per_second = target_bitrate(item, preset).saturating_add(320_000);
    ((bits_per_second as f64 * item.duration.max(1.0) / 8.0) * 1.10) as u64
}

#[tauri::command]
fn cancel_encode(session_id: String) -> Result<(), String> {
    let sessions = encode_sessions()
        .lock()
        .map_err(|_| "无法访问编码会话".to_string())?;
    let cancel = sessions
        .get(&session_id)
        .ok_or_else(|| "编码会话已结束或不存在".to_string())?;
    cancel.store(true, Ordering::Relaxed);
    Ok(())
}

fn encode_sessions() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    static SESSIONS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn emit_cancelled(app: &tauri::AppHandle, item_id: String) {
    let _ = app.emit(
        "encode-progress",
        EncodeProgress {
            item_id,
            progress: 0,
            status: "已取消".into(),
            output: None,
            ok: Some(false),
            message: None,
        },
    );
}

fn default_presets() -> Vec<Preset> {
    vec![
        preset_with_defaults(
            "h265-source-30",
            "H.265 原视频参数 30%码率",
            "h265",
            "_h265_source",
        ),
        {
            let mut preset = preset_with_defaults(
                "h264-source-30",
                "H.264 原分辨率 30%码率",
                "h264",
                "_h264_30pct",
            );
            preset.bit_depth = "8".into();
            preset
        },
        {
            let mut preset =
                preset_with_defaults("av1-1080-10bit", "AV1 1080p 10bit", "av1", "_av1_1080p");
            preset.resolution_mode = "short_edge".into();
            preset.short_edge = 1080;
            preset.bitrate_mode = "target_mbps".into();
            preset.target_bitrate_mbps = 8.0;
            preset.bit_depth = "10".into();
            preset
        },
        {
            let mut preset =
                preset_with_defaults("prores-422-lt", "ProRes 422 LT", "prores", "_prores422lt");
            preset.hardware = "cpu".into();
            preset.bitrate_multiplier = 1.0;
            preset.target_bitrate_mbps = 100.0;
            preset.chroma = "422".into();
            preset
        },
        {
            let mut preset =
                preset_with_defaults("h265-hlg-10bit", "H.265 HLG 10bit", "h265", "_hlg");
            preset.color_space = "rec2020".into();
            preset.hdr_mode = "hlg".into();
            preset.bit_depth = "10".into();
            preset.hardware = "cpu".into();
            preset
        },
        {
            let mut preset =
                preset_with_defaults("h265-hdr10-10bit", "H.265 HDR10 10bit", "h265", "_hdr10");
            preset.color_space = "rec2020".into();
            preset.hdr_mode = "hdr10".into();
            preset.bit_depth = "10".into();
            preset.hardware = "cpu".into();
            preset
        },
        {
            let mut preset = preset_with_defaults(
                "dolby-vision-preserve",
                "Dolby Vision 保留导出",
                "h265",
                "_dovi",
            );
            preset.hdr_mode = "dolby_vision".into();
            preset.bit_depth = "10".into();
            preset.chroma = "420".into();
            preset
        },
    ]
}

fn preset_with_defaults(id: &str, name: &str, codec: &str, suffix: &str) -> Preset {
    Preset {
        id: id.into(),
        name: name.into(),
        codec: codec.into(),
        resolution_mode: "source".into(),
        short_edge: default_short_edge(),
        scale_percent: default_scale_percent(),
        custom_width: default_custom_width(),
        custom_height: default_custom_height(),
        bitrate_mode: "source_multiplier".into(),
        bitrate_multiplier: 0.30,
        target_bitrate_mbps: 20.0,
        hardware: "auto".into(),
        output_mode: "subfolder".into(),
        output_dir: String::new(),
        output_container: if codec == "prores" {
            "mov".into()
        } else {
            default_output_container()
        },
        naming_mode: "suffix_prefix".into(),
        prefix: String::new(),
        suffix: suffix.into(),
        keep_times: true,
        keep_panorama: true,
        color_space: default_color_space(),
        hdr_mode: default_hdr_mode(),
        bit_depth: default_preset_bit_depth(),
        chroma: default_preset_chroma(),
        lut_enabled: false,
        lut_name: String::new(),
        lut_intensity: default_lut_intensity(),
        cpu_fallback: true,
    }
}

fn default_short_edge() -> u32 {
    1080
}

fn default_scale_percent() -> u32 {
    50
}

fn default_custom_width() -> u32 {
    1920
}

fn default_custom_height() -> u32 {
    1080
}

fn default_output_container() -> String {
    "source".into()
}

fn default_media_kind() -> String {
    "video".into()
}

fn default_sequence_fps() -> f64 {
    30.0
}

fn default_pixel_aspect() -> f64 {
    1.0
}

fn default_color_space() -> String {
    "source".into()
}

fn default_hdr_mode() -> String {
    "source".into()
}

fn default_preset_bit_depth() -> String {
    "source".into()
}

fn default_bit_depth_u32() -> u32 {
    10
}

fn default_chroma() -> String {
    "420".into()
}

fn default_preset_chroma() -> String {
    "source".into()
}

fn deserialize_bit_depth<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Number(number) => number.to_string(),
        Value::String(value) => value,
        _ => default_preset_bit_depth(),
    })
}

fn default_lut_intensity() -> u8 {
    80
}

fn default_true() -> bool {
    true
}

fn presets_path(app: &tauri::AppHandle) -> tauri::Result<PathBuf> {
    let dir = app.path().app_config_dir()?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("presets.json"))
}

fn read_presets(path: &Path) -> std::io::Result<Vec<Preset>> {
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text).unwrap_or_else(|_| default_presets()))
}

fn write_presets(path: &Path, presets: &[Preset]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(presets).unwrap())
}

fn collect_video_files(paths: Vec<String>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for raw in paths {
        let path = PathBuf::from(raw);
        if path.is_dir() {
            for entry in WalkDir::new(path).into_iter().filter_map(Result::ok) {
                let candidate = entry.path();
                if candidate.is_file() && is_video(candidate) {
                    files.push(candidate.to_path_buf());
                }
            }
        } else if path.is_file() && is_video(&path) {
            files.push(path);
        }
    }
    files.sort();
    files.dedup();
    files
}

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| VIDEO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn is_lut(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "cube" | "3dl" | "lut"
            )
        })
        .unwrap_or(false)
}

fn unique_child_path(dir: &Path, file_name: &str) -> PathBuf {
    let candidate = dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("lut");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("cube");
    for index in 1..10_000 {
        let candidate = dir.join(format!("{stem}-{index}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{stem}-{}.{}", Uuid::new_v4(), extension))
}

fn probe_video(path: &Path, preset_id: &str) -> Result<QueueItem, String> {
    let output = command_with_hidden_window(resolve_tool("ffprobe"))
        .args([
            "-v",
            "error",
            "-show_format",
            "-show_streams",
            "-print_format",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let json: Value = serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let video = json["streams"]
        .as_array()
        .and_then(|streams| {
            streams
                .iter()
                .find(|stream| stream["codec_type"] == "video")
        })
        .cloned()
        .unwrap_or(Value::Null);
    let format = &json["format"];
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("video")
        .to_string();
    let bit_rate = video["bit_rate"]
        .as_str()
        .or_else(|| format["bit_rate"].as_str())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let duration = video["duration"]
        .as_str()
        .or_else(|| format["duration"].as_str())
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    let streams = json["streams"].as_array().cloned().unwrap_or_default();
    let pixel_format = video["pix_fmt"].as_str().unwrap_or("");
    let color_transfer = video["color_transfer"].as_str().unwrap_or("").to_string();
    let hdr_mode = detect_hdr_mode(&video);

    Ok(QueueItem {
        id: Uuid::new_v4().to_string(),
        source: path.to_string_lossy().to_string(),
        file_name,
        codec: video["codec_name"]
            .as_str()
            .unwrap_or("unknown")
            .to_uppercase(),
        width: video["width"].as_u64().unwrap_or(0) as u32,
        height: video["height"].as_u64().unwrap_or(0) as u32,
        fps: fps_label(
            video["avg_frame_rate"]
                .as_str()
                .or_else(|| video["r_frame_rate"].as_str())
                .unwrap_or(""),
        ),
        bitrate: bit_rate,
        duration,
        size_bytes: path.metadata().map(|meta| meta.len()).unwrap_or(0),
        thumbnail: generate_thumbnail(path, duration),
        is_panorama: detect_panorama(&json),
        panorama_tagged: detect_panorama_tagged(&json),
        bit_depth: detect_bit_depth(&video, pixel_format),
        chroma: detect_chroma(pixel_format),
        color_space: video["color_primaries"]
            .as_str()
            .or_else(|| video["color_space"].as_str())
            .unwrap_or("unknown")
            .to_string(),
        color_transfer,
        hdr_mode,
        audio_tracks: streams
            .iter()
            .filter(|stream| stream["codec_type"] == "audio")
            .count() as u32,
        subtitle_tracks: streams
            .iter()
            .filter(|stream| stream["codec_type"] == "subtitle")
            .count() as u32,
        preset_id: preset_id.to_string(),
        selected: true,
        output: String::new(),
        status: "等待中".into(),
        progress: 0,
        media_kind: default_media_kind(),
        sequence_pattern: String::new(),
        sequence_start_number: 0,
        sequence_frame_count: 0,
        sequence_fps: default_sequence_fps(),
        sequence_pixel_aspect: default_pixel_aspect(),
        sequence_frames: Vec::new(),
    })
}

fn generate_thumbnail(path: &Path, duration: f64) -> String {
    let seek_seconds = if duration > 0.0 {
        (duration * 0.1).clamp(0.1, 3.0)
    } else {
        0.1
    };
    let seek = format!("{seek_seconds:.3}");
    let output = command_with_hidden_window(resolve_tool("ffmpeg"))
        .args(["-v", "error", "-ss", seek.as_str(), "-i"])
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale=280:-2",
            "-f",
            "image2pipe",
            "-c:v",
            "mjpeg",
            "pipe:1",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() && !output.stdout.is_empty() => {
            format!(
                "data:image/jpeg;base64,{}",
                BASE64_STANDARD.encode(output.stdout)
            )
        }
        _ => String::new(),
    }
}

struct TemporaryFileGuard(PathBuf);

impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn encode_job(
    app: &tauri::AppHandle,
    job: EncodeJob,
    cancel: &Arc<AtomicBool>,
) -> Result<(), (String, String)> {
    let item_id = job.item.id.clone();
    let source = PathBuf::from(&job.item.source);
    let output = unique_output_path(&build_output_path(&job.item, &job.preset));
    let temporary_output = temporary_output_path(&output);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| (item_id.clone(), error.to_string()))?;
    }

    let sequence_list = if job.item.media_kind == "sequence" {
        Some(
            write_sequence_concat_file(&job.item, &temporary_output)
                .map_err(|error| (item_id.clone(), error))?,
        )
    } else {
        None
    };
    let _sequence_list_guard = sequence_list.clone().map(TemporaryFileGuard);
    let args = build_ffmpeg_args_with_sequence(
        &job.item,
        &job.preset,
        &temporary_output,
        sequence_list.as_deref(),
    );
    let _ = app.emit(
        "encode-progress",
        EncodeProgress {
            item_id: item_id.clone(),
            progress: 0,
            status: "编码中".into(),
            output: Some(output.to_string_lossy().to_string()),
            ok: None,
            message: Some(format!("ffmpeg {}", shell_join(&args))),
        },
    );

    if let Err(first_error) = run_ffmpeg(app, &item_id, &job.item, &args, &output, cancel) {
        if first_error == CANCELLED_ERROR {
            let _ = fs::remove_file(&temporary_output);
            emit_cancelled(app, item_id);
            return Ok(());
        }
        if job.preset.cpu_fallback
            && job.preset.hardware != "cpu"
            && job.preset.hdr_mode != "dolby_vision"
        {
            let mut cpu_preset = job.preset.clone();
            cpu_preset.hardware = "cpu".into();
            let cpu_args = build_ffmpeg_args_with_sequence(
                &job.item,
                &cpu_preset,
                &temporary_output,
                sequence_list.as_deref(),
            );
            let _ = app.emit(
                "encode-progress",
                EncodeProgress {
                    item_id: item_id.clone(),
                    progress: 0,
                    status: "硬件编码失败，回退 CPU".into(),
                    output: Some(output.to_string_lossy().to_string()),
                    ok: None,
                    message: Some(first_error.clone()),
                },
            );
            if let Err(error) = run_ffmpeg(app, &item_id, &job.item, &cpu_args, &output, cancel) {
                let _ = fs::remove_file(&temporary_output);
                if error == CANCELLED_ERROR {
                    emit_cancelled(app, item_id);
                    return Ok(());
                }
                return Err((
                    item_id.clone(),
                    format!("{first_error}\nCPU 回退失败: {error}"),
                ));
            }
        } else {
            let _ = fs::remove_file(&temporary_output);
            return Err((item_id, first_error));
        }
    }

    if cancel.load(Ordering::Relaxed) {
        let _ = fs::remove_file(&temporary_output);
        emit_cancelled(app, item_id);
        return Ok(());
    }

    if job.preset.keep_panorama && job.item.is_panorama && is_isobmff_path(&temporary_output) {
        if let Err(error) = inject_spherical_metadata(&temporary_output) {
            let _ = fs::remove_file(&temporary_output);
            return Err((item_id, format!("无法写入标准全景元数据: {error}")));
        }
        if let Err(error) = verify_spherical_metadata(&temporary_output) {
            let _ = fs::remove_file(&temporary_output);
            return Err((item_id, error));
        }
    }

    let mut timestamp_error = if job.preset.keep_times {
        preserve_times(&source, &temporary_output).err()
    } else {
        None
    };

    if let Err(error) = fs::rename(&temporary_output, &output) {
        let _ = fs::remove_file(&temporary_output);
        return Err((item_id, format!("无法完成输出文件：{error}")));
    }

    if job.preset.keep_times && timestamp_error.is_none() {
        timestamp_error = verify_preserved_times(&source, &output).err();
    }

    if let Some(error) = timestamp_error {
        let _ = app.emit(
            "encode-progress",
            EncodeProgress {
                item_id: job.item.id,
                progress: 100,
                status: "时间恢复失败".into(),
                output: Some(output.to_string_lossy().to_string()),
                ok: Some(false),
                message: Some(format!(
                    "视频已编码，但无法同时恢复源文件的创建日期和修改时间：{error}"
                )),
            },
        );
        return Ok(());
    }
    let _ = app.emit(
        "encode-progress",
        EncodeProgress {
            item_id: job.item.id,
            progress: 100,
            status: if job.preset.keep_times {
                "完成 · 已保留原始时间".into()
            } else {
                "完成".into()
            },
            output: Some(output.to_string_lossy().to_string()),
            ok: Some(true),
            message: job
                .preset
                .keep_times
                .then(|| "输出完成；创建日期和修改时间已恢复为源视频时间".into()),
        },
    );
    Ok(())
}

fn run_ffmpeg(
    app: &tauri::AppHandle,
    item_id: &str,
    item: &QueueItem,
    args: &[String],
    output: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<(), String> {
    let mut child = command_with_hidden_window(resolve_tool("ffmpeg"))
        .args(args)
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .map_err(|error| format!("无法启动 FFmpeg: {error}"))?;

    let mut tail = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CANCELLED_ERROR.into());
            }
            if !line.trim().is_empty() {
                tail.push(line.clone());
                if tail.len() > 14 {
                    tail.remove(0);
                }
            }
            if let Some(progress) = parse_progress(&line, item.duration) {
                let _ = app.emit(
                    "encode-progress",
                    EncodeProgress {
                        item_id: item_id.to_string(),
                        progress,
                        status: format!("编码中 {progress}%"),
                        output: Some(output.to_string_lossy().to_string()),
                        ok: None,
                        message: None,
                    },
                );
            }
        }
    }

    if cancel.load(Ordering::Relaxed) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(CANCELLED_ERROR.into());
    }

    let status = child.wait().map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("FFmpeg 失败: {status}\n{}", tail.join("\n")))
    }
}

fn build_output_path(item: &QueueItem, preset: &Preset) -> PathBuf {
    let source = PathBuf::from(&item.source);
    let parent = source.parent().unwrap_or_else(|| Path::new("."));
    let output_dir = match preset.output_mode.as_str() {
        "single_folder" if !preset.output_dir.trim().is_empty() => {
            PathBuf::from(&preset.output_dir)
        }
        "in_place" => parent.to_path_buf(),
        _ => parent.join("VideoSizeComposer"),
    };
    let source_stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let sequence_stem;
    let stem = if item.media_kind == "sequence" {
        sequence_stem = sequence_base_name(source_stem);
        if sequence_stem.is_empty() {
            "sequence"
        } else {
            &sequence_stem
        }
    } else {
        source_stem
    };
    let name = if preset.naming_mode == "suffix_prefix" {
        format!("{}{}{}", preset.prefix, stem, preset.suffix)
    } else {
        stem.to_string()
    };
    let extension = output_extension(item, preset);
    output_dir.join(format!("{name}.{extension}"))
}

fn output_extension(item: &QueueItem, preset: &Preset) -> String {
    if preset.output_container != "source" {
        return preset.output_container.clone();
    }
    if item.media_kind == "sequence" {
        return "mp4".into();
    }
    Path::new(&item.source)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(if preset.codec == "prores" {
            "mov"
        } else {
            "mp4"
        })
        .to_ascii_lowercase()
}

fn is_isobmff_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "mp4" | "mov" | "m4v"
            )
        })
        .unwrap_or(false)
}

fn unique_output_path(base: &Path) -> PathBuf {
    if !base.exists() {
        return base.to_path_buf();
    }
    let parent = base.parent().unwrap_or_else(|| Path::new("."));
    let stem = base
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let extension = base
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("mp4");
    for index in 1..10_000 {
        let candidate = parent.join(format!("{stem}-{index}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{stem}-{}.{}", Uuid::new_v4(), extension))
}

fn temporary_output_path(final_output: &Path) -> PathBuf {
    let parent = final_output.parent().unwrap_or_else(|| Path::new("."));
    let stem = final_output
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let extension = final_output
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("mp4");
    parent.join(format!(".{stem}.vsc-part-{}.{}", Uuid::new_v4(), extension))
}

fn write_sequence_concat_file(item: &QueueItem, output: &Path) -> Result<PathBuf, String> {
    if item.sequence_frames.is_empty() {
        return Err(format!("序列“{}”没有可用帧", item.file_name));
    }
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let path = parent.join(format!(".vsc-sequence-{}.txt", Uuid::new_v4()));
    let mut text = String::from("ffconcat version 1.0\n");
    for frame in &item.sequence_frames {
        let escaped = frame.replace('\\', "/").replace('\'', "'\\''");
        text.push_str(&format!("file '{escaped}'\n"));
    }
    fs::write(&path, text).map_err(|error| format!("无法创建序列帧清单：{error}"))?;
    Ok(path)
}

#[cfg(test)]
fn build_ffmpeg_args(item: &QueueItem, preset: &Preset, output: &Path) -> Vec<String> {
    build_ffmpeg_args_with_sequence(item, preset, output, None)
}

fn build_ffmpeg_args_with_sequence(
    item: &QueueItem,
    preset: &Preset,
    output: &Path,
    sequence_list: Option<&Path>,
) -> Vec<String> {
    let encoder = video_encoder(item, preset);
    let dolby_vision_copy = preset.hdr_mode == "dolby_vision";
    let mut args = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-progress".into(),
        "pipe:2".into(),
        "-nostats".into(),
    ];
    if let Some(list) = sequence_list {
        args.extend([
            "-f".into(),
            "concat".into(),
            "-safe".into(),
            "0".into(),
            "-i".into(),
            list.to_string_lossy().to_string(),
            "-r".into(),
            format!("{:.6}", item.sequence_fps.max(0.001)),
        ]);
    } else {
        args.extend(["-i".into(), item.source.clone()]);
    }

    if output_extension(item, preset) == "m4a" {
        args.extend([
            "-map".into(),
            "0:a:0?".into(),
            "-vn".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "320k".into(),
            output.to_string_lossy().to_string(),
        ]);
        return args;
    }

    args.extend([
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "0:a?".into(),
        "-map_metadata".into(),
        "0".into(),
    ]);

    if dolby_vision_copy {
        args.extend(["-c:v".into(), "copy".into(), "-tag:v".into(), "hvc1".into()]);
    } else {
        args.extend(["-c:v".into(), encoder.clone()]);
        if let Some(filter) = video_filter(item, preset) {
            args.extend(["-vf".into(), filter]);
        }

        if preset.codec == "prores" {
            args.extend([
                "-profile:v".into(),
                "1".into(),
                "-pix_fmt".into(),
                "yuv422p10le".into(),
            ]);
        } else {
            args.extend(["-b:v".into(), target_bitrate(item, preset).to_string()]);
            args.extend(["-pix_fmt".into(), pixel_format(item, preset)]);
        }

        match encoder.as_str() {
            "libx265" | "libx264" => args.extend(["-preset".into(), "medium".into()]),
            "libaom-av1" => {
                args.extend(["-cpu-used".into(), "6".into(), "-row-mt".into(), "1".into()])
            }
            _ => {}
        }
        add_color_output_args(&mut args, item, preset, &encoder);
        if preset.codec == "h265"
            && matches!(
                output_extension(item, preset).as_str(),
                "mp4" | "mov" | "m4v"
            )
        {
            args.extend(["-tag:v".into(), "hvc1".into()]);
        }
    }

    let audio_codec = if output_extension(item, preset) == "webm" {
        "libopus"
    } else {
        "aac"
    };
    args.extend([
        "-c:a".into(),
        audio_codec.into(),
        "-b:a".into(),
        "320k".into(),
    ]);
    if preset.keep_panorama && item.is_panorama {
        args.extend([
            "-metadata:s:v:0".into(),
            "projection=equirectangular".into(),
            "-metadata:s:v:0".into(),
            "stereo_mode=mono".into(),
        ]);
    }
    if matches!(
        output_extension(item, preset).as_str(),
        "mp4" | "mov" | "m4v"
    ) {
        let movflags = if preset.keep_panorama && item.is_panorama {
            "+use_metadata_tags"
        } else {
            "+faststart+use_metadata_tags"
        };
        args.extend(["-movflags".into(), movflags.into()]);
    }
    args.push(output.to_string_lossy().to_string());
    args
}

fn video_filter(item: &QueueItem, preset: &Preset) -> Option<String> {
    let mut filters = Vec::new();
    if item.media_kind == "sequence" {
        filters.push(format!("setpts=N/({:.6}*TB)", item.sequence_fps.max(0.001)));
        filters.push(format!(
            "scale={}:{},setsar={:.6}",
            item.width.max(2),
            item.height.max(2),
            item.sequence_pixel_aspect.max(0.001)
        ));
    }
    match preset.resolution_mode.as_str() {
        "short_edge" => {
            let edge = preset.short_edge.max(120);
            filters.push(format!(
                "scale=if(gte(iw\\,ih)\\,-2\\,{edge}):if(gte(iw\\,ih)\\,{edge}\\,-2)"
            ));
        }
        "scale_percent" => {
            let ratio = (preset.scale_percent.clamp(10, 90) as f64) / 100.0;
            filters.push(format!("scale=trunc(iw*{ratio}/2)*2:trunc(ih*{ratio}/2)*2"));
        }
        "custom" => {
            let width = preset.custom_width.max(2) / 2 * 2;
            let height = preset.custom_height.max(2) / 2 * 2;
            filters.push(format!(
                "scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2"
            ));
        }
        _ => {}
    }
    if let Some(color_filter) = color_conversion_filter(item, preset) {
        filters.push(color_filter);
    }
    if preset.lut_enabled {
        let lut = PathBuf::from(&preset.lut_name);
        if lut.exists() {
            let lut_filter = format!("lut3d=file='{}'", escape_filter_path(&preset.lut_name));
            let strength = preset.lut_intensity.min(100);
            if strength >= 100 {
                filters.push(lut_filter);
            } else if strength > 0 {
                let mix = strength as f64 / 100.0;
                filters.push(format!(
                    "split=2[vsc_base][vsc_grade];[vsc_grade]{lut_filter}[vsc_lut];[vsc_base][vsc_lut]blend=all_expr='A*(1-{mix:.4})+B*{mix:.4}'"
                ));
            }
        }
    }
    if filters.is_empty() {
        None
    } else {
        Some(filters.join(","))
    }
}

fn color_conversion_filter(item: &QueueItem, preset: &Preset) -> Option<String> {
    if preset.color_space == "source" && preset.hdr_mode == "source" {
        return None;
    }
    let input_primaries = normalized_primaries(&item.color_space);
    let input_transfer = normalized_transfer(&item.color_transfer);
    let input_matrix = if input_primaries == "bt2020" {
        "bt2020nc"
    } else {
        "bt709"
    };
    let input = format!(
        "zscale=primariesin={input_primaries}:transferin={input_transfer}:matrixin={input_matrix}:rangein=tv:primaries={input_primaries}:transfer=linear:npl=100,format=gbrpf32le"
    );
    match preset.hdr_mode.as_str() {
        "sdr" => {
            let tone_map = if matches!(item.hdr_mode.as_str(), "hlg" | "hdr10" | "dolby_vision") {
                ",tonemap=hable:desat=0"
            } else {
                ""
            };
            Some(format!(
                "{input}{tone_map},zscale=primaries=bt709:transfer=bt709:matrix=bt709:range=tv,format={}",
                pixel_format(item, preset)
            ))
        }
        "hlg" => Some(format!(
            "{input},zscale=primaries=bt2020:transfer=arib-std-b67:matrix=bt2020nc:range=tv,format={}",
            pixel_format(item, preset)
        )),
        "hdr10" => Some(format!(
            "{input},zscale=primaries=bt2020:transfer=smpte2084:matrix=bt2020nc:range=tv,format={}",
            pixel_format(item, preset)
        )),
        _ => {
            let target = if preset.color_space == "rec2020" { "bt2020" } else { "bt709" };
            let matrix = if target == "bt2020" { "bt2020nc" } else { "bt709" };
            Some(format!(
                "{input},zscale=primaries={target}:transfer={input_transfer}:matrix={matrix}:range=tv,format={}",
                pixel_format(item, preset)
            ))
        }
    }
}

fn normalized_primaries(value: &str) -> &'static str {
    if value.contains("2020") {
        "bt2020"
    } else {
        "bt709"
    }
}

fn normalized_transfer(value: &str) -> &'static str {
    match value {
        "arib-std-b67" => "arib-std-b67",
        "smpte2084" => "smpte2084",
        _ => "bt709",
    }
}

fn add_color_output_args(args: &mut Vec<String>, item: &QueueItem, preset: &Preset, encoder: &str) {
    let (primaries, transfer, matrix) = match preset.hdr_mode.as_str() {
        "sdr" => ("bt709", "bt709", "bt709"),
        "hlg" => ("bt2020", "arib-std-b67", "bt2020nc"),
        "hdr10" => ("bt2020", "smpte2084", "bt2020nc"),
        _ => (
            normalized_primaries(&item.color_space),
            normalized_transfer(&item.color_transfer),
            if normalized_primaries(&item.color_space) == "bt2020" {
                "bt2020nc"
            } else {
                "bt709"
            },
        ),
    };
    args.extend([
        "-color_primaries".into(),
        primaries.into(),
        "-color_trc".into(),
        transfer.into(),
        "-colorspace".into(),
        matrix.into(),
        "-color_range".into(),
        "tv".into(),
    ]);
    if preset.hdr_mode == "hdr10" && encoder == "libx265" {
        args.extend([
            "-x265-params".into(),
            "hdr-opt=1:repeat-headers=1:colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:master-display=G(13250,34500)B(7500,3000)R(34000,16000)WP(15635,16450)L(10000000,1):max-cll=1000,400".into(),
        ]);
    }
}

fn escape_filter_path(path: &str) -> String {
    path.replace('\\', "/")
        .chars()
        .flat_map(|value| match value {
            '\'' | ':' | ',' | '[' | ']' | ';' => vec!['\\', value],
            _ => vec![value],
        })
        .collect()
}

fn preset_bit_depth(preset: &Preset) -> Option<u8> {
    preset.bit_depth.parse::<u8>().ok()
}

fn resolved_bit_depth(item: &QueueItem, preset: &Preset) -> u8 {
    preset_bit_depth(preset).unwrap_or(if item.bit_depth > 8 { 10 } else { 8 })
}

fn resolved_chroma<'a>(item: &'a QueueItem, preset: &'a Preset) -> &'a str {
    if preset.chroma == "source" {
        item.chroma.as_str()
    } else {
        preset.chroma.as_str()
    }
}

fn pixel_format(item: &QueueItem, preset: &Preset) -> String {
    match (
        resolved_bit_depth(item, preset),
        resolved_chroma(item, preset),
    ) {
        (10, "444") => "yuv444p10le",
        (10, "422") => "yuv422p10le",
        (10, _) => "yuv420p10le",
        (_, "444") => "yuv444p",
        (_, "422") => "yuv422p",
        _ => "yuv420p",
    }
    .into()
}

fn video_encoder(item: &QueueItem, preset: &Preset) -> String {
    if matches!(preset.hdr_mode.as_str(), "hlg" | "hdr10")
        || matches!(resolved_chroma(item, preset), "422" | "444")
        || (preset.codec == "h264" && resolved_bit_depth(item, preset) == 10)
    {
        return match preset.codec.as_str() {
            "h265" => "libx265",
            "h264" => "libx264",
            "av1" => "libaom-av1",
            "prores" => "prores_ks",
            _ => "libx264",
        }
        .into();
    }
    match (preset.codec.as_str(), preset.hardware.as_str()) {
        ("h265", "auto") if cfg!(target_os = "windows") && encoder_available("hevc_nvenc") => {
            "hevc_nvenc"
        }
        ("h264", "auto") if cfg!(target_os = "windows") && encoder_available("h264_nvenc") => {
            "h264_nvenc"
        }
        ("av1", "auto") if cfg!(target_os = "windows") && encoder_available("av1_nvenc") => {
            "av1_nvenc"
        }
        ("h265", "auto") if cfg!(target_os = "macos") && encoder_available("hevc_videotoolbox") => {
            "hevc_videotoolbox"
        }
        ("h264", "auto") if cfg!(target_os = "macos") && encoder_available("h264_videotoolbox") => {
            "h264_videotoolbox"
        }
        ("h265", "cuda") if cfg!(target_os = "windows") => "hevc_nvenc",
        ("h264", "cuda") if cfg!(target_os = "windows") => "h264_nvenc",
        ("av1", "cuda") if cfg!(target_os = "windows") => "av1_nvenc",
        ("h265", "metal") if cfg!(target_os = "macos") => "hevc_videotoolbox",
        ("h264", "metal") if cfg!(target_os = "macos") => "h264_videotoolbox",
        ("h265", _) => "libx265",
        ("h264", _) => "libx264",
        ("av1", _) => "libaom-av1",
        ("prores", _) => "prores_ks",
        _ => "libx264",
    }
    .to_string()
}

fn encoder_available(name: &str) -> bool {
    detected_encoders().iter().any(|encoder| encoder == name)
}

fn filter_available(name: &str) -> bool {
    static FILTERS: OnceLock<String> = OnceLock::new();
    FILTERS
        .get_or_init(|| {
            command_with_hidden_window(resolve_tool("ffmpeg"))
                .args(["-hide_banner", "-filters"])
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
                .unwrap_or_default()
        })
        .contains(name)
}

fn target_bitrate(item: &QueueItem, preset: &Preset) -> u64 {
    if preset.bitrate_mode == "source_multiplier" && item.bitrate > 0 {
        ((item.bitrate as f64) * preset.bitrate_multiplier).max(1.0) as u64
    } else {
        (preset.target_bitrate_mbps * 1_000_000.0).max(1.0) as u64
    }
}

fn preserve_times(source: &Path, output: &Path) -> Result<(), String> {
    let metadata = fs::metadata(source).map_err(|error| error.to_string())?;
    let modified = metadata.modified().map_err(|error| error.to_string())?;
    let created = metadata
        .created()
        .map_err(|error| format!("无法读取源文件创建日期：{error}"))?;
    set_file_mtime(output, FileTime::from_system_time(modified))
        .map_err(|error| error.to_string())?;
    #[cfg(windows)]
    {
        set_windows_creation_time(output, created)?;
    }
    #[cfg(target_os = "macos")]
    {
        set_macos_creation_time(output, created)?;
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = created;
        return Err("当前平台无法可靠写入文件创建日期".into());
    }

    verify_preserved_times(source, output)
}

fn verify_preserved_times(source: &Path, output: &Path) -> Result<(), String> {
    let source_metadata = fs::metadata(source).map_err(|error| error.to_string())?;
    let output_metadata = fs::metadata(output).map_err(|error| error.to_string())?;
    let source_modified = source_metadata
        .modified()
        .map_err(|error| error.to_string())?;
    let output_modified = output_metadata
        .modified()
        .map_err(|error| error.to_string())?;
    if !system_times_match(source_modified, output_modified) {
        return Err("输出文件修改时间与源视频不一致".into());
    }
    let source_created = source_metadata
        .created()
        .map_err(|error| format!("无法读取源文件创建日期：{error}"))?;
    let output_created = output_metadata
        .created()
        .map_err(|error| format!("无法读取输出文件创建日期：{error}"))?;
    if !system_times_match(source_created, output_created) {
        return Err("输出文件创建日期与源视频不一致".into());
    }
    Ok(())
}

fn system_times_match(left: SystemTime, right: SystemTime) -> bool {
    left.duration_since(right)
        .or_else(|_| right.duration_since(left))
        .map(|difference| difference.as_secs_f64() <= 2.0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn set_windows_creation_time(path: &Path, created: SystemTime) -> Result<(), String> {
    use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt, os::windows::io::AsRawHandle};
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::Storage::FileSystem::{SetFileTime, FILE_FLAG_BACKUP_SEMANTICS};

    let duration = created
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?;
    let intervals =
        (duration.as_secs() + 11_644_473_600) * 10_000_000 + (duration.subsec_nanos() as u64 / 100);
    let filetime = FILETIME {
        dwLowDateTime: intervals as u32,
        dwHighDateTime: (intervals >> 32) as u32,
    };
    let file = OpenOptions::new()
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .map_err(|error| error.to_string())?;
    let ok = unsafe {
        SetFileTime(
            file.as_raw_handle() as _,
            &filetime,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if ok == 0 {
        Err("无法设置 Windows 创建时间".into())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn set_macos_creation_time(path: &Path, created: SystemTime) -> Result<(), String> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let duration = created
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?;
    let mut attributes = libc::attrlist {
        bitmapcount: libc::ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: libc::ATTR_CMN_CRTIME,
        volattr: 0,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };
    let mut timestamp = libc::timespec {
        tv_sec: duration.as_secs() as libc::time_t,
        tv_nsec: duration.subsec_nanos() as libc::c_long,
    };
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|error| error.to_string())?;
    let result = unsafe {
        libc::setattrlist(
            path.as_ptr(),
            &mut attributes as *mut _ as *mut libc::c_void,
            &mut timestamp as *mut _ as *mut libc::c_void,
            std::mem::size_of::<libc::timespec>(),
            libc::FSOPT_NOFOLLOW,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(format!(
            "无法设置 macOS 创建日期：{}",
            std::io::Error::last_os_error()
        ))
    }
}

fn fps_label(rate: &str) -> String {
    let Some((num, den)) = rate.split_once('/') else {
        return rate.to_string();
    };
    let Ok(num) = num.parse::<f64>() else {
        return rate.to_string();
    };
    let Ok(den) = den.parse::<f64>() else {
        return rate.to_string();
    };
    if den == 0.0 {
        String::new()
    } else {
        format!("{:.2} fps", num / den)
    }
}

fn detect_panorama(json: &Value) -> bool {
    detect_panorama_tagged(json) || detect_panorama_shape(json)
}

fn detect_panorama_tagged(json: &Value) -> bool {
    let text = json.to_string().to_ascii_lowercase();
    [
        "spherical mapping",
        "equirectangular",
        "stereo_mode",
        "projection",
        "sv3d",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn detect_panorama_shape(json: &Value) -> bool {
    json["streams"]
        .as_array()
        .and_then(|streams| {
            streams
                .iter()
                .find(|stream| stream["codec_type"] == "video")
        })
        .and_then(|video| Some((video["width"].as_f64()?, video["height"].as_f64()?)))
        .map(|(width, height)| height > 0.0 && width / height >= 1.95)
        .unwrap_or(false)
}

#[derive(Clone, Copy)]
struct Mp4BoxInfo {
    start: usize,
    size: usize,
    header_size: usize,
}

impl Mp4BoxInfo {
    fn end(self) -> usize {
        self.start + self.size
    }
}

fn read_mp4_box(data: &[u8], start: usize, end: usize) -> Option<(Mp4BoxInfo, [u8; 4])> {
    if start.checked_add(8)? > end || end > data.len() {
        return None;
    }
    let size32 = u32::from_be_bytes(data[start..start + 4].try_into().ok()?);
    let name: [u8; 4] = data[start + 4..start + 8].try_into().ok()?;
    let (size, header_size) = if size32 == 1 {
        if start + 16 > end {
            return None;
        }
        (
            u64::from_be_bytes(data[start + 8..start + 16].try_into().ok()?) as usize,
            16,
        )
    } else if size32 == 0 {
        (end - start, 8)
    } else {
        (size32 as usize, 8)
    };
    if size < header_size || start.checked_add(size)? > end {
        return None;
    }
    Some((
        Mp4BoxInfo {
            start,
            size,
            header_size,
        },
        name,
    ))
}

fn child_boxes(data: &[u8], start: usize, end: usize) -> Vec<(Mp4BoxInfo, [u8; 4])> {
    let mut boxes = Vec::new();
    let mut cursor = start;
    while cursor + 8 <= end {
        let Some((info, name)) = read_mp4_box(data, cursor, end) else {
            break;
        };
        boxes.push((info, name));
        cursor = info.end();
    }
    boxes
}

fn named_child(
    data: &[u8],
    parent: Mp4BoxInfo,
    name: &[u8; 4],
    prefix: usize,
) -> Option<Mp4BoxInfo> {
    child_boxes(
        data,
        parent.start + parent.header_size + prefix,
        parent.end(),
    )
    .into_iter()
    .find_map(|(info, kind)| (kind == *name).then_some(info))
}

fn video_track_path(data: &[u8]) -> Result<Vec<Mp4BoxInfo>, String> {
    let moov = child_boxes(data, 0, data.len())
        .into_iter()
        .find_map(|(info, name)| (name == *b"moov").then_some(info))
        .ok_or_else(|| "输出文件缺少 moov 容器".to_string())?;
    let mdat = child_boxes(data, 0, data.len())
        .into_iter()
        .find_map(|(info, name)| (name == *b"mdat").then_some(info))
        .ok_or_else(|| "输出文件缺少 mdat 数据".to_string())?;
    if moov.start < mdat.start {
        return Err("全景元数据注入要求 moov 位于媒体数据之后".into());
    }
    for (trak, name) in child_boxes(data, moov.start + moov.header_size, moov.end()) {
        if name != *b"trak" {
            continue;
        }
        let Some(mdia) = named_child(data, trak, b"mdia", 0) else {
            continue;
        };
        let Some(hdlr) = named_child(data, mdia, b"hdlr", 0) else {
            continue;
        };
        let handler_pos = hdlr.start + hdlr.header_size + 8;
        if handler_pos + 4 > hdlr.end() || &data[handler_pos..handler_pos + 4] != b"vide" {
            continue;
        }
        let minf = named_child(data, mdia, b"minf", 0).ok_or("视频轨缺少 minf")?;
        let stbl = named_child(data, minf, b"stbl", 0).ok_or("视频轨缺少 stbl")?;
        let stsd = named_child(data, stbl, b"stsd", 0).ok_or("视频轨缺少 stsd")?;
        let sample_start = stsd.start + stsd.header_size + 8;
        let (sample, _) =
            read_mp4_box(data, sample_start, stsd.end()).ok_or("无法读取视频采样描述")?;
        return Ok(vec![moov, trak, mdia, minf, stbl, stsd, sample]);
    }
    Err("输出文件中没有可注入的主视频轨".into())
}

fn set_box_size(data: &mut [u8], info: Mp4BoxInfo, delta: usize) -> Result<(), String> {
    let new_size = info.size.checked_add(delta).ok_or("MP4 容器尺寸溢出")?;
    if info.header_size == 16 {
        data[info.start + 8..info.start + 16].copy_from_slice(&(new_size as u64).to_be_bytes());
    } else {
        let size = u32::try_from(new_size).map_err(|_| "MP4 容器超过 32 位尺寸限制")?;
        data[info.start..info.start + 4].copy_from_slice(&size.to_be_bytes());
    }
    Ok(())
}

fn make_box(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(payload.len() + 8);
    output.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
    output.extend_from_slice(name);
    output.extend_from_slice(payload);
    output
}

fn spherical_v2_boxes() -> Vec<u8> {
    let mut st3d_payload = vec![0, 0, 0, 0];
    st3d_payload.push(0);
    let st3d = make_box(b"st3d", &st3d_payload);

    let mut svhd_payload = vec![0, 0, 0, 0];
    svhd_payload.extend_from_slice(b"VideoSizeComposer\0");
    let svhd = make_box(b"svhd", &svhd_payload);
    let prhd = make_box(b"prhd", &[0; 16]);
    let equi = make_box(b"equi", &[0; 20]);
    let mut proj_payload = prhd;
    proj_payload.extend_from_slice(&equi);
    let proj = make_box(b"proj", &proj_payload);
    let mut sv3d_payload = svhd;
    sv3d_payload.extend_from_slice(&proj);
    let sv3d = make_box(b"sv3d", &sv3d_payload);

    [st3d, sv3d].concat()
}

fn spherical_v1_uuid_box() -> Vec<u8> {
    const UUID: [u8; 16] = [
        0xff, 0xcc, 0x82, 0x63, 0xf8, 0x55, 0x4a, 0x93, 0x88, 0x14, 0x58, 0x7a, 0x02, 0x52, 0x1f,
        0xdd,
    ];
    const XML: &str = r#"<?xml version="1.0"?><rdf:SphericalVideo xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:GSpherical="http://ns.google.com/videos/1.0/spherical/"><GSpherical:Spherical>true</GSpherical:Spherical><GSpherical:Stitched>true</GSpherical:Stitched><GSpherical:StitchingSoftware>VideoSizeComposer</GSpherical:StitchingSoftware><GSpherical:ProjectionType>equirectangular</GSpherical:ProjectionType></rdf:SphericalVideo>"#;
    let mut payload = UUID.to_vec();
    payload.extend_from_slice(XML.as_bytes());
    make_box(b"uuid", &payload)
}

fn inject_spherical_metadata(path: &Path) -> Result<(), String> {
    let mut data = fs::read(path).map_err(|error| error.to_string())?;
    let path_before = video_track_path(&data)?;
    let sample = *path_before.last().unwrap();
    let child_start = sample.start + sample.header_size + 78;
    let has_v2 = child_start <= sample.end()
        && child_boxes(&data, child_start, sample.end())
            .iter()
            .any(|(_, name)| name == b"sv3d");
    if !has_v2 {
        let boxes = spherical_v2_boxes();
        let delta = boxes.len();
        data.splice(sample.end()..sample.end(), boxes);
        for info in path_before.iter().rev() {
            set_box_size(&mut data, *info, delta)?;
        }
    }

    let path_after = video_track_path(&data)?;
    let moov = path_after[0];
    let trak = path_after[1];
    const UUID: [u8; 16] = [
        0xff, 0xcc, 0x82, 0x63, 0xf8, 0x55, 0x4a, 0x93, 0x88, 0x14, 0x58, 0x7a, 0x02, 0x52, 0x1f,
        0xdd,
    ];
    if !data[trak.start..trak.end()]
        .windows(UUID.len())
        .any(|window| window == UUID)
    {
        let uuid = spherical_v1_uuid_box();
        let delta = uuid.len();
        data.splice(trak.end()..trak.end(), uuid);
        set_box_size(&mut data, trak, delta)?;
        set_box_size(&mut data, moov, delta)?;
    }
    fs::write(path, data).map_err(|error| error.to_string())
}

fn verify_spherical_metadata(path: &Path) -> Result<(), String> {
    let output = command_with_hidden_window(resolve_tool("ffprobe"))
        .args(["-v", "error", "-show_streams", "-print_format", "json"])
        .arg(path)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err("无法复核全景输出".into());
    }
    let json: Value = serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    detect_panorama_tagged(&json)
        .then_some(())
        .ok_or_else(|| "全景元数据写入后未通过 ffprobe 复核".into())
}

fn detect_hdr_mode(video: &Value) -> String {
    let text = video.to_string().to_ascii_lowercase();
    if text.contains("dovi configuration record")
        || text.contains("dv_profile")
        || matches!(video["codec_tag_string"].as_str(), Some("dvhe" | "dvh1"))
    {
        "dolby_vision".into()
    } else {
        match video["color_transfer"].as_str().unwrap_or("") {
            "arib-std-b67" => "hlg".into(),
            "smpte2084" => "hdr10".into(),
            _ => "sdr".into(),
        }
    }
}

fn detect_bit_depth(video: &Value, pixel_format: &str) -> u32 {
    video["bits_per_raw_sample"]
        .as_str()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| {
            if pixel_format.contains("10") {
                10
            } else if pixel_format.contains("12") {
                12
            } else {
                8
            }
        })
}

fn detect_chroma(pixel_format: &str) -> String {
    if pixel_format.contains("444") {
        "444".into()
    } else if pixel_format.contains("422") {
        "422".into()
    } else {
        "420".into()
    }
}

fn parse_progress(line: &str, duration: f64) -> Option<u8> {
    if duration <= 0.0 {
        return None;
    }
    let time_index = line.find("time=")?;
    let value = line.get(time_index + 5..)?.split_whitespace().next()?;
    let mut parts = value.split(':');
    let hours = parts.next()?.parse::<f64>().ok()?;
    let minutes = parts.next()?.parse::<f64>().ok()?;
    let seconds = parts.next()?.parse::<f64>().ok()?;
    let current = hours * 3600.0 + minutes * 60.0 + seconds;
    Some(((current / duration) * 100.0).clamp(0.0, 99.0) as u8)
}

fn resolve_tool(name: &str) -> OsString {
    let exe_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let sibling = parent.join(&exe_name);
            if sibling.exists() {
                return sibling.into_os_string();
            }
            let internal = parent.join("_internal").join(&exe_name);
            if internal.exists() {
                return internal.into_os_string();
            }
        }
    }
    OsString::from(name)
}

fn command_exists(name: &str) -> bool {
    command_with_hidden_window(OsString::from(name))
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn command_with_hidden_window(program: OsString) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

fn shell_join(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if arg.contains(' ') {
                format!("\"{arg}\"")
            } else {
                arg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn preserve_times_restores_creation_and_modification_dates() {
        let root = std::env::temp_dir().join(format!("vsc-time-preservation-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source video.mp4");
        let output = root.join("encoded video.mp4");
        fs::write(&source, "source").unwrap();
        fs::write(&output, "output").unwrap();

        let source_created = UNIX_EPOCH + std::time::Duration::from_secs(1_650_000_000);
        let source_modified = UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        set_windows_creation_time(&source, source_created).unwrap();
        set_file_mtime(&source, FileTime::from_system_time(source_modified)).unwrap();

        preserve_times(&source, &output).unwrap();

        let source_metadata = fs::metadata(&source).unwrap();
        let output_metadata = fs::metadata(&output).unwrap();
        assert!(system_times_match(
            source_metadata.created().unwrap(),
            output_metadata.created().unwrap()
        ));
        assert!(system_times_match(
            source_metadata.modified().unwrap(),
            output_metadata.modified().unwrap()
        ));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn probe_and_encode_h264_h265_smoke() {
        let root = std::env::temp_dir().join(format!("vsc-smoke-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let sample = root.join("sample.mp4");
        let sample_status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args([
                "-y",
                "-hide_banner",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=24",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=880:sample_rate=48000",
                "-t",
                "1",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(&sample)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(sample_status.success(), "failed to create sample video");

        let original_modified = UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        set_file_mtime(&sample, FileTime::from_system_time(original_modified)).unwrap();
        #[cfg(windows)]
        {
            let original_created = UNIX_EPOCH + std::time::Duration::from_secs(1_650_000_000);
            set_windows_creation_time(&sample, original_created).unwrap();
        }

        let item = probe_video(&sample, "h264-source-30").unwrap();
        assert_eq!(item.width, 320);
        assert_eq!(item.height, 180);
        assert!(item.duration > 0.0);
        assert!(item.thumbnail.starts_with("data:image/jpeg;base64,"));

        for codec in ["h264", "h265"] {
            let mut preset = default_presets()
                .into_iter()
                .find(|preset| preset.codec == codec)
                .unwrap();
            preset.hardware = "cpu".into();
            preset.output_mode = "single_folder".into();
            preset.output_dir = root.join(codec).to_string_lossy().to_string();
            let output = build_output_path(&item, &preset);
            fs::create_dir_all(output.parent().unwrap()).unwrap();
            let args = build_ffmpeg_args(&item, &preset, &output);
            let status = command_with_hidden_window(resolve_tool("ffmpeg"))
                .args(args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "{codec} encode failed");
            assert!(output.exists(), "{codec} output missing");
            assert!(output.metadata().unwrap().len() > 0, "{codec} output empty");
            preserve_times(&sample, &output).unwrap();
            verify_preserved_times(&sample, &output).unwrap();
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn build_args_cover_reserved_features() {
        let item = QueueItem {
            id: "item".into(),
            source: "input.mp4".into(),
            file_name: "input.mp4".into(),
            codec: "H264".into(),
            width: 3840,
            height: 2160,
            fps: "24 fps".into(),
            bitrate: 40_000_000,
            duration: 10.0,
            size_bytes: 10,
            thumbnail: String::new(),
            is_panorama: true,
            panorama_tagged: true,
            bit_depth: 8,
            chroma: "420".into(),
            color_space: "bt709".into(),
            color_transfer: "bt709".into(),
            hdr_mode: "sdr".into(),
            audio_tracks: 1,
            subtitle_tracks: 0,
            preset_id: "preset".into(),
            selected: true,
            output: String::new(),
            status: "等待中".into(),
            progress: 0,
            media_kind: default_media_kind(),
            sequence_pattern: String::new(),
            sequence_start_number: 0,
            sequence_frame_count: 0,
            sequence_fps: default_sequence_fps(),
            sequence_pixel_aspect: default_pixel_aspect(),
            sequence_frames: Vec::new(),
        };

        let mut av1 = preset_with_defaults("av1", "AV1", "av1", "_av1");
        av1.resolution_mode = "short_edge".into();
        av1.short_edge = 1080;
        av1.bit_depth = "10".into();
        av1.chroma = "420".into();
        let av1_args = build_ffmpeg_args(&item, &av1, Path::new("out.mp4")).join(" ");
        assert!(av1_args.contains("-progress pipe:2 -nostats"));
        assert!(av1_args.contains("libaom-av1") || av1_args.contains("av1_nvenc"));
        assert!(av1_args.contains("-vf"));
        assert!(av1_args.contains("scale="));
        assert!(av1_args.contains("yuv420p10le"));
        assert!(av1_args.contains("projection=equirectangular"));

        let source_preset = preset_with_defaults("source", "跟随源视频", "h265", "_source");
        assert_eq!(source_preset.bit_depth, "source");
        assert_eq!(source_preset.chroma, "source");
        let mut legacy_json = serde_json::to_value(&source_preset).unwrap();
        legacy_json["bitDepth"] = Value::from(10);
        let migrated: Preset = serde_json::from_value(legacy_json).unwrap();
        assert_eq!(migrated.bit_depth, "10");
        assert_eq!(pixel_format(&item, &source_preset), "yuv420p");
        let mut ten_bit_422_item = item.clone();
        ten_bit_422_item.bit_depth = 10;
        ten_bit_422_item.chroma = "422".into();
        assert_eq!(
            pixel_format(&ten_bit_422_item, &source_preset),
            "yuv422p10le"
        );
        assert_eq!(video_encoder(&ten_bit_422_item, &source_preset), "libx265");

        let mut hlg = av1.clone();
        hlg.color_space = "rec2020".into();
        hlg.hdr_mode = "hlg".into();
        assert!(validate_preset(&hlg).is_ok());
        let hlg_args = build_ffmpeg_args(&item, &hlg, Path::new("hlg.mp4")).join(" ");
        assert!(hlg_args.contains("arib-std-b67"));
        assert!(hlg_args.contains("zscale="));

        let mut prores = preset_with_defaults("prores", "ProRes", "prores", "_prores");
        prores.chroma = "422".into();
        let output = build_output_path(&item, &prores);
        let prores_args = build_ffmpeg_args(&item, &prores, &output).join(" ");
        assert!(output.ends_with("input_prores.mov"));
        assert!(prores_args.contains("prores_ks"));
        assert!(prores_args.contains("-profile:v 1"));
        assert!(prores_args.contains("yuv422p10le"));

        let lut_root = std::env::temp_dir().join(format!("vsc-arg-lut-{}", Uuid::new_v4()));
        fs::create_dir_all(&lut_root).unwrap();
        let lut_path = lut_root.join("Cine Grade, Soft.cube");
        fs::write(&lut_path, "TITLE \"Look\"").unwrap();
        let mut lut = preset_with_defaults("lut", "LUT", "h264", "_lut");
        lut.lut_enabled = true;
        lut.lut_name = lut_path.to_string_lossy().to_string();
        lut.lut_intensity = 35;
        let lut_args = video_filter(&item, &lut).unwrap();
        assert!(lut_args.contains("split=2"));
        assert!(lut_args.contains("blend=all_expr"));
        assert!(lut_args.contains("Cine Grade\\, Soft.cube"));

        let mut dovi_item = item.clone();
        dovi_item.hdr_mode = "dolby_vision".into();
        let mut dovi = preset_with_defaults("dovi", "Dolby Vision 保留", "h265", "_dovi");
        dovi.hdr_mode = "dolby_vision".into();
        dovi.resolution_mode = "source".into();
        let dovi_job = EncodeJob {
            item: dovi_item.clone(),
            preset: dovi.clone(),
        };
        assert!(validate_job(&dovi_job).is_ok());
        let dovi_args = build_ffmpeg_args(&dovi_item, &dovi, Path::new("dovi.mp4")).join(" ");
        assert!(dovi_args.contains("-c:v copy"));
        assert!(dovi_args.contains("-tag:v hvc1"));
        assert!(!dovi_args.contains("-vf"));
        let mut non_dovi_item = dovi_item;
        non_dovi_item.hdr_mode = "hdr10".into();
        assert!(validate_job(&EncodeJob {
            item: non_dovi_item,
            preset: dovi
        })
        .is_err());
        let _ = fs::remove_dir_all(lut_root);
    }

    #[test]
    fn encode_hlg_hdr10_and_h265_422_outputs() {
        let root = std::env::temp_dir().join(format!("vsc-hdr-matrix-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let sample = root.join("bt709-source.mp4");
        let sample_status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args([
                "-y",
                "-hide_banner",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=128x72:rate=24",
                "-t",
                "0.4",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-color_primaries",
                "bt709",
                "-color_trc",
                "bt709",
                "-colorspace",
                "bt709",
            ])
            .arg(&sample)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(
            sample_status.success(),
            "failed to create HDR matrix source"
        );
        let item = probe_video(&sample, "hdr-matrix").unwrap();

        for (mode, expected_transfer) in [("hlg", "arib-std-b67"), ("hdr10", "smpte2084")] {
            let mut preset = preset_with_defaults(mode, mode, "h265", &format!("_{mode}"));
            preset.hardware = "cpu".into();
            preset.output_mode = "single_folder".into();
            preset.output_dir = root.join(mode).to_string_lossy().to_string();
            preset.bitrate_mode = "target_mbps".into();
            preset.target_bitrate_mbps = 1.0;
            preset.bit_depth = "10".into();
            preset.chroma = "420".into();
            preset.color_space = "rec2020".into();
            preset.hdr_mode = mode.into();
            fs::create_dir_all(&preset.output_dir).unwrap();
            let output = build_output_path(&item, &preset);
            let encode_output = command_with_hidden_window(resolve_tool("ffmpeg"))
                .args(build_ffmpeg_args(&item, &preset, &output))
                .stdout(Stdio::null())
                .output()
                .unwrap();
            assert!(
                encode_output.status.success(),
                "{mode} encode failed: {}",
                String::from_utf8_lossy(&encode_output.stderr)
            );
            let stream = probe_output_stream(&output);
            assert_eq!(stream["codec_name"], "hevc");
            assert_eq!(stream["pix_fmt"], "yuv420p10le");
            assert_eq!(stream["color_transfer"], expected_transfer);
            assert_eq!(stream["color_primaries"], "bt2020");
        }

        let mut h265_422 = preset_with_defaults("h265-422", "H.265 4:2:2", "h265", "_422");
        h265_422.hardware = "cpu".into();
        h265_422.output_mode = "single_folder".into();
        h265_422.output_dir = root.join("h265-422").to_string_lossy().to_string();
        h265_422.bit_depth = "10".into();
        h265_422.chroma = "422".into();
        fs::create_dir_all(&h265_422.output_dir).unwrap();
        let output_422 = build_output_path(&item, &h265_422);
        let status_422 = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args(build_ffmpeg_args(&item, &h265_422, &output_422))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status_422.success(), "H.265 4:2:2 encode failed");
        assert_eq!(probe_output_stream(&output_422)["pix_fmt"], "yuv422p10le");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lut_helpers_filter_and_deduplicate_paths() {
        let root = std::env::temp_dir().join(format!("vsc-lut-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let first = root.join("Look.cube");
        fs::write(&first, "TITLE \"Look\"").unwrap();

        assert!(is_lut(&first));
        assert!(!is_lut(&root.join("notes.txt")));
        assert_eq!(
            unique_child_path(&root, "Look.cube"),
            root.join("Look-1.cube")
        );
        assert_eq!(unique_child_path(&root, "New.cube"), root.join("New.cube"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn collect_video_files_recurses_filters_and_sorts() {
        let root = std::env::temp_dir().join(format!("vsc-import-{}", Uuid::new_v4()));
        let nested = root.join("nested");
        fs::create_dir_all(&nested).unwrap();
        let a = root.join("A.MP4");
        let b = nested.join("b.mov");
        let note = nested.join("note.txt");
        fs::write(&a, "").unwrap();
        fs::write(&b, "").unwrap();
        fs::write(&note, "").unwrap();

        let files = collect_video_files(vec![
            root.to_string_lossy().to_string(),
            a.to_string_lossy().to_string(),
        ]);
        assert_eq!(files.len(), 2);
        assert!(files.contains(&a));
        assert!(files.contains(&b));
        assert!(!files.contains(&note));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sequence_frame_selection_groups_suffix_names_and_encodes() {
        let root = std::env::temp_dir().join(format!("vsc-sequence-{}", Uuid::new_v4()));
        let output_dir = root.join("encoded");
        fs::create_dir_all(&root).unwrap();
        let pattern = root.join("shot_%04d_left.png");
        let create_status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args([
                "-y",
                "-hide_banner",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=64x36:rate=3",
                "-frames:v",
                "3",
                "-threads",
                "1",
            ])
            .arg(&pattern)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(create_status.success(), "failed to create sequence frames");

        let selected = root.join("shot_0002_left.png");
        let groups = collect_sequence_groups(vec![selected.to_string_lossy().to_string()]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 3);
        let item = probe_sequence(&groups[0], "sequence-preset", 24.0).unwrap();
        assert_eq!(item.media_kind, "sequence");
        assert_eq!(item.sequence_pattern, "shot_%04d_left.png");
        assert_eq!(item.sequence_frame_count, 3);
        assert_eq!(item.file_name, "shot_left [1–3].png");
        assert_eq!(item.fps, "24 fps");

        let mut preset = preset_with_defaults("sequence-preset", "Sequence", "h264", "_encoded");
        preset.hardware = "cpu".into();
        preset.output_mode = "single_folder".into();
        preset.output_dir = output_dir.to_string_lossy().to_string();
        fs::create_dir_all(&output_dir).unwrap();
        let output = build_output_path(&item, &preset);
        assert!(output.ends_with("shot_left_encoded.mp4"));
        let concat = write_sequence_concat_file(&item, &output).unwrap();
        let args = build_ffmpeg_args_with_sequence(&item, &preset, &output, Some(&concat));
        let encode_status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(encode_status.success(), "sequence encode failed");
        assert!(output.exists());
        assert!(output.metadata().unwrap().len() > 0);
        let frame_probe = command_with_hidden_window(resolve_tool("ffprobe"))
            .args([
                "-v",
                "error",
                "-count_frames",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=nb_read_frames",
                "-of",
                "default=nokey=1:noprint_wrappers=1",
            ])
            .arg(&output)
            .output()
            .unwrap();
        assert!(frame_probe.status.success());
        assert_eq!(String::from_utf8_lossy(&frame_probe.stdout).trim(), "3");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unique_output_path_avoids_existing_files() {
        let root = std::env::temp_dir().join(format!("vsc-output-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let base = root.join("clip_h265.mp4");
        let first = root.join("clip_h265-1.mp4");
        fs::write(&base, "base").unwrap();
        fs::write(&first, "first").unwrap();

        assert_eq!(unique_output_path(&base), root.join("clip_h265-2.mp4"));
        assert_eq!(
            unique_output_path(&root.join("new.mp4")),
            root.join("new.mp4")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn output_modes_naming_and_panorama_detection_are_predictable() {
        let root = std::env::temp_dir().join(format!("vsc-output-modes-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let mut item = QueueItem {
            id: "output-item".into(),
            source: root.join("源 视频.mp4").to_string_lossy().to_string(),
            file_name: "源 视频.mp4".into(),
            codec: "H264".into(),
            width: 3840,
            height: 1920,
            fps: "30 fps".into(),
            bitrate: 10_000_000,
            duration: 10.0,
            size_bytes: 10,
            thumbnail: String::new(),
            is_panorama: true,
            panorama_tagged: false,
            bit_depth: 8,
            chroma: "420".into(),
            color_space: "bt709".into(),
            color_transfer: "bt709".into(),
            hdr_mode: "sdr".into(),
            audio_tracks: 1,
            subtitle_tracks: 0,
            preset_id: "preset".into(),
            selected: true,
            output: String::new(),
            status: "就绪".into(),
            progress: 0,
            media_kind: default_media_kind(),
            sequence_pattern: String::new(),
            sequence_start_number: 0,
            sequence_frame_count: 0,
            sequence_fps: default_sequence_fps(),
            sequence_pixel_aspect: default_pixel_aspect(),
            sequence_frames: Vec::new(),
        };
        let mut preset = preset_with_defaults("preset", "Preset", "h265", "_压缩");
        preset.prefix = "交付_".into();
        assert_eq!(
            build_output_path(&item, &preset),
            root.join("VideoSizeComposer").join("交付_源 视频_压缩.mp4")
        );
        preset.output_mode = "in_place".into();
        preset.naming_mode = "original".into();
        assert_eq!(build_output_path(&item, &preset), root.join("源 视频.mp4"));
        preset.output_mode = "single_folder".into();
        preset.output_dir = root.join("统一输出").to_string_lossy().to_string();
        assert_eq!(
            build_output_path(&item, &preset),
            root.join("统一输出").join("源 视频.mp4")
        );

        let panorama_json =
            serde_json::json!({"streams":[{"codec_type":"video","width":4096,"height":2048}]});
        assert!(detect_panorama(&panorama_json));
        let dovi_json = serde_json::json!({"codec_tag_string":"dvhe","side_data_list":[{"side_data_type":"DOVI configuration record","dv_profile":8}]});
        assert_eq!(detect_hdr_mode(&dovi_json), "dolby_vision");
        item.is_panorama = false;
        assert!(!item.is_panorama);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn spherical_metadata_is_injected_and_verified_on_real_mp4() {
        let root = std::env::temp_dir().join(format!("vsc-panorama-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let output = root.join("panorama.mp4");
        let status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args([
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=640x320:rate=24",
                "-t",
                "0.25",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+use_metadata_tags",
            ])
            .arg(&output)
            .status()
            .unwrap();
        assert!(status.success());

        inject_spherical_metadata(&output).unwrap();
        verify_spherical_metadata(&output).unwrap();
        let probe = command_with_hidden_window(resolve_tool("ffprobe"))
            .args(["-v", "error", "-show_streams", "-print_format", "json"])
            .arg(&output)
            .output()
            .unwrap();
        let json: Value = serde_json::from_slice(&probe.stdout).unwrap();
        assert!(detect_panorama_tagged(&json));
        assert!(json
            .to_string()
            .to_ascii_lowercase()
            .contains("equirectangular"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn temporary_output_is_a_same_folder_partial_with_the_final_extension() {
        let final_output = Path::new("D:/Output/clip compressed.mp4");
        let temporary = temporary_output_path(final_output);
        assert_eq!(temporary.parent(), final_output.parent());
        assert_eq!(temporary.extension(), final_output.extension());
        let name = temporary.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with(".clip compressed.vsc-part-"));
        assert!(name.ends_with(".mp4"));
    }

    #[test]
    fn cancel_encode_marks_only_the_requested_session() {
        let requested_id = format!("cancel-test-{}", Uuid::new_v4());
        let other_id = format!("cancel-test-{}", Uuid::new_v4());
        let requested = Arc::new(AtomicBool::new(false));
        let other = Arc::new(AtomicBool::new(false));
        {
            let mut sessions = encode_sessions().lock().unwrap();
            sessions.insert(requested_id.clone(), requested.clone());
            sessions.insert(other_id.clone(), other.clone());
        }

        cancel_encode(requested_id.clone()).unwrap();

        assert!(requested.load(Ordering::Relaxed));
        assert!(!other.load(Ordering::Relaxed));
        let mut sessions = encode_sessions().lock().unwrap();
        sessions.remove(&requested_id);
        sessions.remove(&other_id);
    }

    #[test]
    fn probe_paths_reports_when_no_supported_video_exists() {
        let root = std::env::temp_dir().join(format!("vsc-empty-import-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("notes.txt"), "not a video").unwrap();

        let error = probe_paths_impl(vec![root.to_string_lossy().to_string()], "preset".into())
            .unwrap_err();
        assert!(error.contains("没有找到支持的视频文件"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preflight_checks_source_output_and_cleans_write_probe() {
        let root = std::env::temp_dir().join(format!("vsc-preflight-{}", Uuid::new_v4()));
        let source = root.join("source.mp4");
        let output_dir = root.join("输出 folder");
        fs::create_dir_all(&root).unwrap();
        fs::write(&source, "video placeholder").unwrap();
        let mut preset = preset_with_defaults("preflight", "Preflight", "h264", "_out");
        preset.output_mode = "single_folder".into();
        preset.output_dir = output_dir.to_string_lossy().to_string();
        let item = QueueItem {
            id: "preflight-item".into(),
            source: source.to_string_lossy().to_string(),
            file_name: "source.mp4".into(),
            codec: "H264".into(),
            width: 320,
            height: 180,
            fps: "24 fps".into(),
            bitrate: 1_000_000,
            duration: 1.0,
            size_bytes: 1024,
            thumbnail: String::new(),
            is_panorama: false,
            panorama_tagged: false,
            bit_depth: 8,
            chroma: "420".into(),
            color_space: "bt709".into(),
            color_transfer: "bt709".into(),
            hdr_mode: "sdr".into(),
            audio_tracks: 0,
            subtitle_tracks: 0,
            preset_id: preset.id.clone(),
            selected: true,
            output: String::new(),
            status: "就绪".into(),
            progress: 0,
            media_kind: default_media_kind(),
            sequence_pattern: String::new(),
            sequence_start_number: 0,
            sequence_frame_count: 0,
            sequence_fps: default_sequence_fps(),
            sequence_pixel_aspect: default_pixel_aspect(),
            sequence_frames: Vec::new(),
        };
        preflight_jobs(&[EncodeJob { item, preset }]).unwrap();
        assert!(output_dir.is_dir());
        assert!(fs::read_dir(&output_dir).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".vsc-write-test-")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn encode_with_lut_strength_and_preserve_modified_time() {
        let root = std::env::temp_dir().join(format!("vsc-lut-encode-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let sample = root.join("sample source.mp4");
        let lut = root.join("Cine Grade Soft.cube");
        fs::write(
            &lut,
            "TITLE \"Identity\"\nLUT_3D_SIZE 2\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 1 1 1\n0 0 0\n0 0 1\n0 1 0\n0 1 1\n1 0 0\n1 0 1\n1 1 0\n1 1 1\n",
        )
        .unwrap();

        let sample_status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args([
                "-y",
                "-hide_banner",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=160x90:rate=24",
                "-t",
                "0.5",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&sample)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(sample_status.success(), "failed to create sample video");

        let source_time = FileTime::from_unix_time(1_700_000_000, 0);
        set_file_mtime(&sample, source_time).unwrap();

        let item = probe_video(&sample, "h264-source-30").unwrap();
        let mut preset = preset_with_defaults("h264-lut", "H.264 LUT", "h264", "_lut");
        preset.hardware = "cpu".into();
        preset.output_mode = "single_folder".into();
        preset.output_dir = root.join("encoded").to_string_lossy().to_string();
        preset.lut_enabled = true;
        preset.lut_name = lut.to_string_lossy().to_string();
        preset.lut_intensity = 50;
        fs::create_dir_all(&preset.output_dir).unwrap();
        let output = build_output_path(&item, &preset);
        let args = build_ffmpeg_args(&item, &preset, &output);
        assert!(args.iter().any(|arg| arg.contains("blend=all_expr")));

        let status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "lut encode failed");
        preserve_times(&sample, &output).unwrap();
        assert_eq!(
            FileTime::from_last_modification_time(&output.metadata().unwrap()),
            source_time
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn encode_av1_10bit_and_prores_422_outputs() {
        let root = std::env::temp_dir().join(format!("vsc-matrix-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let sample = root.join("matrix-source.mp4");
        let sample_status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args([
                "-y",
                "-hide_banner",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=128x72:rate=24",
                "-t",
                "0.4",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&sample)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(sample_status.success(), "failed to create matrix source");

        let item = probe_video(&sample, "av1-1080-10bit").unwrap();
        let mut av1 = preset_with_defaults("av1", "AV1 10bit", "av1", "_av1");
        av1.hardware = "cpu".into();
        av1.output_mode = "single_folder".into();
        av1.output_dir = root.join("av1").to_string_lossy().to_string();
        av1.bitrate_mode = "target_mbps".into();
        av1.target_bitrate_mbps = 1.0;
        av1.bit_depth = "10".into();
        av1.chroma = "420".into();
        fs::create_dir_all(&av1.output_dir).unwrap();
        let av1_output = build_output_path(&item, &av1);
        let av1_status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args(build_ffmpeg_args(&item, &av1, &av1_output))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(av1_status.success(), "av1 encode failed");
        let av1_stream = probe_output_stream(&av1_output);
        assert_eq!(av1_stream["codec_name"], "av1");
        assert_eq!(av1_stream["pix_fmt"], "yuv420p10le");

        let mut av1_422 = av1.clone();
        av1_422.id = "av1-422".into();
        av1_422.suffix = "_av1_422".into();
        av1_422.output_dir = root.join("av1-422").to_string_lossy().to_string();
        av1_422.chroma = "422".into();
        fs::create_dir_all(&av1_422.output_dir).unwrap();
        let av1_422_output = build_output_path(&item, &av1_422);
        let av1_422_status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args(build_ffmpeg_args(&item, &av1_422, &av1_422_output))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(av1_422_status.success(), "av1 4:2:2 encode failed");
        assert_eq!(
            probe_output_stream(&av1_422_output)["pix_fmt"],
            "yuv422p10le"
        );

        let mut prores = preset_with_defaults("prores", "ProRes 422", "prores", "_prores");
        prores.output_mode = "single_folder".into();
        prores.output_dir = root.join("prores").to_string_lossy().to_string();
        prores.chroma = "422".into();
        prores.bit_depth = "10".into();
        fs::create_dir_all(&prores.output_dir).unwrap();
        let prores_output = build_output_path(&item, &prores);
        let prores_status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args(build_ffmpeg_args(&item, &prores, &prores_output))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(prores_status.success(), "prores encode failed");
        let prores_stream = probe_output_stream(&prores_output);
        assert_eq!(prores_stream["codec_name"], "prores");
        assert_eq!(prores_stream["pix_fmt"], "yuv422p10le");

        let _ = fs::remove_dir_all(root);
    }

    fn probe_output_stream(path: &Path) -> Value {
        let output = command_with_hidden_window(resolve_tool("ffprobe"))
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=codec_name,pix_fmt,color_transfer,color_space,color_primaries",
                "-of",
                "json",
            ])
            .arg(path)
            .output()
            .unwrap();
        assert!(output.status.success(), "ffprobe output stream failed");
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        json["streams"][0].clone()
    }
}
