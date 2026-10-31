# Design QA — 2026v12-05 panorama protection and SDK settings

final result: passed

Inspected `qa/media-edit/panorama-locked.jpg` and `sdk-settings.jpg`, both captured from the production frontend at the application's minimum 1180×760 viewport. Existing compact spacing, semantic theme colors and shared controls are preserved.

- Panorama: the disabled crop control explains that the full projection is locked. No crop handles, margin inputs, ratio selector or canvas selector are available. LUT source, file selection and intensity remain enabled, as do the time range controls. Switching an already edited ordinary item to panorama clears its crop/canvas while retaining trim and LUT choices. The dialog fits within the viewport without horizontal overflow.
- SDK settings: detection, directory selection, recheck, automatic local search/download, official account download and the optional official ZIP URL are grouped in a compact section. The download action is disabled for an empty URL. Settings save/reset are disabled during an active setup task. Progress and failure messages have accessible status/alert regions.
- Browser demo explicitly reports that local SDK operations require the desktop app. It does not simulate a successful SDK download or processed LUT frame. Expanded direct-download fields produce no horizontal overflow and the footer remains reachable.

Console errors: none. Temporary viewport override was reset. Backend evidence includes an actual panorama LUT frame comparison and archive/discovery tests; authenticated official SDK download and a real SDK stitching session were not performed. The official repository currently exposes no binary release assets, so account authorization is presented as an actionable state rather than a successful installation.

---

# Earlier 2026v12-04 compact media editor

final result: passed

Source visual truth: the preceding application-themed editor in `qa/media-edit/density-before.jpg` (1180×760). Implementation: `density-minimum.jpg` at the same viewport and edit state; `density-desktop.jpg` at 1317×1017; `density-light.jpg` at 1180×760. The previous and compact dark editor, together with the light variant, were inspected in one comparison input.

- Typography and controls: title and filename share one header line, with truncation for long names. Normal controls are 26px high, the apply button is 28px, and existing theme colors and focus outlines remain in use.
- Layout: maximum dialog dimensions are 1120×780. Header/footer heights are 42/44px; toolbar, transport, timeline, range and collapsible settings have reduced vertical spacing. At the same 1180×760 viewport, the preview grows from 386px to 450px while the dialog remains within the viewport.
- Colors and imagery: the existing semantic dark/light palettes and neutral media monitor are preserved. The explicit source-thumbnail fallback label remains visible in browser demo mode.
- Interaction: actual crop-corner and timeline in-point dragging still update their values. Expanded canvas and LUT sections, including custom canvas dimensions, have no horizontal overflow; the preview remains 408px high with both sections expanded and the footer stays visible.
- Audio: the editor uses a 920×430 dialog without the crop toolbar; the timeline and range controls remain visible with no horizontal overflow.

Browser console errors: none. Temporary theme changes were restored. The single preview, crop completion, trim behavior and underlying media filters are unchanged. No remaining P0/P1/P2 visual issues. The final production capture is `qa/media-edit/density-final.jpg`, at the restored default 1280×720 viewport with a 1120×672 dialog and 410px preview. This pass verifies presentation and interactions; prior real-media backend evidence remains recorded below.

---

# Earlier 2026v12-03 application theme and single preview

final result: passed

Source visual truth: the current application's own controls and theme, captured in `qa/media-edit/app-theme-reference.jpg` (1317×1017).
Implementation: `qa/media-edit/theme-dark.jpg` and `theme-light.jpg` (1317×1017); minimum-window captures are `theme-dark-minimum.jpg` and `theme-light-minimum.jpg` (1180×760).

The app reference and final editor were opened together in one comparison input. Full-frame captures use the same CSS viewport at desktop density, including the title, crop controls, media area, timeline, settings and footer. All affected regions are readable, so no additional focused crop was needed.

- Typography and controls: match the application's system font, 11–12px control text, 15px title, shared input/button styles, 6–9px control radii and 12px modal radius. Filename and title have separate readable levels.
- Layout: crop margins are grouped consistently, the preview remains dominant, canvas/LUT controls use compact collapsible sections, and footer spacing follows other app modals. At 1180×760 the single preview is 386px high, document width is 1180px, and editor content/client widths are both 1130px.
- Colors: chrome uses the existing background/surface/text/muted/accent/action/border variables. Both the default dark palette and silver light palette follow their parent app; the light primary button correctly uses white text on its configured purple. The media monitor and white crop edge remain neutral.
- Imagery: the browser demo shows an explicitly labelled real FFmpeg test-frame thumbnail. Local-file preview failures are reported and are not presented as successful processed frames.
- Copy and interaction: the source/output selector is removed. One preview region remains. The crop tool enters direct manipulation, “完成裁切” preserves its values and shows the result, and canvas/LUT changes automatically return that same region to the processed preview. Clearing crop and opening/closing a full-frame crop tool do not leave unintended geometry edits.

