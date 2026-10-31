# 2026v12-05 verification

Date: 2026-10-31

- `pnpm build` passed. `cargo test --offline --locked v12_05 --lib -- --nocapture`: 2 passed; 55 unrelated tests were filtered out.
- `cargo build --offline --locked --release --features custom-protocol` passed. Packaged `dist/VideoSizeComposer/VideoSizeComposer-2026v12-05.exe` (product version `2026.12.5`, 11,479,552 bytes). The original stable executable remained in use, so it was preserved alongside the new executable with the existing configuration and tools.
- Panorama guard: direct crop and canvas changes are rejected in job validation, including an INSV item with its panorama flag cleared. The geometry-filter helper also refuses to add crop filters defensively. A real spherical MP4 fixture produced different original/LUT JPEG previews while keeping its full dimensions and valid middle trim.
- SDK: tested local discovery and a real ZIP extraction/install using an inert runtime fixture and a model file, including a Unicode directory name. Missing-model packages are rejected without disturbing the installed fixture. Traversal paths and nonofficial download origins are rejected. No fixture executable was run.
- Native HTTP acquisition command was checked against the official latest-release API with the application's curl options; the request succeeded and returned zero binary assets. The [official repository](https://github.com/Insta360Develop/Desktop-MediaSDK-Cpp) directs users to apply for the complete SDK. Setup therefore reports account authorization when no public package exists; official HTTPS ZIP direct links and locally downloaded ZIPs use the automatic installer.
- Frontend checks: panorama disables crop and canvas controls, keeps LUT controls active, and preserves trim/LUT when toggling an edited item to panorama. SDK settings expose detection, local search/download, official download page and direct ZIP URL actions. At 1180×760 there is no horizontal overflow; console errors: none. Browser operations explicitly state their desktop-only boundary.
- Full SDK account download, native settings interaction and real INSV stitching with a proprietary SDK were not performed. No SHA checks or broad regression suite were run.

## Earlier 2026v12-04 verification

Date: 2026-10-31

- `pnpm build` and `cargo build --offline --release --features custom-protocol` passed. The new portable executable is `dist/VideoSizeComposer/VideoSizeComposer-2026v12-04.exe`, product version `2026.12.4`, 11,100,672 bytes. The existing stable executable was still running and was left in place; configuration and bundled tools were preserved.
- The editor uses a single-line title, 26px normal controls, 42/44px header/footer and a maximum dialog size of 1120×780. At 1180×760, the preview grows from 386px to 450px; dark/light themes have no horizontal overflow.
- Actual crop-corner and timeline in-point drags update their values. Expanded canvas/LUT settings, including extra dimension inputs, remain within the dialog. The audio editor fits in 920×430 without the video crop toolbar.
- The production build was checked through the in-app browser. `density-final.jpg` records the compact editor at the restored default 1280×720 viewport (1120×672 dialog, 410px preview, no horizontal overflow). Console errors: none. Temporary viewport and theme changes were restored.
- Comparison images and details are in `design-qa.md`. Media filters are unchanged, so the earlier three focused backend tests were not repeated. Full native manual acceptance with user media remains outstanding.

## Earlier 2026v12-03 verification

Date: 2026-10-31

- `pnpm build` and `cargo build --offline --release --features custom-protocol` passed. The portable executable was updated to product version `2026.12.3` without replacing its configuration or bundled tools.
- Editor styling now follows the application's semantic theme and shared button/input conventions. Dark and light palettes were checked in the in-app browser at 1317×1017 and 1180×760.
- A single preview replaces the source/output selector. Checked: one preview region and zero selector tablists; crop completion retains its values; LUT changes finish direct manipulation; a full-frame no-op crop is discarded; applying and reopening retains edits.
- Actual pointer dragging of crop/timeline handles was checked; expanded canvas/LUT settings do not overflow. The minimum window keeps a 386px-high preview and no horizontal page overflow. Browser console errors: none.
- UI images and the reference comparison are recorded in `design-qa.md`.
- The Rust media backend is unchanged in this pass, so the earlier three targeted media tests were not repeated. Native manual acceptance with user media remains outstanding.

## Earlier 2026v12-02 verification

Date: 2026-10-31

- `pnpm build`: TypeScript and production assets passed.
- `cargo build --offline --release --features custom-protocol`: passed; the portable executable in `dist/VideoSizeComposer/VideoSizeComposer.exe` was updated to product version `2026.12.2`, preserving its existing config and bundled tools.
- `cargo test --offline media_edit --lib -- --nocapture`: 3 passed; the other 52 tests were not run.
- Real media coverage: crop/canvas/trim dimensions and duration; original/edited JPEG preview; 50% LUT preview and encoding; spherical metadata removal after geometry changes; sequence, external short audio and Alpha using the same range and size; rotated source dimensions; rejection of audio export without a soundtrack.
- MP3: both audio and video source paths produced one MP3 stream at 48 kHz and 320,000 bps from a 16 kHz source, with the requested one-second segment.
- The Media Encoder style editor was checked through the in-app browser at 1317×1017 and 1180×760, including real pointer dragging of crop and trim handles, apply/reopen, zoom boundaries and console errors. Details and source comparison are in `design-qa.md`.
- Browser screenshots use an explicitly labelled FFmpeg source-thumbnail fallback. They are visual/interaction evidence, not native output-preview acceptance.
- Full native manual acceptance with real user media was not completed.

No SHA verification or broad regression suite was run.
