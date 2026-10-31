//! INSV is camera footage, not an already stitched equirectangular video.
//! Use the official Desktop Media SDK CLI; never silently transcode a single lens.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(super) struct PanoramaPreferences {
    pub stabilization: String,
    pub stitching: String,
    pub stitch_fusion: bool,
    pub denoise: bool,
}

impl Default for PanoramaPreferences {
    fn default() -> Self {
        Self {
            stabilization: "flowstate".into(),
            stitching: "optflow".into(),
            stitch_fusion: true,
            denoise: false,
        }
    }
}

impl PanoramaPreferences {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(
            self.stabilization.as_str(),
            "off" | "flowstate" | "direction_lock"
        ) {
            return Err("无效的全景防抖模式".into());
        }
        if !matches!(
            self.stitching.as_str(),
            "optflow" | "dynamicstitch" | "aistitch"
        ) {
            return Err("无效的全景拼接算法".into());
        }
        Ok(())
    }
}

pub(super) fn is_insv(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("insv"))
}

fn other_lens(path: &Path) -> Option<PathBuf> {
    if !is_insv(path) {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    let mut parts: Vec<_> = stem.split('_').collect();
    // VID_<date>_<time>_<lens>_<clip>.insv; do not replace dates or clip indices.
    if parts.len() != 5 || !parts[0].eq_ignore_ascii_case("VID") {
        return None;
    }
    parts[3] = match parts[3] {
        "00" => "10",
        "10" => "00",
        _ => return None,
    };
    let name = format!("{}.{}", parts.join("_"), path.extension()?.to_str()?);
    let parent = path.parent()?;
    // Preserve the real directory entry spelling. On Windows, is_file() also
    // accepts a different extension case, which otherwise defeats dedup().
    fs::read_dir(parent)
        .ok()?
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(&name)
                && entry.path().is_file()
        })
        .map(|entry| entry.path())
}

pub(super) fn primary_path(path: &Path) -> PathBuf {
    let is_rear = path
        .file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.split('_').nth(3) == Some("10"));
    if is_rear {
        other_lens(path).unwrap_or_else(|| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

fn inputs(path: &Path) -> Vec<PathBuf> {
    let first = primary_path(path);
    let mut paths = vec![first.clone()];
    if let Some(second) = other_lens(&first) {
        paths.push(second);
    }
    paths
}

pub(super) fn describe_insv(item: &mut QueueItem, json: &Value) -> Result<(), String> {
    let videos: Vec<_> = json["streams"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["codec_type"] == "video")
        .collect();
    if videos.is_empty() {
        return Err("INSV 中没有可读视频轨".into());
    }
    let paths = inputs(Path::new(&item.source));
    let width = item.width;
    let height = item.height;
    if width == 0 || height == 0 {
        return Err("INSV 视频轨尺寸无效".into());
    }
    if videos.len() == 1 && width == height && paths.len() == 1 {
        return Err("该 INSV 只有一个方形镜头视频轨，缺少配对的 _00_ / _10_ 原片；请将两份文件保持原名放在同一目录".into());
    }
    // Two square lens streams -> a 2:1 panorama. Low-resolution recordings may
    // already store both fisheyes side by side in one 2:1 raster.
    let edge = width.max(height);
    let pano_width = if width == height {
        edge.checked_mul(2)
    } else {
        Some(edge)
    }
    .ok_or("INSV 尺寸溢出")?;
    item.width = (pano_width.max(4) / 4) * 4;
    item.height = item.width / 2;
    item.is_panorama = true;
    item.panorama_tagged = false;
    item.has_alpha = false;
    item.size_bytes = paths
        .iter()
        .filter_map(|p| p.metadata().ok())
        .map(|m| m.len())
        .sum();
    let track_bitrate: u64 = videos
        .iter()
        .filter_map(|s| s["bit_rate"].as_str()?.parse::<u64>().ok())
        .sum();
    item.bitrate = if paths.len() == 2 {
        item.bitrate.saturating_mul(2)
    } else {
        track_bitrate.max(item.bitrate)
    };
    item.codec = format!("INSV · {} · 待拼接", item.codec);
    Ok(())
}

pub(super) fn sdk_in_directory(path: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "MediaSDKTest.exe"
    } else {
        "MediaSDKTest"
    };
    if path.is_file() {
        return path
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case(name))
            .then(|| path.to_path_buf());
    }
    [path.join(name), path.join("bin").join(name)]
        .into_iter()
        .find(|p| p.is_file())
}

