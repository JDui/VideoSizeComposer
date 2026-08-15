import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { AppPreferences, EncodeJob, EncodeProgress, PlatformInfo, Preset, QueueItem, ToolStatus } from "./types";

export const isTauriRuntime = "__TAURI_INTERNALS__" in window;
const browserPresetsKey = "videosize-composer.presets.v1";
const preferencesKey = "videosize-composer.preferences.v1";

export const defaultPreferences: AppPreferences = {
  defaultHardware: "auto",
  defaultOutputMode: "subfolder",
  defaultOutputDir: "",
  keepTimesByDefault: true,
  confirmBeforeClear: true,
  autoOpenDetails: true,
  defaultSequenceFps: 30
};

export async function getPlatformInfo(): Promise<PlatformInfo> {
  if (!isTauriRuntime) return { os: "windows", accelerators: ["auto", "cuda", "cpu"] };
  return invoke("get_platform_info");
}

export async function getToolStatus(): Promise<ToolStatus> {
  if (!isTauriRuntime) {
    return {
      ffmpeg: "FFmpeg 6.1.1 / bundled",
      ffprobe: "FFprobe 6.1.1 / bundled",
      encoders: ["libx264", "libx265", "h264_nvenc", "hevc_nvenc", "prores_ks", "libaom-av1"],
      ok: true
    };
  }
  return invoke("get_tool_status");
}

export async function loadPresets(): Promise<Preset[]> {
  if (!isTauriRuntime) {
    const stored = localStorage.getItem(browserPresetsKey);
    if (!stored) {
      const defaults = demoPresets();
      localStorage.setItem(browserPresetsKey, JSON.stringify(defaults));
      return defaults;
    }
    try {
      return (JSON.parse(stored) as Partial<Preset>[]).map(normalizePreset);
    } catch {
      return demoPresets();
    }
  }
  const presets = await invoke<Preset[]>("load_presets");
  return presets.map(normalizePreset);
}

export async function savePreset(preset: Preset): Promise<Preset[]> {
  if (!isTauriRuntime) {
    const items = await loadPresets();
    const next = upsertPreset(items, normalizePreset(preset));
    localStorage.setItem(browserPresetsKey, JSON.stringify(next));
    return next;
  }
  const presets = await invoke<Preset[]>("save_preset", { preset: normalizePreset(preset) });
  return presets.map(normalizePreset);
}

export async function deletePreset(id: string): Promise<Preset[]> {
  if (!isTauriRuntime) {
    const next = (await loadPresets()).filter((item) => item.id !== id);
    localStorage.setItem(browserPresetsKey, JSON.stringify(next));
    return next;
  }
  const presets = await invoke<Preset[]>("delete_preset", { id });
  return presets.map(normalizePreset);
}

export async function pickVideoFiles(): Promise<string[]> {
  if (!isTauriRuntime) return [];
  const result = await open({
    multiple: true,
    directory: false,
    filters: [{ name: "Video", extensions: ["mp4", "mov", "mkv", "mxf", "avi", "webm", "m4v", "mts", "m2ts"] }]
  });
  if (!result) return [];
  return Array.isArray(result) ? result : [result];
}

export async function pickSequenceFrame(): Promise<string[]> {
  if (!isTauriRuntime) return [];
  const result = await open({
    multiple: false,
    directory: false,
    filters: [{
      name: "序列帧",
      extensions: ["png", "jpg", "jpeg", "tif", "tiff", "bmp", "webp", "exr", "dpx", "tga"]
    }]
  });
  if (!result) return [];
  return Array.isArray(result) ? result : [result];
}

export async function pickSequenceFolder(): Promise<string[]> {
  if (!isTauriRuntime) return [];
  const result = await open({ multiple: false, directory: true });
  return result && !Array.isArray(result) ? [result] : [];
}

export async function pickFolder(): Promise<string[]> {
  if (!isTauriRuntime) return [];
  const result = await open({ multiple: false, directory: true });
  return result && !Array.isArray(result) ? [result] : [];
}

export async function pickOutputFolder(): Promise<string> {
  if (!isTauriRuntime) return "";
  const result = await open({ multiple: false, directory: true });
  return result && !Array.isArray(result) ? result : "";
}

export async function pickLutFiles(): Promise<string[]> {
  if (!isTauriRuntime) return [];
  const result = await open({
    multiple: true,
    directory: false,
    filters: [{ name: "LUT", extensions: ["cube", "3dl", "lut"] }]
  });
  if (!result) return [];
  const paths = Array.isArray(result) ? result : [result];
  return invoke("import_lut_files", { paths });
}

