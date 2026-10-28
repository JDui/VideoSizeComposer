# HLG 读取 / 输出链路验证

日期：2026-10-29　对象：`src-tauri/src/lib.rs` 的色彩链（`color_conversion_filter` / `tone_map_strategy` / `detect_hdr_signal` / `probe_hdr_metadata` / `sample_coded_peak_ratio`）

## 运行方式

```powershell
powershell -ExecutionPolicy Bypass -File .\qa\hlg\verify-hlg.ps1
# 保留中间产物（work/ 目录）便于复查
powershell -ExecutionPolicy Bypass -File .\qa\hlg\verify-hlg.ps1 -Keep
```

脚本使用 `dist/VideoSizeComposer/ffmpeg.exe`、`ffprobe.exe`，与打包后应用调用的是同一套二进制；全部检查通过时退出码为 0。

单元测试侧：`cd src-tauri && cargo test --lib`，其中与 HLG 直接相关的用例为
`auto_tone_map_keeps_hlg_sdr_luma_close_to_the_sdr_master`、`hlg_to_sdr_keeps_display_referred_luma_for_sdr_dynamic_range`、
`hlg_to_sdr_compresses_highlights_only_for_true_hdr`、`tone_map_choice_follows_the_preset_override`、
`unlabelled_hlg_is_flagged_but_stays_sdr_until_confirmed`、`probe_flags_unlabelled_wide_gamut_clips_as_suspect_hlg`、
`hdr_static_metadata_is_read_from_frame_side_data`、`hdr10_tags_reuse_the_source_mastering_metadata`、
`colour_labels_follow_the_pixels_after_conversion`、`keeping_an_hdr_source_refuses_lossy_targets`、
`coded_peak_ratio_normalises_coded_range_and_depth`、`placeholder_colour_tags_fall_back_instead_of_claiming_a_gamut`。

## 环境

| 项 | 值 |
| --- | --- |
| FFmpeg | `N-120228-g3334bdc6e7-20250711`（含 `zimg`/`zscale`、`tonemap`、`lut3d`、libx264/x265/libaom-av1/prores_ks） |
| x265 | 4.1+168-1e993ee8（8/10/12-bit） |
| `tonemap` 可用算法 | `none`、`linear`、`gamma`、`clip`、`reinhard`、`hable`、`mobius`（**没有** `bt2390`，也**没有** `max_peak`/`target_peak`，只有 `peak`，默认 0） |
| 测试素材 | `testsrc2` 160×96、25 fps、0.2 s（`sdr_master` → 套 HLG OETF 得 `hlg_sdr_referred` → 亮度整体推到码值顶端得 `hlg_true_peak`） |

## 素材与编码域峰值

`coded_peak_ratio = (YMAX - black) / (white - black)`，10-bit/tv 取 64–940，与 `lib.rs` 的实现一致；抽样用 `-skip_frame nokey` + `signalstats,metadata=print`。

| 素材 | 含义 | YMAX（10-bit） | 编码域峰值 | 自动档选择 |
| --- | --- | --- | --- | --- |
| `hlg_sdr_referred.mp4` | 手机/相机直出的“HLG”：SDR 亮度套 HLG OETF，白点约在码值 0.6 | 603 | **0.62** | BT.2408 直映射 |
| `hlg_true_peak.mp4` | 广电式 HLG：场景参考信号真的打到满量程 | 961 | **1.02** | 软膝压缩高光 |

两档间距 0.40，阈值 0.80 落在空隙正中，判定不敏感。触发软膝的其余条件：`peak_luminance >= 600`（MDCV 母版峰值）或 `max_cll >= 300`。

## HLG → SDR 的亮度保持

参考值是同一内容直接编成 SDR 的母版（YAVG 105.3）。