pub(super) fn resolve_sdk() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        return None;
    }
    let exe_dir = executable_dir_path().ok()?;
    let configured = read_preferences(&exe_dir.join("config/preferences.json"))
        .ok()
        .flatten()
        .map(|p| p.insta360_sdk_path)
        .unwrap_or_default();
    resolve_sdk_path(&configured)
}

pub(super) fn resolve_sdk_path(configured: &str) -> Option<PathBuf> {
    if cfg!(target_os = "macos") { return None; }
    let exe_dir = executable_dir_path().ok()?;
    if !configured.trim().is_empty() {
        // An explicitly configured but invalid directory must not use a different SDK.
        return sdk_in_directory(Path::new(configured.trim()));
    }
    if let Some(root) = std::env::var_os("INSTA360_MEDIA_SDK_DIR") {
        return sdk_in_directory(Path::new(&root));
    }
    sdk_in_directory(&exe_dir.join("insta360"))
        .or_else(|| super::sdk_setup::managed_sdk(&exe_dir))
        .or_else(|| sdk_in_directory(&exe_dir))
        .or_else(|| {
            std::env::var_os("PATH")
                .and_then(|paths| std::env::split_paths(&paths).find_map(|p| sdk_in_directory(&p)))
        })
}

fn missing_sdk() -> String {
    if cfg!(target_os = "macos") {
        return "官方 Desktop Media SDK 暂无 macOS 版本，当前平台不能直接拼接 INSV；请先用 Studio 导出全景 MP4".into();
    }
    "未找到 Insta360 Desktop Media SDK。请在应用设置中指定含 MediaSDKTest 的 SDK 目录，或将完整 bin（含 DLL 和 models）放到程序旁 insta360/。官方 SDK 需申请后下载：https://www.insta360.com/sdk/apply".into()
}

pub(super) fn validate_insv_job(item: &QueueItem, preset: &Preset) -> Result<(), String> {
    if matches!(output_extension(item, preset).as_str(), "m4a" | "wav" | "mp3") {
        return Ok(());
    }
    if preset.hdr_mode == "dolby_vision" {
        return Err("INSV 需要拼接，不能使用杜比视界源流保留模式".into());
    }
    let (w, h) = estimated_output_dimensions(item, preset);
    if w != h * 2 || w % 4 != 0 {
        return Err("INSV 全景输出必须为 2:1，宽为 4 的倍数；请调整尺寸或选择保持原尺寸".into());
    }
    let sdk = resolve_sdk().ok_or_else(missing_sdk)?;
    validate_sdk(&sdk, item, preset)
}

fn check_help(help: &str, item: &QueueItem, preset: &Preset) -> Result<(), String> {
    // Modern model-root handling is required: older demo versions silently ignore
    // unknown flags and some even default fusion to ON regardless of the flag.
    let mut required = vec![
        "-inputs",
        "-output",
        "-output_size",
        "-stitch_type",
        "-model_root_dir",
        "-enable_h265_encoder",
    ];
    if preset.panorama.stabilization != "off" {
        required.push("-enable_flowstate");
    }
    if preset.panorama.stabilization == "direction_lock" {
        required.push("-enable_directionlock");
    }
    if preset.panorama.stitch_fusion {
        required.push("-enable_stitchfusion");
    }
    if preset.panorama.denoise {
        required.push("-enable_denoise");
    }
    if item.bit_depth > 8 {
        required.push("-enable_10bit");
    }
    if preset.hardware == "cpu" || preset.cpu_fallback {
        required.extend([
            "-disable_cuda",
            "-enable_soft_encode",
            "-enable_soft_decode",
            "-image_processing_accel",
        ]);
    }
    if let Some(flag) = required.iter().find(|flag| !help.contains(**flag)) {
        return Err(format!("当前 MediaSDKTest 不支持 {flag}，请使用带 model_root_dir 参数的新版官方 SDK；未执行拼接"));
    }
    if !help.contains(&preset.panorama.stitching) {
        return Err("当前 SDK 未声明所选拼接算法，请更新 SDK 或选择其他算法".into());
    }
    Ok(())
}