export async function probePaths(paths: string[], presetId: string): Promise<QueueItem[]> {
  if (!isTauriRuntime) return demoQueue(presetId);
  return invoke("probe_paths", { paths, presetId });
}

export async function probeSequencePaths(paths: string[], presetId: string, defaultFps: number): Promise<QueueItem[]> {
  if (!isTauriRuntime) return [];
  return invoke("probe_sequence_paths", { paths, presetId, defaultFps });
}

export async function startEncode(jobs: EncodeJob[]): Promise<string> {
  if (!isTauriRuntime) return `demo-session-${jobs.length}`;
  return invoke("start_encode", { jobs });
}

export async function cancelEncode(sessionId: string): Promise<void> {
  if (!isTauriRuntime) return;
  return invoke("cancel_encode", { sessionId });
}

export function onEncodeProgress(handler: (payload: EncodeProgress) => void) {
  if (!isTauriRuntime) return Promise.resolve(() => undefined);
  return listen<EncodeProgress>("encode-progress", (event) => handler(event.payload));
}

export async function onNativeDrop(handler: (paths: string[]) => void): Promise<() => void> {
  if (!isTauriRuntime) return () => undefined;
  const { getCurrentWebview } = await import("@tauri-apps/api/webview");
  return getCurrentWebview().onDragDropEvent((event: { payload: { type: string; paths?: string[] } }) => {
    if (event.payload.type === "drop" && event.payload.paths?.length) handler(event.payload.paths);
  });
}

export function normalizePreset(preset: Partial<Preset>): Preset {
  const bitDepth = preset.bitDepth === "source" ? "source" : Number(preset.bitDepth ?? 0);
  const isLegacyProResDefault = preset.id === "prores-422-hq";
  return {
    id: preset.id ?? crypto.randomUUID(),
    name: isLegacyProResDefault ? "ProRes 422 LT" : preset.name ?? "H.265 10bit HDR",
    codec: preset.codec ?? "h265",
    resolutionMode: preset.resolutionMode ?? "source",
    shortEdge: preset.shortEdge ?? 1080,
    scalePercent: preset.scalePercent ?? 50,
    customWidth: preset.customWidth ?? 1920,
    customHeight: preset.customHeight ?? 1080,
    bitrateMode: preset.bitrateMode ?? "source_multiplier",
    bitrateMultiplier: preset.bitrateMultiplier ?? 0.3,
    targetBitrateMbps: preset.targetBitrateMbps ?? 20,
    hardware: preset.hardware ?? "auto",
    outputMode: preset.outputMode ?? "subfolder",
    outputDir: preset.outputDir ?? "",
    outputContainer: preset.outputContainer ?? (preset.codec === "prores" ? "mov" : "source"),
    namingMode: preset.namingMode ?? "suffix_prefix",
    prefix: preset.prefix ?? "",
    suffix: isLegacyProResDefault && preset.suffix === "_prores422" ? "_prores422lt" : preset.suffix ?? "_compressed",
    keepTimes: preset.keepTimes ?? true,
    keepPanorama: preset.keepPanorama ?? true,
    alphaBackground: preset.alphaBackground === "black" || preset.alphaBackground === "white" ? preset.alphaBackground : "checkerboard",
    colorSpace: preset.colorSpace ?? "source",
    hdrMode: preset.hdrMode ?? "source",
    bitDepth: bitDepth === 8 || bitDepth === 10 ? bitDepth : "source",
    chroma: preset.chroma === "420" || preset.chroma === "422" ? preset.chroma : "source",
    lutEnabled: preset.lutEnabled ?? false,
    lutName: preset.lutName ?? "",
    lutIntensity: preset.lutIntensity ?? 80,
    cpuFallback: preset.cpuFallback ?? true
  };
}

export function loadPreferences(): AppPreferences {
  try {
    return { ...defaultPreferences, ...JSON.parse(localStorage.getItem(preferencesKey) ?? "{}") };
  } catch {
    return defaultPreferences;
  }
}

export function savePreferences(preferences: AppPreferences) {
  localStorage.setItem(preferencesKey, JSON.stringify(preferences));
}

