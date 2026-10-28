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

## Addendum — HLG read/write pass (2026-10-29)

- Rust suite: 48 passed, 0 failed (`cargo test --lib`), including 12 new HLG/colour cases with two real encode-and-measure regressions.
- TypeScript + Vite production build: passed (`pnpm build`).
- HLG chain harness: `qa/hlg/verify-hlg.ps1` — all checks pass; measurements and thresholds recorded in `qa/hlg/verification.md`.
- Verified on real encodes with the bundled FFmpeg: HLG tag round-trip (`color_transfer=arib-std-b67`), HLG→SDR display-referred preservation (+5.5% luma vs the SDR master), full-scale HLG highlight compression (YMAX 231 within legal range), MDCV/MaxCLL read from frame side data and reused on HDR10 output, unlabelled wide-gamut clips flagged as suspect without pixel reinterpretation.
- Not re-checked visually in this pass: the new `色调映射` select and the inspector's `源色彩解释` control.