fn validate_sdk(sdk: &Path, item: &QueueItem, preset: &Preset) -> Result<(), String> {
    let models = sdk.parent().unwrap_or(Path::new(".")).join("models");
    if !models.is_dir() {
        return Err(format!(
            "SDK 模型目录不存在：{}；请部署完整 SDK bin",
            models.display()
        ));
    }
    if preset.panorama.stitching == "aistitch" && !models.join("ai_stitcher.ins").is_file() {
        return Err("AI 拼接缺少 models/ai_stitcher.ins，请部署匹配版本的官方模型".into());
    }
    if preset.panorama.denoise && !models.join("jpg_denoise_9d006262.ins").is_file() {
        return Err("视频降噪缺少 SDK 配套模型 jpg_denoise_9d006262.ins".into());
    }
    // Bound this probe and drain both pipes, including SDK startup diagnostics.
    let log = std::env::temp_dir().join(format!("vsc-sdk-help-{}.log", Uuid::new_v4()));
    let _guard = TemporaryFileGuard(log.clone());
    let file = fs::File::create(&log).map_err(|e| e.to_string())?;
    let mut child = command_with_hidden_window(sdk.into())
        .arg("-help")
        .current_dir(sdk.parent().unwrap_or(Path::new(".")))
        .stdout(Stdio::from(file.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(file))
        .spawn()
        .map_err(|e| format!("无法启动 MediaSDKTest：{e}"))?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break, // Older help commands return non-zero even for valid help.
            Ok(None) if start.elapsed() < Duration::from_secs(15) => {
                thread::sleep(Duration::from_millis(50))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("MediaSDKTest 环境检测失败或超时；请检查 DLL、驱动与 SDK 版本".into());
            }
        }
    }
    check_help(&read_log_tail(&log), item, preset)
}

pub(super) fn intermediate_bytes(item: &QueueItem, preset: &Preset) -> u64 {
    if !is_insv(Path::new(&item.source))
        || matches!(output_extension(item, preset).as_str(), "m4a" | "wav" | "mp3")
    {
        return 0;
    }
    ((intermediate_bitrate(item) as f64 * item.duration.max(1.0) / 8.0) * 1.15) as u64
}

fn intermediate_bitrate(item: &QueueItem) -> u64 {
    item.bitrate.max(80_000_000)
}

fn stitch_args(item: &QueueItem, preset: &Preset, sdk: &Path, output: &Path) -> Vec<String> {
    let mut args = vec!["-inputs".into()];
    args.extend(
        inputs(Path::new(&item.source))
            .into_iter()
            .map(|p| p.to_string_lossy().to_string()),
    );
    args.extend([
        "-output".into(),
        output.to_string_lossy().to_string(),
        "-model_root_dir".into(),
        sdk.parent()
            .unwrap_or(Path::new("."))
            .join("models")
            .to_string_lossy()
            .to_string(),
        "-stitch_type".into(),
        preset.panorama.stitching.clone(),
        "-output_size".into(),
        format!("{}x{}", item.width, item.height),
        "-bitrate".into(),
        intermediate_bitrate(item).to_string(),
        "-enable_h265_encoder".into(),
    ]);
    if preset.panorama.stabilization != "off" {
        args.push("-enable_flowstate".into());
    }
    if preset.panorama.stabilization == "direction_lock" {
        args.push("-enable_directionlock".into());
    }
    if preset.panorama.stitch_fusion {
        args.push("-enable_stitchfusion".into());
    }
    if preset.panorama.denoise {
        args.push("-enable_denoise".into());
    }
    if item.bit_depth > 8 {
        args.push("-enable_10bit".into());
    }
    if preset.hardware == "cpu" {
        args.extend([
            "-disable_cuda".into(),
            "-enable_soft_encode".into(),
            "-enable_soft_decode".into(),
            "-image_processing_accel".into(),
            "cpu".into(),
        ]);
    }
    args
}