| 链条 | YAVG | YMAX（8-bit，合法上限 235） | 相对母版 |
| --- | --- | --- | --- |
| 母版（SDR 直通） | 105.3 | 219 | — |
| **SDR 参考素材 · BT.2408 直映射** | **111.2** | **229** | **+5.5%** |
| SDR 参考素材 · 软膝（旧链条 `npl=100`+`hable:desat=0`） | 75.9 | 147 | −28%（整体压暗，高光同时塌到 147） |
| 满量程素材 · BT.2408 直映射 | 187.9 | **255（越界）** | 不适用 |
| **满量程素材 · 软膝 `tonemap=hable:desat=0`** | **128.9** | **231** | 合法且保留层次 |

结论：两条链都必须保留，按素材选择——这正是“自动”档做的事。旧实现对所有 HDR 一律线性化 + `tonemap`，把手机 HLG 整体压暗约 28%，属于可感知缺陷。

## 已验证的行为

- **HLG 输出标签落地**：`color_primaries=bt2020`、`color_transfer=arib-std-b67`、`color_space=bt2020nc`、`color_range=tv`、`pix_fmt=yuv420p10le`（仅需命令行 `-color_*`，不依赖 `-tag:v hvc1` 或 `-x265-params colorprim/transfer`）。
- **HDR→SDR 后标签改写**：转换输出为 `bt709` 三件套，不会把 BT.2100 标签留在 SDR 像素上。
- **`tonemap` 算法**：`hable`/`mobius`/`reinhard`/`clip` 均可用，满量程素材 YMAX 分别 231/231/230/235，均落在合法量程内；`clip` 最亮但会削顶，`hable` 作为默认。
- **`tonemap=bt2390` 确认不可用**（`Undefined constant`），因此自动档不依赖 BT.2390 实现。
- **`npl` 不参与色彩链**：`npl=100/1000` 对 SDR 参考素材只是整体等比压暗（实测 YAVG 111→96→52），已从全部链条移除。
- **MDCV / MaxCLL 读取**：`-show_streams` 不含这些字段，必须用 `-read_intervals "%+#1" -show_frames -show_entries frame=side_data_list`；实测读到 `max_luminance=10000000/10000`、`max_content=700`、`max_average=200`，单帧探测约 183 ms。
- **HDR10 输出元数据回写**：`master-display`/`max-cll` 优先复用源 side_data，缺失时回落 BT.2020 默认值。
- **未标注素材**：`color_transfer` 缺失（`unspecified`/`unknown`/缺字段）但宽色域 + 10-bit 时只标记 `hlg_suspect`，`hdr_mode` 仍为 `sdr`，像素按 SDR 处理；用户在任务详情确认后（`sourceColorOverride`）才按 BT.2100 解释。
- **占位标签回落**：`unknown`/`unspecified` 的三原色被忽略并回落到矩阵判定（`bt2020nc`），不会被当成 BT.709。
- **跟随源文件的保护**：HDR 源在“跟随源文件”下拒绝 H.264、Rec.709 目标与 8-bit 降级；已确认只是 SDR 的素材不受此限制。

## 验证过程中的两个工具陷阱（写死在脚本注释里）

1. **ffprobe 字段名是 `color_transfer`，不是编码器选项名 `color_trc`**。写成 `color_trc` 时 ffprobe 静默省略该字段，会被误读成“HLG 标签没写进去”。
2. **`metadata=print:file=<Windows 绝对路径>` 解析失败**（`No option name near '/Users/...'`），因为滤镜图里的冒号转义与该选项的解析冲突。改为不带 `file=`、从 stderr 的 `av_log` 输出解析，既避开转义也不需要临时文件。

## 已知边界

- **SDR → HDR10 的亮度缩放尚未处理**：该路径只做色彩空间/传递函数换算，不会把 SDR 亮度映射到 HDR 目标亮度，输出可能偏暗。属于既有行为，本次未改变。
- 编码域峰值只在“HDR 模式 = SDR 且色调映射 = 自动”时抽样，其余组合为 0（表示未测），不会额外增加探测开销。
- Alpha 遮罩导出链路是灰度输出，不参与色彩链，本次未纳入验证。
- 疑似 HDR 素材保持按 SDR 编码，是“不误判优先于自动猜对”的有意取舍；界面提供徽标与下拉，由用户确认。
