use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(super) struct MediaEdits {
    pub trim_start: f64,
    pub trim_end: f64,
    pub crop: Option<MediaCrop>,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub fit: String,
    pub lut_enabled: Option<bool>,
    pub lut_name: String,
    pub lut_intensity: u8,
}

impl Default for MediaEdits {
    fn default() -> Self {
        Self {
            trim_start: 0.0,
            trim_end: 0.0,
            crop: None,
            canvas_width: 0,
            canvas_height: 0,
            fit: "contain".into(),
            lut_enabled: None,
            lut_name: String::new(),
            lut_intensity: 80,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MediaCrop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub(super) fn effective_preset(item: &QueueItem, preset: &Preset) -> Preset {
    let mut preset = preset.clone();
    if let Some(edits) = &item.edits {
        if let Some(enabled) = edits.lut_enabled {
            preset.lut_enabled = enabled;
            preset.lut_name = edits.lut_name.clone();
            preset.lut_intensity = edits.lut_intensity;
        }
    }
    preset
}

pub(super) fn duration(item: &QueueItem) -> f64 {
    match &item.edits {
        Some(edits) => {
            let end = if edits.trim_end > 0.0 {
                edits.trim_end.min(item.duration)
            } else {
                item.duration
            };
            (end - edits.trim_start).max(0.0)
        }
        None => item.duration,
    }
}

pub(super) fn has_geometry(item: &QueueItem) -> bool {
    item.edits
        .as_ref()
        .is_some_and(|e| e.crop.is_some() || e.canvas_width > 0 || e.canvas_height > 0)
}

pub(super) fn panorama_locked(item: &QueueItem) -> bool {
    item.is_panorama || is_insv(Path::new(&item.source))
}

pub(super) fn preserve_panorama(item: &QueueItem, preset: &Preset) -> bool {
    preset.keep_panorama && item.is_panorama && !has_geometry(item)
}

pub(super) fn validate(item: &QueueItem, preset: &Preset) -> Result<(), String> {
    if panorama_locked(item) && has_geometry(item) {
        return Err("全景视频禁止裁切或更改画幅；请保留完整画面，可截取片段与使用 LUT".into());
    }
    let Some(edits) = &item.edits else {
        return Ok(());
    };
    if !edits.trim_start.is_finite()
        || !edits.trim_end.is_finite()
        || edits.trim_start < 0.0
        || edits.trim_end < 0.0
        || ((edits.trim_start > 0.0 || edits.trim_end > 0.0)
            && (!item.duration.is_finite()
                || item.duration <= 0.0
                || edits.trim_start >= item.duration))
        || (edits.trim_end > 0.0
            && (edits.trim_end <= edits.trim_start || edits.trim_end > item.duration + 0.001))
    {
        return Err("片段时间无效：开始时间需小于结束时间，且片段必须位于源媒体时长内".into());
    }
    if let Some(crop) = &edits.crop {
        if crop.width < 2
            || crop.height < 2
            || crop.width % 2 != 0
            || crop.height % 2 != 0
            || u64::from(crop.x) + u64::from(crop.width) > u64::from(item.width)
            || u64::from(crop.y) + u64::from(crop.height) > u64::from(item.height)
        {
            return Err("裁切区域无效：宽高需为至少 2 像素的偶数，且需位于源画面内".into());
        }
    }
    if (edits.canvas_width == 0) != (edits.canvas_height == 0)
        || (edits.canvas_width > 0
            && (edits.canvas_width < 2
                || edits.canvas_height < 2
                || edits.canvas_width > 16384
                || edits.canvas_height > 16384
                || edits.canvas_width % 2 != 0
                || edits.canvas_height % 2 != 0))
    {
        return Err("画幅宽高需同时填写，范围为 2–16384 的偶数像素".into());
    }
    if !matches!(edits.fit.as_str(), "contain" | "cover") || edits.lut_intensity > 100 {
        return Err("画幅适配方式或 LUT 强度无效".into());
    }
    if preset.hdr_mode == "dolby_vision"
        && (has_geometry(item) || edits.trim_start > 0.0 || edits.trim_end > 0.0)
    {
        return Err("片段、裁切和画幅编辑需要重新编码，请改用普通 H.265 预设；杜比视界保留模式会复制原视频流".into());
    }
    Ok(())
}

pub(super) fn dimensions(item: &QueueItem) -> (u64, u64) {
    let Some(edits) = &item.edits else {
        return (u64::from(item.width.max(2)), u64::from(item.height.max(2)));
    };
    if edits.canvas_width > 0 && edits.canvas_height > 0 {
        return (
            u64::from(edits.canvas_width),
            u64::from(edits.canvas_height),
        );
    }
    edits
        .crop
        .as_ref()
        .map(|c| (u64::from(c.width), u64::from(c.height)))
        .unwrap_or((u64::from(item.width.max(2)), u64::from(item.height.max(2))))
}

pub(super) fn append_geometry(filters: &mut Vec<String>, item: &QueueItem) {
    if panorama_locked(item) {
        return;
    }
    let Some(edits) = &item.edits else { return };
    if let Some(crop) = &edits.crop {
        // exact keeps the source-pixel selection even for subsampled input.
        filters.push(format!(
            "crop={}:{}:{}:{}:exact=1",
            crop.width, crop.height, crop.x, crop.y
        ));
    }
    if edits.canvas_width > 0 && edits.canvas_height > 0 {
        let (w, h) = (edits.canvas_width, edits.canvas_height);
        if edits.fit == "cover" {
            filters.push(format!("scale={w}:{h}:force_original_aspect_ratio=increase:force_divisible_by=2,crop={w}:{h},setsar=1"));
        } else {
            filters.push(format!("scale={w}:{h}:force_original_aspect_ratio=decrease:force_divisible_by=2,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,setsar=1"));
        }
    }
}

pub(super) fn append_trim(args: &mut Vec<String>, item: &QueueItem) {
    if let Some(edits) = &item.edits {
        if edits.trim_start > 0.0 {
            args.extend(["-ss".into(), format!("{:.6}", edits.trim_start)]);
        }
        if edits.trim_start > 0.0 || edits.trim_end > 0.0 {
            args.extend(["-t".into(), format!("{:.6}", duration(item).max(0.001))]);
        }
    }
}

#[tauri::command]
pub(super) async fn preview_media_edit(
    item: QueueItem,
    preset: Preset,
    at_seconds: f64,
    compare_original: bool,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        render_preview(item, preset, at_seconds, compare_original)
    })
    .await
    .map_err(|e| format!("预览任务失败：{e}"))?
}

fn render_preview(
    mut item: QueueItem,
    preset: Preset,
    at_seconds: f64,
    compare_original: bool,
) -> Result<String, String> {
    if item.media_kind == "audio" {
        return Err("音频没有可预览的源画面，可编辑截取时间后导出".into());
    }
    if is_insv(Path::new(&item.source)) {
        return Err("INSV 需先使用官方 SDK 或 Studio 拼接为全景视频后预览编辑".into());
    }
    if !at_seconds.is_finite() || at_seconds < 0.0 || at_seconds >= item.duration {
        return Err("预览时间需位于源媒体时长内".into());
    }
    let mut preset = effective_preset(&item, &preset);
    if compare_original {
        item.edits = None;
        preset.resolution_mode = "source".into();
        preset.lut_enabled = false;
        preset.hdr_mode = "source".into();
        preset.color_space = "source".into();
    } else {
        validate(&item, &preset)?;
        if preset.lut_enabled {
            let lut = resolve_stored_lut_path(&executable_dir_path()?, &preset.lut_name);
            if !lut.is_file() {
                return Err(format!("LUT 文件不存在：{}", preset.lut_name));
            }
        }
    }
    // Preview decoding does not use the selected output encoder or container.
    let mut args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-threads", "2"]
        .iter()
        .map(|s| (*s).into())
        .collect();
    if item.media_kind == "sequence" {
        hydrate_sequence_frames(&mut item)?;
        let frame = (at_seconds * item.sequence_fps).floor() as usize;
        let path = item
            .sequence_frames
            .get(frame.min(item.sequence_frames.len().saturating_sub(1)))
            .ok_or("序列没有可预览的帧")?;
        args.extend(["-i".into(), path.clone()]);
    } else {
        args.extend([
            "-ss".into(),
            format!("{at_seconds:.6}"),
            "-i".into(),
            item.source.clone(),
        ]);
    }
    let mut filters = Vec::new();
    if let Some(filter) = video_filter(&item, &preset) {
        filters.push(filter);
    }
    // Keep LUT and geometry identical to export, then map HDR output for an SDR display.
    let (primaries, transfer, matrix) = output_color(&item, &preset);
    if matches!(transfer, "arib-std-b67" | "smpte2084") {
        let mut output_item = item.clone();
        output_item.source_color_override.clear();
        output_item.hdr_mode = if transfer == "arib-std-b67" {
            "hlg"
        } else {
            "hdr10"
        }
        .into();
        output_item.hdr_signal = output_item.hdr_mode.clone();
        output_item.color_space = primaries.into();
        output_item.color_transfer = transfer.into();
        output_item.color_matrix = matrix.into();
        output_item.color_range = if color_conversion_filter(&item, &preset).is_some() {
            "tv".into()
        } else {
            source_color(&item).range.into()
        };
        let mut sdr = preset.clone();
        sdr.hdr_mode = "sdr".into();
        sdr.color_space = "rec709".into();
        sdr.bit_depth = "8".into();
        sdr.chroma = "420".into();
        if let Some(filter) = color_conversion_filter(&output_item, &sdr) {
            filters.push(filter);
        }
    }
    filters.push("scale=w='min(960,iw)':h='min(540,ih)':force_original_aspect_ratio=decrease:force_divisible_by=2,setsar=1,format=yuvj420p".into());
    args.extend([
        "-map".into(),
        "0:v:0".into(),
        "-an".into(),
        "-frames:v".into(),
        "1".into(),
        "-vf".into(),
        filters.join(","),
        "-filter_threads".into(),
        "1".into(),
        "-threads".into(),
        "2".into(),
        "-f".into(),
        "image2pipe".into(),
        "-c:v".into(),
        "mjpeg".into(),
        "pipe:1".into(),
    ]);
    let result = command_with_hidden_window(resolve_tool("ffmpeg"))
        .args(args)
        .output()
        .map_err(|e| format!("无法启动预览：{e}"))?;
    if !result.status.success() || result.stdout.is_empty() {
        return Err(format!(
            "预览失败：{}",
            String::from_utf8_lossy(&result.stderr)
                .chars()
                .take(1200)
                .collect::<String>()
        ));
    }
    Ok(format!(
        "data:image/jpeg;base64,{}",
        BASE64_STANDARD.encode(result.stdout)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_edit_validation_and_export_arguments() {
        assert_eq!(
            display_dimensions(
                &serde_json::json!({"width":1920,"height":1080,"side_data_list":[{"rotation":-90}]})
            ),
            (1080, 1920)
        );
        assert_eq!(
            display_dimensions(
                &serde_json::json!({"width":1920,"height":1080,"tags":{"rotate":"270"}})
            ),
            (1080, 1920)
        );
        let mut item = QueueItem {
            width: 640,
            height: 360,
            duration: 10.0,
            audio_tracks: 1,
            source: "source.mp4".into(),
            edits: Some(MediaEdits {
                trim_start: 2.0,
                trim_end: 5.0,
                crop: Some(MediaCrop {
                    x: 20,
                    y: 10,
                    width: 320,
                    height: 240,
                }),
                canvas_width: 180,
                canvas_height: 320,
                ..MediaEdits::default()
            }),
            ..QueueItem::default()
        };
        let mut preset = preset_with_defaults("edit", "Edit", "h264", "_edit");
        preset.hardware = "cpu".into();
        assert!(validate(&item, &preset).is_ok());
        let mut unknown_duration = item.clone();
        unknown_duration.duration = 0.0;
        unknown_duration.edits.as_mut().unwrap().trim_start = 0.0;
        unknown_duration.edits.as_mut().unwrap().trim_end = 0.0;
        assert!(validate(&unknown_duration, &preset).is_ok());
        unknown_duration.edits.as_mut().unwrap().trim_start = 1.0;
        assert!(validate(&unknown_duration, &preset).is_err());
        assert_eq!(duration(&item), 3.0);
        assert_eq!(estimated_output_dimensions(&item, &preset), (180, 320));
        let args = build_ffmpeg_args(&item, &preset, Path::new("edited.mp4"));
        assert!(args.windows(2).any(|p| p == ["-ss", "2.000000"]));
        assert!(args.windows(2).any(|p| p == ["-t", "3.000000"]));
        let filter = video_filter(&item, &preset).unwrap();
        assert!(filter.contains("crop=320:240:20:10:exact=1"));
        assert!(filter.contains("pad=180:320"));
        item.is_panorama = true;
        assert!(!preserve_panorama(&item, &preset));
        assert!(validate(&item, &preset)
            .unwrap_err()
            .contains("全景视频禁止"));
        item.is_panorama = false;
        preset.hdr_mode = "dolby_vision".into();
        assert!(validate(&item, &preset).unwrap_err().contains("重新编码"));
        preset.hdr_mode = "source".into();
        item.edits.as_mut().unwrap().trim_end = 11.0;
        assert!(validate(&item, &preset).is_err());
        item.edits.as_mut().unwrap().trim_end = 5.0;
        item.edits.as_mut().unwrap().crop.as_mut().unwrap().x = 500;
        assert!(validate(&item, &preset).is_err());
        item.edits.as_mut().unwrap().crop = None;
        item.edits.as_mut().unwrap().lut_enabled = Some(false);
        preset.lut_enabled = true;
        assert!(!effective_preset(&item, &preset).lut_enabled);
    }

    #[test]
    fn v12_05_panorama_crop_guard_and_real_lut_preview() {
        let dir = TestDirectory::new();
        let source = dir.0.join("panorama.mp4");
        encode(
            [
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x160:rate=4",
                "-t",
                "1",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ]
            .into_iter()
            .map(str::to_string)
            .chain([source.to_string_lossy().into_owned()])
            .collect(),
        );
        inject_spherical_metadata(&source).unwrap();
        let mut item = probe_video(&source, "pano").unwrap();
        assert!(item.is_panorama && item.panorama_tagged);
        let mut preset = preset_with_defaults("pano", "Panorama", "h264", "_pano");
        preset.hardware = "cpu".into();
        item.edits = Some(MediaEdits {
            crop: Some(MediaCrop {
                x: 16,
                y: 0,
                width: 288,
                height: 160,
            }),
            ..MediaEdits::default()
        });
        assert!(validate_job(&EncodeJob {
            item: item.clone(),
            preset: preset.clone()
        })
        .unwrap_err()
        .contains("全景视频禁止"));
        let mut filters = Vec::new();
        append_geometry(&mut filters, &item);
        assert!(
            filters.is_empty(),
            "defensive path must never crop panoramas"
        );
        item.edits.as_mut().unwrap().crop = None;
        item.edits.as_mut().unwrap().canvas_width = 160;
        item.edits.as_mut().unwrap().canvas_height = 320;
        assert!(validate(&item, &preset).is_err());
        item.is_panorama = false;
        let original_source = item.source.clone();
        item.source = "camera.insv".into();
        assert!(validate(&item, &preset)
            .unwrap_err()
            .contains("全景视频禁止"));
        item.source = original_source;
        item.is_panorama = true;
        let lut = dir.0.join("invert.cube");
        fs::write(
            &lut,
            "LUT_3D_SIZE 2\n1 1 1\n0 1 1\n1 0 1\n0 0 1\n1 1 0\n0 1 0\n1 0 0\n0 0 0\n",
        )
        .unwrap();
        item.edits = Some(MediaEdits {
            trim_start: 0.25,
            trim_end: 0.75,
            lut_enabled: Some(true),
            lut_name: lut.to_string_lossy().into_owned(),
            lut_intensity: 70,
            ..MediaEdits::default()
        });
        assert!(validate_job(&EncodeJob {
            item: item.clone(),
            preset: preset.clone()
        })
        .is_ok());
        assert!(preserve_panorama(&item, &preset));
        let original = render_preview(item.clone(), preset.clone(), 0.5, true).unwrap();
        let graded = render_preview(item.clone(), preset.clone(), 0.5, false).unwrap();
        assert_ne!(original, graded, "panorama LUT preview must remain active");
        assert!(graded.starts_with("data:image/jpeg;base64,"));
        assert!(!video_filter(&item, &preset).unwrap().contains("crop="));
        assert_eq!(dimensions(&item), (320, 160));
        assert_eq!(duration(&item), 0.5);
    }

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join(format!("vsc-media-edit-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn encode(args: Vec<String>) {
        let output = command_with_hidden_window(resolve_tool("ffmpeg"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn probe(path: &Path) -> Value {
        let out = command_with_hidden_window(resolve_tool("ffprobe"))
            .args([
                "-v",
                "error",
                "-show_format",
                "-show_streams",
                "-of",
                "json",
            ])
            .arg(path)
            .output()
            .unwrap();
        serde_json::from_slice(&out.stdout).unwrap()
    }

    #[test]
    fn media_edit_real_video_preview_and_mp3() {
        let dir = TestDirectory::new();
        let source = dir.0.join("source.mp4");
        encode(
            [
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=320x240:r=12",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=16000",
                "-t",
                "2",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ]
            .iter()
            .map(|s| (*s).into())
            .chain([source.to_string_lossy().to_string()])
            .collect(),
        );
        let mut item = probe_video(&source, "edit").unwrap();
        assert!(!item.panorama_tagged);
        item.edits = Some(MediaEdits {
            trim_start: 0.5,
            trim_end: 1.5,
            crop: Some(MediaCrop {
                x: 40,
                y: 20,
                width: 240,
                height: 180,
            }),
            canvas_width: 180,
            canvas_height: 320,
            ..MediaEdits::default()
        });
        let mut preset = preset_with_defaults("edit", "Edit", "h264", "_edit");
        preset.hardware = "cpu".into();
        preset.bitrate_mode = "target_mbps".into();
        preset.target_bitrate_mbps = 0.5;
        let output = dir.0.join("edited.mp4");
        encode(build_ffmpeg_args(&item, &preset, &output));
        let json = probe(&output);
        assert!(
            !detect_panorama_tagged(&json),
            "ordinary cropped output unexpectedly contains spherical metadata"
        );
        let stream = json["streams"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["codec_type"] == "video")
            .unwrap();
        assert_eq!(
            (stream["width"].as_u64(), stream["height"].as_u64()),
            (Some(180), Some(320))
        );
        let actual_duration: f64 = json["format"]["duration"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!((actual_duration - 1.0).abs() < 0.15);
        let original = render_preview(item.clone(), preset.clone(), 0.75, true).unwrap();
        let edited = render_preview(item.clone(), preset.clone(), 0.75, false).unwrap();
        assert!(BASE64_STANDARD
            .decode(edited.trim_start_matches("data:image/jpeg;base64,"))
            .unwrap()
            .starts_with(&[0xff, 0xd8]));
        assert_ne!(original, edited);
        let lut = dir.0.join("invert.cube");
        fs::write(
            &lut,
            "LUT_3D_SIZE 2\n1 1 1\n0 1 1\n1 0 1\n0 0 1\n1 1 0\n0 1 0\n1 0 0\n0 0 0\n",
        )
        .unwrap();
        let edits = item.edits.as_mut().unwrap();
        edits.lut_enabled = Some(true);
        edits.lut_name = lut.to_string_lossy().into();
        edits.lut_intensity = 50;
        let graded = render_preview(item.clone(), preset.clone(), 0.75, false).unwrap();
        assert_ne!(graded, edited);
        let graded_output = dir.0.join("graded.mp4");
        encode(build_ffmpeg_args(&item, &preset, &graded_output));

        preset.output_container = "mp3".into();
        for media_kind in ["video", "audio"] {
            item.media_kind = media_kind.into();
            let output = dir.0.join(format!("{media_kind}.mp3"));
            assert!(validate_job(&EncodeJob {
                item: item.clone(),
                preset: preset.clone()
            })
            .is_ok());
            encode(build_ffmpeg_args(&item, &preset, &output));
            let json = probe(&output);
            assert_eq!(json["streams"].as_array().unwrap().len(), 1);
            assert_eq!(json["streams"][0]["codec_name"], "mp3");
            assert_eq!(json["streams"][0]["sample_rate"], "48000");
            assert_eq!(json["streams"][0]["bit_rate"], "320000");
            let actual_duration: f64 = json["format"]["duration"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap();
            assert!((actual_duration - 1.0).abs() < 0.15);
        }
        item.audio_tracks = 0;
        assert!(validate_job(&EncodeJob { item, preset })
            .unwrap_err()
            .contains("没有音轨"));
    }

    #[test]
    fn media_edit_sequence_external_audio_and_alpha_share_trim() {
        let dir = TestDirectory::new();
        let pattern = dir.0.join("frame_%03d.png");
        encode(
            [
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=red@0.5:s=160x120:r=5,format=rgba",
                "-frames:v",
                "10",
                "-threads",
                "1",
            ]
            .iter()
            .map(|s| (*s).into())
            .chain([pattern.to_string_lossy().to_string()])
            .collect(),
        );
        let frames: Vec<_> = (1..=10)
            .map(|i| dir.0.join(format!("frame_{i:03}.png")))
            .collect();
        let mut item = probe_sequence(&frames, "edit", 5.0).unwrap();
        hydrate_sequence_frames(&mut item).unwrap();
        assert!(item.has_alpha);
        let audio = dir.0.join("audio.wav");
        encode(
            [
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000",
                "-t",
                "0.4",
            ]
            .iter()
            .map(|s| (*s).into())
            .chain([audio.to_string_lossy().to_string()])
            .collect(),
        );
        item.external_audio = audio.to_string_lossy().into();
        item.edits = Some(MediaEdits {
            trim_start: 0.6,
            trim_end: 1.6,
            crop: Some(MediaCrop {
                x: 20,
                y: 10,
                width: 120,
                height: 100,
            }),
            canvas_width: 100,
            canvas_height: 100,
            ..MediaEdits::default()
        });
        let mut preset = preset_with_defaults("edit", "Edit", "h264", "_edit");
        preset.hardware = "cpu".into();
        let output = dir.0.join("sequence.mp4");
        let list = write_sequence_concat_file(&item, &output).unwrap();
        encode(build_ffmpeg_args_with_sequence(
            &item,
            &preset,
            &output,
            Some(&list),
        ));
        let alpha = dir.0.join("alpha.mp4");
        encode(build_alpha_ffmpeg_args_with_sequence(
            &item,
            &preset,
            &alpha,
            Some(&list),
        ));
        for path in [&output, &alpha] {
            let json = probe(path);
            let stream = json["streams"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["codec_type"] == "video")
                .unwrap();
            assert_eq!(
                (stream["width"].as_u64(), stream["height"].as_u64()),
                (Some(100), Some(100))
            );
            let actual_duration: f64 = json["format"]["duration"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap();
            assert!(
                (actual_duration - 1.0).abs() < 0.21,
                "{path:?}: {actual_duration}"
            );
        }
        assert!(probe(&output)["streams"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["codec_type"] == "audio"));
        assert!(!probe(&alpha)["streams"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["codec_type"] == "audio"));
        let preview = render_preview(item, preset, 1.0, false).unwrap();
        assert!(preview.starts_with("data:image/jpeg;base64,"));
    }
}