pub(super) struct PreparedInsv {
    pub item: QueueItem,
    _guard: TemporaryFileGuard,
}

fn read_log_tail(path: &Path) -> String {
    let Ok(mut file) = fs::File::open(path) else {
        return String::new();
    };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = file.seek(SeekFrom::Start(size.saturating_sub(32_768)));
    let mut bytes = Vec::new();
    let _ = file.take(32_768).read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).to_string()
}

fn sdk_progress(text: &str) -> Option<u8> {
    // Official demos print "process = 42%" with CR, not necessarily newlines.
    text.rsplit('%').skip(1).find_map(|before| {
        let digits: String = before
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits
            .chars()
            .rev()
            .collect::<String>()
            .parse::<u8>()
            .ok()
            .filter(|n| *n <= 100)
    })
}

fn run_stitch_command(
    mut command: Command,
    log: &Path,
    cancel: &Arc<AtomicBool>,
    mut progress: impl FnMut(u8),
) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err(CANCELLED_ERROR.into());
    }
    let file = fs::File::create(log).map_err(|e| e.to_string())?;
    let mut child = command
        .stdout(Stdio::from(file.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(file))
        .spawn()
        .map_err(|e| format!("无法启动 INSV 拼接：{e}"))?;
    let mut last_progress = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CANCELLED_ERROR.into());
        }
        let text = read_log_tail(log);
        if let Some(value) = sdk_progress(&text).filter(|p| *p > last_progress) {
            last_progress = value;
            progress(value);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let text = read_log_tail(log);
                // Some official demo versions return 0 even after a state error.
                if !status.success()
                    || text
                        .lines()
                        .any(|s| s.trim_start().to_lowercase().starts_with("error:"))
                {
                    return Err(format!("INSV 拼接失败：{status}\n{text}"));
                }
                return Ok(());
            }
            Ok(None) => thread::sleep(Duration::from_millis(150)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("无法等待 INSV 拼接：{error}"));
            }
        }
    }
}

pub(super) fn prepare_insv(
    app: &tauri::AppHandle,
    item: &QueueItem,
    preset: &Preset,
    output: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<PreparedInsv, String> {
    let sdk = resolve_sdk().ok_or_else(missing_sdk)?;
    let intermediate = output
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!(".vsc-insv-{}.mp4", Uuid::new_v4()));
    let guard = TemporaryFileGuard(intermediate.clone());
    let log = intermediate.with_extension("log");
    let _log_guard = TemporaryFileGuard(log.clone());
    let args = stitch_args(item, preset, &sdk, &intermediate);
    let emit = |progress: u8, message: Option<String>| {
        let _ = app.emit(
            "encode-progress",
            EncodeProgress {
                item_id: item.id.clone(),
                progress,
                status: format!("INSV 拼接与防抖 {progress}%"),
                output: Some(output.to_string_lossy().to_string()),
                ok: None,
                message,
                alpha_output: None,
            },
        );
    };
    emit(0, Some(format!("MediaSDKTest {}", shell_join(&args))));
    let mut command = command_with_hidden_window(sdk.clone().into_os_string());
    command
        .args(&args)
        .current_dir(sdk.parent().unwrap_or(Path::new(".")));
    if let Err(error) = run_stitch_command(command, &log, cancel, |p| {
        emit((u16::from(p) * 35 / 100) as u8, None)
    }) {
        if error == CANCELLED_ERROR || !preset.cpu_fallback || preset.hardware == "cpu" {
            return Err(error);
        }
        emit(0, Some(format!("SDK 拼接失败，尝试 CPU 路径：{error}")));
        let _ = fs::remove_file(&intermediate);
        let mut cpu = preset.clone();
        cpu.hardware = "cpu".into();
        let mut retry = command_with_hidden_window(sdk.clone().into_os_string());
        retry
            .args(stitch_args(item, &cpu, &sdk, &intermediate))
            .current_dir(sdk.parent().unwrap_or(Path::new(".")));
        run_stitch_command(retry, &log, cancel, |p| {
            emit((u16::from(p) * 35 / 100) as u8, None)
        })?;
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(CANCELLED_ERROR.into());
    }
    let mut stitched =
        probe_video(&intermediate, &item.preset_id).map_err(|e| format!("SDK 输出不可读：{e}"))?;
    validate_stitched(item, &stitched)?;
    stitched.id = item.id.clone();
    stitched.is_panorama = true;
    stitched.bitrate = item.bitrate; // Source-multiplier presets use the original recording bitrate.
    stitched.external_audio = item.external_audio.clone();
    emit(
        35,
        Some("INSV 拼接完成，开始按输出预设编码；拼接中间文件会自动清理".into()),
    );
    Ok(PreparedInsv {
        item: stitched,
        _guard: guard,
    })
}

