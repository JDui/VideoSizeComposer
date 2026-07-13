# Design QA — Direction 1

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