In-app browser checks passed for minimum/default layouts in dark and light themes, expanded settings without horizontal overflow, actual crop/timeline dragging, finished-crop value retention, automatic LUT preview transition, apply/reopen and absence of duplicate preview selectors. Console errors: none. Temporary theme changes were restored.

Fixes made during comparison: removed the independently hardcoded gray/blue palette and tiny controls; reused semantic application styles; removed redundant preview-fallback text; removed the preview switch and reclaimed its row; replaced captures taken during viewport resizing with complete full-frame captures.

Validation boundary: this pass does not change the Rust media filters. Browser interaction and production compilation cover the new UI; actual media filter tests are recorded in the earlier verification below. Full native manual acceptance with user media remains outstanding. No remaining P0/P1/P2 visual issues.

---

# Earlier 2026v12-02 media editor QA

final result: passed

Source visual truth: `qa/media-edit/reference-ame.png` (user-supplied Adobe Media Encoder screenshot, 1317×1017).
Implementation: `qa/media-edit/ame-source.jpg` (1317×1017), `qa/media-edit/ame-minimum.jpg` (1180×760).
State: source tab, crop enabled, right/bottom margins set, custom source range with a centered 10-second segment; canvas and LUT settings collapsed.

The reference and implementation were opened together in one image comparison after capture through the in-app browser. Captures use CSS pixels at the same reference viewport; functional controls retain desktop density. The full captures show the source/output tabs, crop toolbar, image, crop selection and timeline at readable detail, so an additional focused image was unnecessary.

- Typography: compact Segoe UI/system text, blue numeric crop values and monospace timecode match the reference's hierarchy.
- Layout: the image dominates the center; source/output and pixel margins sit above it, with transport, in/out handles and range controls below. The surrounding app modal supplies its own title and apply/cancel footer.
- Color: graphite surfaces, restrained dividers, a white crop boundary with eight handles, dimmed excluded regions and a blue timeline marker follow the reference.
- Imagery: an actual FFmpeg test frame is used in browser demo data. The example media is 16:9, so the image bounds differ from the reference's 4:3 media without stretching either image.
- Copy and controls: source/output, four pixel margins, crop ratio, fit/zoom, frame stepping, source range, exact in/out values, centered segment, canvas and LUT controls are functional.

Interaction checks through the in-app browser:

- Double-click opens the editor. Actual pointer dragging of the crop corner updates the four margins; keyboard movement/resizing also updates them.
- Actual pointer dragging of the timeline in-point changes the source range. Frame stepping and keyboard adjustments work; centered 10 seconds yields 64–74 seconds for the 138-second demo.
- Applying and reopening preserves the crop and range. Source/output switching keeps source thumbnails out of output previews.
- At 100% zoom, the top-left image corner remains reachable after the overflow alignment fix.
- At 1180×760, document width is 1180, the dialog stays inside the viewport, and the image area remains 408 pixels high. No console errors were recorded.

Fixed during comparison: replaced the previous split form layout with the reference's preview-first structure; delayed queue pointer capture until a real marquee drag so double-click works; used safe image centering so enlarged images do not hide their top-left edge.

Validation boundary: browser demo mode labels the source thumbnail fallback and does not claim to render local-file output/LUT previews. Actual preview filters, LUT strength, trimming, MP3, sequence/external audio and Alpha were verified by three focused Rust tests using FFmpeg. Full native manual acceptance with user media remains outstanding.

No remaining P0/P1/P2 visual issues. P3: native window chrome and the application's apply/cancel footer intentionally wrap the referenced editor layout.

---

# Earlier workbench QA — Direction 1

source visual truth path: `C:\Users\Muxinzheng\Documents\VideoSizeComposer\qa\selected-direction-1.png`  
implementation screenshot path: `C:\Users\Muxinzheng\Documents\VideoSizeComposer\qa\implementation-final-1440.png`  
viewport: 1440 × 1024  
state: Windows desktop app browser fallback, six demo media files loaded, all selected, H.265 10-bit preset active, original creation and modification time preservation enabled  
full-view comparison evidence: the source and final implementation were opened together at original detail after the implementation capture.  
focused region comparison evidence: a separate crop was not required because both source and implementation are full-resolution desktop captures with the queue rows, forecast, preset rail, settings controls, icons, thumbnails and bottom action bar readable at original detail.

## Findings

- No remaining P0, P1 or P2 visual findings.
- The implementation intentionally shows only hardware available on the active platform. The source mock shows CUDA, Metal and CPU together, but the shipped Windows product must not advertise Metal.
- The implementation intentionally adds a prominent “保留原始时间 / 同时恢复创建日期和修改时间” control and verification state. This differs from the source only to satisfy the product's primary requirement.

## Required Fidelity Surfaces