fn validate_stitched(source: &QueueItem, stitched: &QueueItem) -> Result<(), String> {
    if stitched.width != source.width
        || stitched.height != source.height
        || stitched.width != 2 * stitched.height
    {
        return Err("SDK 输出尺寸不符合预期的 2:1 全景，未继续编码".into());
    }
    let fps = estimated_output_fps(source, &preset_with_defaults("", "", "h265", ""));
    if stitched.duration <= 0.0
        || (stitched.duration - source.duration).abs() > (2.0 / fps).max(0.1)
    {
        return Err("SDK 输出时长与 INSV 原片不匹配，可能拼接未完成；未继续编码".into());
    }
    if stitched.bit_depth < source.bit_depth || stitched.hdr_mode != source.hdr_mode {
        return Err(
            "SDK 输出未保留原片位深或 HDR 特性，未继续编码；请检查 SDK 对此拍摄模式的支持".into(),
        );
    }
    if source.audio_tracks > 0
        && source.external_audio.trim().is_empty()
        && stitched.audio_tracks == 0
    {
        return Err("SDK 输出丢失原片音轨，未继续编码".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insv_pair_import_deduplicates_and_legacy_presets_keep_defaults() {
        let root = std::env::temp_dir().join(format!("vsc-insv-pair-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let front = root.join("VID_20261024_120000_00_001.INSV");
        let rear = root.join("VID_20261024_120000_10_001.insv");
        fs::write(&front, b"front").unwrap();
        fs::write(&rear, b"rear").unwrap();
        assert_eq!(
            collect_media_files(vec![root.to_string_lossy().into()]),
            vec![front.clone()]
        );
        assert_eq!(primary_path(&rear), front);
        let mut json =
            serde_json::to_value(preset_with_defaults("old", "Old", "h265", "")).unwrap();
        json.as_object_mut().unwrap().remove("panorama");
        let loaded: Preset = serde_json::from_value(json).unwrap();
        assert_eq!(loaded.panorama.stabilization, "flowstate");
        assert!(loaded.panorama.stitch_fusion);
        let mut json = serde_json::to_value(&loaded).unwrap();
        json["panorama"] = serde_json::json!({"stabilization":"direction_lock", "stitching":"aistitch", "stitchFusion":false, "denoise":true});
        let loaded: Preset = serde_json::from_value(json).unwrap();
        validate_preset(&loaded).unwrap();
        let saved: Preset = serde_json::from_slice(&serde_json::to_vec(&loaded).unwrap()).unwrap();
        assert_eq!(saved.panorama.stitching, "aistitch");
        assert!(!saved.panorama.stitch_fusion);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn insv_real_media_probe_and_transcode_verify_panorama_and_reject_truncation() {
        let root = std::env::temp_dir().join(format!("vsc-insv-media-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("双镜头.insv");
        let status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=64x64:rate=10:duration=1",
                "-map",
                "0:v",
                "-map",
                "0:v",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-f",
                "mp4",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());
        let item = probe_video(&source, "insv").unwrap();
        assert_eq!((item.width, item.height), (128, 64));
        assert!(item.is_panorama);
        let mut preset = preset_with_defaults("insv", "INSV", "h264", "_encoded");
        preset.hardware = "cpu".into();
        preset.keep_times = false;
        assert_eq!(output_extension(&item, &preset), "mp4");
        preset.codec = "prores".into();
        assert_eq!(output_extension(&item, &preset), "mov");
        preset.codec = "h264".into();
        // Exercise the boundary using FFmpeg as a synthetic stitcher. This is
        // not a test of Insta360's proprietary image processing or calibration.
        let intermediate = root.join("stitched.mp4");
        let status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args(["-v", "error", "-i"])
            .arg(&source)
            .args([
                "-filter_complex",
                "[0:v:0][0:v:1]hstack=inputs=2[v]",
                "-map",
                "[v]",
                "-c:v",
                "libx264",
            ])
            .arg(&intermediate)
            .status()
            .unwrap();
        assert!(status.success());
        let mut stitched = probe_video(&intermediate, "insv").unwrap();
        validate_stitched(&item, &stitched).unwrap();
        let mut truncated = stitched.clone();
        truncated.duration = 0.2;
        assert!(validate_stitched(&item, &truncated).is_err());
        let mut downgraded = item.clone();
        downgraded.bit_depth = 10;
        assert!(validate_stitched(&downgraded, &stitched).is_err());
        let help = "-inputs -output -output_size -stitch_type optflow -enable_h265_encoder";
        assert!(check_help(help, &item, &preset)
            .unwrap_err()
            .contains("model_root_dir"));
        let args = stitch_args(
            &item,
            &preset,
            Path::new("C:/sdk/MediaSDKTest.exe"),
            &intermediate,
        );
        assert!(args.windows(2).any(|p| p == ["-output_size", "128x64"]));
        stitched.is_panorama = true;
        let output = root.join("final.mp4");
        let status = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args(build_ffmpeg_args(&stitched, &preset, &output))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        inject_spherical_metadata(&output).unwrap();
        verify_spherical_metadata(&output).unwrap();
        assert_eq!(probe_video(&output, "insv").unwrap().width, 128);
        // A failed/cancelled prepare must clean the intermediate via RAII.
        drop(TemporaryFileGuard(intermediate.clone()));
        assert!(!intermediate.exists());
        assert!(source.is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn insv_sdk_failure_and_silent_process_cancellation_are_terminal() {
        let log = std::env::temp_dir().join(format!("vsc-insv-process-{}.log", Uuid::new_v4()));
        let _guard = TemporaryFileGuard(log.clone());
        let cancel = Arc::new(AtomicBool::new(false));
        let mut failed = command_with_hidden_window("powershell.exe".into());
        failed.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Write-Output 'error: E_PARSE_METADATA'; exit 0",
        ]);
        assert!(run_stitch_command(failed, &log, &cancel, |_| {})
            .unwrap_err()
            .contains("E_PARSE_METADATA"));
        let mut silent = command_with_hidden_window("powershell.exe".into());
        silent.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Write-Output 'process = 1%'; Start-Sleep -Seconds 10",
        ]);
        let mut reported_at = None;
        assert_eq!(
            run_stitch_command(silent, &log, &cancel, |_| {
                reported_at = Some(std::time::Instant::now());
                cancel.store(true, Ordering::Relaxed);
            })
            .unwrap_err(),
            CANCELLED_ERROR
        );
        assert!(reported_at.unwrap().elapsed() < Duration::from_secs(2));
    }
}
