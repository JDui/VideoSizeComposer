# Windows Native End-to-End Verification

Date: 2026-07-11  
Application: `src-tauri\target\release\videosize-composer.exe`  
Platform: Windows / NTFS  
Flow: launch native Tauri app → import video through native file picker → keep original time enabled → run H.265 encode → inspect terminal UI state → read source/output filesystem metadata → inspect output with ffprobe.

## Source

- Filename: `timestamp-source.mp4`
- Synthetic media: 2 seconds, 320×180, 24 fps, H.264 + AAC
- Creation date: `2022-02-03 04:05:06.0000000`
- Modification time: `2023-03-04 05:06:07.0000000`

## Output

- Filename: `timestamp-source_h265_30pct.mp4`
- UI terminal state: `已完成`
- UI timestamp state: `创建与修改时间已保留`
- Session summary: `完成 1 · 失败 0 · 总计 1`
- Creation date: `2022-02-03 04:05:06.0000000`
- Modification time: `2023-03-04 05:06:07.0000000`
- Creation date exact match: `true`
- Modification time exact match: `true`
- ffprobe codec: `hevc`
- ffprobe dimensions: `320×180`
- ffprobe pixel format: `yuv420p10le`
- Partial `.vsc-part-*` artifacts remaining after success: none

## Result

Windows native end-to-end verification passed. The UI success state, valid encoded media, creation date, and modification time agree with the filesystem evidence.
