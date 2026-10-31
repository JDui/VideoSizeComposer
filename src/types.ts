export type Codec = "h265" | "h264" | "av1" | "prores";
export type Hardware = "auto" | "cuda" | "metal" | "cpu";
export type OutputMode = "single_folder" | "in_place" | "subfolder";
export type NamingMode = "original" | "suffix_prefix";
export type OutputContainer = "source" | "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" | "m4a" | "wav" | "mp3";
export type ResolutionMode = "source" | "short_edge" | "scale_percent" | "custom";
export type FpsMode = "source" | "preset" | "custom";
export type HdrMode = "source" | "sdr" | "hlg" | "hdr10" | "dolby_vision";
/** 探测到的源 HDR 信号；`*_suspect` 表示只找到间接证据，需用户确认。 */
export type HdrSignal = "sdr" | "hlg" | "hdr10" | "dolby_vision" | "hlg_suspect" | "hdr10_suspect";
/** 检查器里用户对源色彩的主动解释，用于未标注/错标的 HLG、PQ 素材。 */
export type SourceColorChoice = "auto" | "sdr" | "hlg" | "hdr10";
/** HDR→SDR 的高光映射策略。auto 依据源峰值亮度与编码域峰值自动选择。 */
export type ToneMap = "auto" | "bt2408" | "tonemap" | "clip";
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
  insta360Sdk?: string;
}

export interface Insta360SdkStatus {
  state: "ready" | "missing" | "unsupported" | "authorization_required";
  path: string | null;
  message: string;
}

export interface PanoramaPreferences {
  stabilization: "off" | "flowstate" | "direction_lock";
  stitching: "optflow" | "dynamicstitch" | "aistitch";
  stitchFusion: boolean;
  denoise: boolean;
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
  wavEncoding: "pcm16" | "pcm24" | "float32";
  namingMode: NamingMode;
  prefix: string;
  suffix: string;
  keepTimes: boolean;
  keepPanorama: boolean;
  panorama: PanoramaPreferences;
  alphaBackground: AlphaBackground;
  colorSpace: "source" | "rec709" | "rec2020";
  hdrMode: HdrMode;
  /** HDR→SDR 的高光映射方式。 */
  toneMap: ToneMap;
  bitDepth: BitDepth;
  chroma: Chroma;
  lutEnabled: boolean;
  lutName: string;
  lutIntensity: number;
  cpuFallback: boolean;
}

/** Per-item draft edits, committed only when the media editor is applied. */
export interface MediaEdits {
  trimStart: number;
  /** Zero keeps the source ending. */
  trimEnd: number;
  crop: { x: number; y: number; width: number; height: number } | null;
  /** Zero for both dimensions follows the output preset. */
  canvasWidth: number;
  canvasHeight: number;
  fit: "contain" | "cover";
  /** Null inherits the output preset LUT. */
  lutEnabled: boolean | null;
  lutName: string;
  lutIntensity: number;
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
  /** 探测到的源 HDR 信号（含未标注的疑似态），仅用于提示与默认解释。 */
  hdrSignal?: HdrSignal;
  /** 用户手动指定的源色彩解释；"auto"/缺省表示沿用探测结果。 */
  sourceColorOverride?: SourceColorChoice;
  /** ffprobe color_range：tv / pc / unspecified。 */
  colorRange?: string;
  /** ffprobe color_space（矩阵），三原色缺失时用于判定色域。 */
  colorMatrix?: string;
  /** 源文件携带的母版显示元数据（x265 写法），空表示未检出。 */
  masteringDisplay?: string;
  maxCll?: number;
  maxFall?: number;
  /** 母版峰值亮度 cd/m2；0 表示未携带 MDCV。 */
  peakLuminance?: number;
  /** 抽样得到的编码域峰值（1.0 = HLG/PQ 满量程）；0 表示未测。 */
  codedPeakRatio?: number;
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
  edits?: MediaEdits;
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
  theme: import("./theme").ThemeColors;
  defaultHardware: Hardware;
  defaultOutputMode: OutputMode;
  defaultOutputDir: string;
  keepTimesByDefault: boolean;
  confirmBeforeClear: boolean;
  autoOpenDetails: boolean;
  defaultSequenceFps: number;
  insta360SdkPath: string;
}