function demoPresets(): Preset[] {
  return [
    normalizePreset({
      id: "h265-source-30",
      name: "H.265 10bit（保留 HDR）",
      codec: "h265",
      hardware: "cuda",
      outputDir: "D:/Output",
      suffix: "_h265_10bit",
      lutEnabled: true,
      lutName: "CineLook Soft.cube"
    }),
    normalizePreset({
      id: "h264-source-30",
      name: "H.264 高画质",
      codec: "h264",
      hardware: "cuda",
      outputDir: "D:/Output",
      suffix: "_h264_high",
      bitDepth: 8
    }),
    normalizePreset({
      id: "av1-1080-10bit",
      name: "AV1 1080p 10bit",
      codec: "av1",
      resolutionMode: "short_edge",
      shortEdge: 1080,
      bitrateMode: "target_mbps",
      targetBitrateMbps: 8,
      suffix: "_av1_1080p"
    }),
    normalizePreset({
      id: "prores-422-lt",
      name: "ProRes 422 LT",
      codec: "prores",
      chroma: "422",
      bitrateMode: "source_multiplier",
      bitrateMultiplier: 1,
      outputContainer: "mov",
      suffix: "_prores422lt"
    }),
    normalizePreset({ id: "h265-hlg-10bit", name: "H.265 HLG 10bit", codec: "h265", colorSpace: "rec2020", hdrMode: "hlg", hardware: "cpu", suffix: "_hlg" }),
    normalizePreset({ id: "h265-hdr10-10bit", name: "H.265 HDR10 10bit", codec: "h265", colorSpace: "rec2020", hdrMode: "hdr10", hardware: "cpu", suffix: "_hdr10" }),
    normalizePreset({ id: "dolby-vision-preserve", name: "Dolby Vision 保留导出", codec: "h265", hdrMode: "dolby_vision", resolutionMode: "source", suffix: "_dovi" })
  ];
}

function demoQueue(presetId: string): QueueItem[] {
  const rows: Array<Pick<QueueItem, "fileName" | "codec" | "width" | "height" | "fps" | "bitrate" | "duration" | "sizeBytes" | "isPanorama" | "progress" | "status">> = [
    { fileName: "Mountains_01.mov", codec: "HEVC 10-bit", width: 3840, height: 2160, fps: "29.97 fps", bitrate: 148000000, duration: 138, sizeBytes: 1288490188, isPanorama: false, progress: 62, status: "正在编码" },
    { fileName: "Waterfall_02.mp4", codec: "H.264 8-bit", width: 3840, height: 2160, fps: "59.94 fps", bitrate: 84000000, duration: 494, sizeBytes: 268435456, isPanorama: false, progress: 0, status: "排队中" },
    { fileName: "City_Night_03.mov", codec: "ProRes 422 HQ", width: 4096, height: 2160, fps: "23.98 fps", bitrate: 310000000, duration: 92, sizeBytes: 2630667468, isPanorama: true, progress: 0, status: "排队中" },
    { fileName: "Event_Interview_01.mov", codec: "H.264 10-bit", width: 1920, height: 1080, fps: "25 fps", bitrate: 58000000, duration: 311, sizeBytes: 134217728, isPanorama: false, progress: 15, status: "正在编码" },
    { fileName: "Product_Demo_01.mov", codec: "ProRes 422 HQ", width: 3840, height: 2160, fps: "29.97 fps", bitrate: 220000000, duration: 118, sizeBytes: 1932735283, isPanorama: false, progress: 0, status: "等待中" },
    { fileName: "Event_Stage_02.mp4", codec: "AV1 Main", width: 1920, height: 1080, fps: "50 fps", bitrate: 30000000, duration: 183, sizeBytes: 220200960, isPanorama: false, progress: 0, status: "等待中" }
  ];

  return rows.map((row, index) => ({
    id: `demo-${index}`,
    source: `D:/Media/${row.fileName}`,
    selected: true,
    presetId,
    output: `D:/Output/${row.fileName.replace(/\.[^.]+$/, "_encoded.mp4")}`,
    bitDepth: row.codec.includes("10") ? 10 : 8,
    chroma: row.codec.includes("ProRes") ? "422" : "420",
    colorSpace: index === 0 ? "bt2020" : "bt709",
    colorTransfer: index === 0 ? "arib-std-b67" : "bt709",
    hdrMode: index === 0 ? "hlg" : "sdr",
    audioTracks: index === 2 ? 2 : 1,
    subtitleTracks: index === 1 ? 1 : 0,
    panoramaTagged: row.isPanorama && index === 2,
    mediaKind: "video",
    sequencePattern: "",
    sequenceStartNumber: 0,
    sequenceFrameCount: 0,
    sequenceFps: 30,
    sequencePixelAspect: 1,
    hasAlpha: false,
    ...row
  }));
}

function upsertPreset(items: Preset[], preset: Preset) {
  return items.some((item) => item.id === preset.id)
    ? items.map((item) => (item.id === preset.id ? preset : item))
    : [...items, preset];
}
