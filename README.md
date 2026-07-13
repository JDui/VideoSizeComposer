# VideoSize Composer

VideoSize Composer is a portable desktop batch video encoder built with Tauri v2, React, TypeScript, and a Rust backend. Its queue, preset, output, and naming workflow is modeled after Adobe Media Encoder while retaining a compact liquid-glass interface.

The main workflow is organized around three visible stages: add media, choose the output, and start compression. Before encoding, the workbench shows an estimated output size, duration, compatibility, and destination. Each completed task ends in an explicit terminal state.

## Original File Times

Preserving the source video's original timestamps is a first-class feature and is enabled by default.

- On Windows, both the original creation date and modification time are written to the encoded output and read back for verification.
- On macOS, the creation date is written with the native attribute API and the modification time is written separately; both are then read back for verification.
- If either timestamp cannot be restored, the task ends as `时间恢复失败` instead of being shown as a successful output.
- Encoding uses a same-folder partial file and only renames it to the final path after FFmpeg succeeds, so a failed encode is not presented as a finished file.
- Before a session starts, the output directory is checked for writability and sufficient free space; name collisions are resolved with a visible, predictable numeric suffix.
- A running session can be cancelled, which terminates FFmpeg and removes partial output files.

## Stack

- Tauri v2 desktop shell
- React + TypeScript + Vite frontend
- Rust backend commands for platform capability detection, presets, ffprobe metadata, FFmpeg command construction, and encoding progress events
- FFmpeg / ffprobe expected on `PATH` or bundled next to the app by the final packaging step

## Encoding Features

- Saved, reusable presets with codec, bitrate, source/short-edge/percentage resolution, per-file source bit depth/chroma inheritance, color/HDR mode, LUT, output strategy, naming, panorama, and timestamp settings.
- A collapsible “My Presets” drawer with separate create/edit dialogs and persistent deletion; task output settings remain independent from the preset library.
- Persistent application preferences for default hardware/output behavior, timestamp handling, clear confirmation, and task-detail behavior.
- H.264, H.265/HEVC, AV1, and ProRes 422 output; 8/10-bit and 4:2:0/4:2:2 combinations are validated against the selected codec.
- SDR, HLG, and HDR10 pixel conversion through FFmpeg `zscale`, with matching transfer/primaries metadata. HDR conversion and 4:2:2 use the CPU path for predictable output.
- Dolby Vision source preservation uses lossless HEVC stream copy so the existing RPU is retained. This mode deliberately disallows scaling and LUT processing; it does not synthesize new Dolby Vision metadata.
- LUT files (`.cube`, `.3dl`, `.lut`) can be imported, stored, enabled, and saved as part of a preset.
- Tagged panorama sources and 2:1 equirectangular candidates are recognized separately. Panorama output receives both Google Spherical Video V1 metadata and standard V2 `st3d`/`sv3d` boxes, then passes an ffprobe readback check before completion.
- Batch import accepts files, folders, and drag-and-drop. Selected rows can share a preset, destination, and naming policy.

## Platform Behavior

- Windows shows `CUDA`, `Auto`, and `CPU`; it does not show Metal.
- macOS shows `Metal`, `Auto`, and `CPU`; it does not show CUDA.
- Other platforms show `Auto` and `CPU`.

## Default Presets

- H.265 10bit HDR
- H.264 Source 30% Bitrate
- AV1 1080p 10bit
- ProRes 422 HQ
- H.265 HLG 10bit
- H.265 HDR10 10bit
- Dolby Vision Preserve

The H.264/H.265 source-multiplier presets keep the source resolution and default to `source bitrate * 0.30`. The multiplier is editable and saved with the preset.

## Development

Install the Tauri prerequisites before native builds:

- Rust stable toolchain
- Node.js or pnpm
- WebView2 runtime on Windows
- Xcode Command Line Tools on macOS

```powershell
pnpm install
pnpm tauri dev
```

## Portable Build

```powershell
pnpm build:portable
```

The portable Windows onedir folder is created at `dist/VideoSizeComposer`. Run `VideoSizeComposer.exe` directly; FFmpeg and ffprobe are bundled beside it.

For macOS, build the `.app` bundle on macOS:

```bash
pnpm build:mac
```

Validated on Windows in this workspace:

- `pnpm build`
- `cargo test` (15 tests, including real codec, HDR, LUT, panorama metadata, and independent creation/modification-time preservation checks)
- `pnpm tauri build --no-bundle`
- `pnpm build:portable`
