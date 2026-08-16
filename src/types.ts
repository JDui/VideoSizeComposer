export type Codec = "h265" | "h264" | "av1" | "prores";
export type Hardware = "auto" | "cuda" | "metal" | "cpu";
export type OutputMode = "single_folder" | "in_place" | "subfolder";
export type NamingMode = "original" | "suffix_prefix";
export type OutputContainer = "source" | "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" | "m4a" | "wav";
export type ResolutionMode = "source" | "short_edge" | "scale_percent" | "custom";
export type FpsMode = "source" | "preset" | "custom";
export type HdrMode = "source" | "sdr" | "hlg" | "hdr10" | "dolby_vision";
export type Chroma = "source" | "420" | "422";
export type BitDepth = "source" | 8 | 10;
export type AlphaBackground = "checkerboard" | "black" | "white";

export interface PlatformInfo {
  os: "windows" | "macos" | "linux" | "unknown";
  accelerators: Hardware[];
}

export interface ToolStatus {
  ffmpeg: string;
  ffprobe: string;
  encoders: string[];
  ok: boolean;
}

export interface Preset {
  id: string;
  name: string;
  codec: Codec;
  resolutionMode: ResolutionMode;
  shortEdge: number;
  scalePercent: number;
  customWidth: number;
  customHeight: number;
  fpsMode: FpsMode;
  fpsValue: number;
  bitrateMode: "source_multiplier" | "target_mbps";
  bitrateMultiplier: number;
  targetBitrateMbps: number;
  hardware: Hardware;
  outputMode: OutputMode;
  outputDir: string;
  outputContainer: OutputContainer;
  namingMode: NamingMode;
  prefix: string;
  suffix: string;
  keepTimes: boolean;
  keepPanorama: boolean;
  alphaBackground: AlphaBackground;
  colorSpace: "source" | "rec709" | "rec2020";
  hdrMode: HdrMode;
  bitDepth: BitDepth;
  chroma: Chroma;
  lutEnabled: boolean;
  lutName: string;
  lutIntensity: number;
  cpuFallback: boolean;
}

export interface QueueItem {
  id: string;
  source: string;
  fileName: string;
  codec: string;
  width: number;
  height: number;
  fps: string;
  bitrate: number;
  duration: number;
  sizeBytes: number;
  thumbnail?: string;
  hasAlpha: boolean;
  isPanorama: boolean;
  panoramaTagged: boolean;
  bitDepth: number;
  chroma: string;
  colorSpace: string;
  colorTransfer: string;
  hdrMode: HdrMode;
  audioTracks: number;
  subtitleTracks: number;
  presetId: string;
  selected: boolean;
  output: string;
  status: string;
  progress: number;
  mediaKind: "video" | "sequence" | "audio";
  sequencePattern: string;
  sequenceStartNumber: number;
  sequenceFrameCount: number;
  sequenceFps: number;
  sequencePixelAspect: number;
  /** Whether an independent black/white Alpha matte should be encoded. */
  exportAlphaMask: boolean;
  /** Final Alpha matte output path, populated after encoding. */
  alphaOutput: string;
  /** Optional external audio file used as the output soundtrack. */
  externalAudio: string;
  /** Visual generated for audio-only media when producing video. */
  audioVisual: "timecode" | "black" | "white";
}

export interface EncodeJob {
  item: QueueItem;
  preset: Preset;
}

export interface EncodeProgress {
  itemId: string;
  progress: number;
  status: string;
  output?: string;
  ok?: boolean | null;
  message?: string;
  alphaOutput?: string;
}

export interface AppPreferences {
  defaultHardware: Hardware;
  defaultOutputMode: OutputMode;
  defaultOutputDir: string;
  keepTimesByDefault: boolean;
  confirmBeforeClear: boolean;
  autoOpenDetails: boolean;
  defaultSequenceFps: number;
}
