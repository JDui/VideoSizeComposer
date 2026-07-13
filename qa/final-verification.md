# Final Verification

Date: 2026-07-12  
Target: Windows portable onedir

## Automated checks

- TypeScript + Vite production build: passed.
- Rust suite: 15 passed, 0 failed.
- Real codecs exercised: H.264, H.265, AV1, ProRes 422.
- Real professional formats exercised: 10-bit, H.265 4:2:2, AV1 4:2:2, ProRes 422, HLG, HDR10.
- LUT encode: passed.
- Google Spherical Video V1 UUID plus V2 `st3d`/`sv3d` injection: passed and ffprobe-verified on a real MP4.
- Creation and modification timestamp preservation/readback: passed.
- Cancellation, cleanup, preflight, recursive import, output strategies, naming, and collision handling: passed.

## Visual checks

- Selected reference: `qa/selected-direction-1.png`.
- Final 1440×1024 implementation: `qa/implementation-final-1440.png`.
- Final 1180×760 implementation: `qa/implementation-final-1180.png`.
- Page-level horizontal overflow: none at either viewport.
- Browser console errors: none.
- Final design QA result: passed.

## Portable output

Folder: `dist/VideoSizeComposer`

- `VideoSizeComposer.exe`
- `ffmpeg.exe`
- `ffprobe.exe`
- `README-PORTABLE.txt`