- **Fonts and typography:** passed. The system product stack uses Inter/SF Pro/Segoe UI with Microsoft YaHei UI and PingFang SC fallbacks. The 10–15px hierarchy, weights, truncation and line heights closely reproduce the compact desktop target while remaining readable at the 1180×760 minimum window.
- **Spacing and layout rhythm:** passed. The 280px preset rail, flexible central workbench, 292px settings panel, 54px title bar and 72px action bar reproduce the target composition. Queue rows, prediction header and settings groups use the same visual rhythm without page-level overflow.
- **Colors and visual tokens:** passed. Deep navy/graphite liquid-glass surfaces, cool cyan focus, mint result/success colors, restrained translucent dividers and amber favorite accents match the source direction without reducing legibility.
- **Image quality and asset fidelity:** passed. All six queue rows use real raster thumbnail assets from the project with stable crops and no placeholder or CSS-drawn imagery. Icons use the existing Lucide product icon family, which matches the source's thin outline style.
- **Copy and app-specific content:** passed. The interface includes VideoSize Composer, recommended and personal presets, the three-step flow, source-to-output prediction, compatibility, output location, file details, H.264/H.265/AV1/ProRes, bit depth, resolution, quality, hardware status, output controls and the primary compression action.

## Interaction And Responsive Verification

- Tested the primary action from ready state through simulated progress to six terminal success states.
- Tested cancellation from the running state: the cancel action is unique and all six rows reach `已取消 / 未生成输出文件` with no console errors.
- Verified every completed row reports `创建与修改时间已保留`.
- Tested the details drawer open/closed state through the primary task flow.
- Verified the persistent CTA and output summary remain visible at 1440×1024 and 1180×760.
- At 1180×760, `window.innerWidth`, document `scrollWidth` and screenshot width all equal 1180; there is no page-level horizontal overflow. The right inspector scrolls internally when needed.
- Browser console error log checked after final reload: no errors.
- Rechecked after the professional-settings pass: uncalibrated duration reads `待首次校准`; HLG/HDR10, 10-bit, 4:2:2, LUT, output strategy, naming and panorama preservation controls are available in the inspector; Dolby Vision is explicitly constrained to source-stream preservation.
- HLG pass (2026-10-29): the inspector's 色彩 tab adds a `色调映射` select (自动 / BT.2408 直映射 / 软膝压缩高光 / 硬截顶, default 自动) that only appears when HDR 模式 is SDR, plus a hint line; the queue row can show an amber `HDR（未标注）` badge and the details modal gains a `源色彩解释` select (自动 / SDR / HLG / HDR10) alongside the probed HDR signal, mastering peak, MaxCLL/MaxFALL and coded-domain peak. `pnpm build` (tsc + vite) is clean; the colour math behind these controls is covered by `cargo test --lib` and `qa\hlg\verify-hlg.ps1`. Pixel-level UI screenshots of these two new controls have not been re-captured in this pass.
- Final minimum-window evidence: `qa\implementation-final-1180.png`; `innerWidth` and document `scrollWidth` both equal 1180, while the inspector and queue use internal scrolling.

## Comparison History

### Baseline audit

- [P1] The previous implementation forced a 1280px page minimum inside an 1180px native minimum window, clipping the inspector, toolbar and summary.
- [P1] The browser encoding flow ended at 56% while returning the primary button to idle, leaving no terminal success or failure state.
- Fixes: replaced the free-resize dashboard with the selected three-column guided workbench, removed the conflicting page minimum width, added a finite terminal state flow and made the primary action persistent.
- Post-fix evidence: `qa\implementation-min-window-1180x760.png` and `qa\implementation-complete-state.png`.

### Direction 1 implementation pass 1

- [P2] The output forecast was 3.3 GB rather than the direction's approximately 1.8 GB because the demo's stream bitrates and declared file sizes were inconsistent.
- [P2] Row thumbnails and index treatment were smaller/sparser than the selected visual, and the brand mark used a circular play symbol rather than the source's triangular mark.
- Fixes: calculate source-multiplier preview from declared file size plus audio overhead, enlarge row media, show row numbers, correct thumbnail ordering and use the matching play icon.
- Post-fix evidence: `qa\implementation-direction-1-v3.png`, which shows a 1.9 GB forecast and corrected row/brand treatment.

## Follow-up Polish

- [P3] Native Windows title-bar controls are supplied by the decorated Tauri window and therefore are not duplicated inside the web content as shown in the source mock.
- [P3] Output estimates deliberately use “预计” and may differ slightly from the synthetic mock values because the implementation calculates them from each queue item's real metadata.
- [P3] The selected visual's precise duration was replaced with `待首次校准` until device throughput has been measured; this is an intentional correctness change.
- [P3] macOS-specific typography and native packaging still require capture on macOS hardware.

final result: passed
