import { useEffect, useMemo, useRef, useState } from "react";
import packageMetadata from "../package.json";
import appIcon from "../assets/app-icon.png";
import { applyTheme } from "./theme";
import { ThemeSettings } from "./ThemeSettings";
import {
  Check,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  Clock3,
  Cpu,
  Edit3,
  FilePlus2,
  Film,
  Folder,
  FolderOpen,
  HelpCircle,
  Images,
  LoaderCircle,
  PanelLeftClose,
  PanelLeftOpen,
  Play,
  Plus,
  Search,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
  MoreHorizontal,
  Square,
  Trash2,
  X,
  Zap
} from "lucide-react";
import {
  cancelEncode,
  defaultPreferences,
  deletePreset as removePreset,
  getPlatformInfo,
  getToolStatus,
  isTauriRuntime,
  loadPresets,
  loadPreferences,
  normalizePreset,
  onEncodeProgress,
  onNativeDrop,
  pickFolder,
  pickAudioFiles,
  pickLutFiles,
  pickOutputFolder,
  pickSequenceFolder,
  pickSequenceFrame,
  pickVideoFiles,
  probePaths,
  probeSequencePaths,
  revealPath,
  savePreset,
  savePreferences,
  setAdaptiveConcurrency,
  startEncode
} from "./tauri";
import type { AppPreferences, EncodeJob, Hardware, PlatformInfo, Preset, QueueItem, SourceColorChoice, ToolStatus } from "./types";
import { codecLabels, OutputSettings, presetSummary } from "./OutputSettings";

const defaultPlatform: PlatformInfo = { os: "unknown", accelerators: ["auto", "cpu"] };
const defaultToolStatus: ToolStatus = { ffmpeg: "检测中", ffprobe: "检测中", encoders: [], ok: false };

const packageVersion = typeof packageMetadata.version === "string" && packageMetadata.version.trim()
  ? packageMetadata.version
  : "unknown";
const displayVersion = typeof packageMetadata.displayVersion === "string" && packageMetadata.displayVersion.trim()
  ? packageMetadata.displayVersion
  : deriveDisplayVersion(packageVersion);
const functionVersion = deriveFunctionVersion(displayVersion);

type ContextMenuState = {
  kind: "preset" | "media";
  id: string;
  left: number;
  top: number;
};

/** 折叠面板状态：记忆在 localStorage，重启应用后仍保持上次展开的分组。 */
function useToggleState(key: string, initial: boolean) {
  const [open, setOpen] = useState(() => {
    try {
      const stored = localStorage.getItem(key);
      return stored === null ? initial : stored === "1";
    } catch {
      return initial;
    }
  });
  const toggle = () => {
    setOpen((current) => {
      const next = !current;
      try {
        localStorage.setItem(key, next ? "1" : "0");
      } catch {
        /* 存储不可用时仅切换内存状态 */
      }
      return next;
    });
  };
  const set = (value: boolean) => {
    setOpen(value);
    try {
      localStorage.setItem(key, value ? "1" : "0");
    } catch {
      /* 存储不可用时仅更新内存状态 */
    }
  };
  return [open, toggle, set] as const;
}

export default function App() {
  const [platform, setPlatform] = useState<PlatformInfo>(defaultPlatform);
  const [toolStatus, setToolStatus] = useState<ToolStatus>(defaultToolStatus);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [activePresetId, setActivePresetId] = useState("");
  const [outputPreset, setOutputPreset] = useState<Preset>();
  const [queue, setQueue] = useState<QueueItem[]>([]);
  const [filter, setFilter] = useState("");
  const [presetFilter, setPresetFilter] = useState("");
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [mediaInfoItemId, setMediaInfoItemId] = useState("");
  const [presetRailCollapsed, togglePresetRail] = useToggleState("vsc.presetRailCollapsed", false);
  const [presetListOpen, togglePresetList] = useToggleState("vsc.presetListOpen", true);
  const [presetEditorOpen, setPresetEditorOpen] = useState(false);
  const [presetDraft, setPresetDraft] = useState<Preset>();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [addMenuOpen, setAddMenuOpen] = useState(false);
  const [sequenceImportOpen, setSequenceImportOpen] = useState(false);
  const [sequenceDraft, setSequenceDraft] = useState<QueueItem>();
  const [contextMenu, setContextMenu] = useState<ContextMenuState>();
  const [preferences, setPreferences] = useState<AppPreferences>(defaultPreferences);
  const [preferencesDraft, setPreferencesDraft] = useState<AppPreferences>(defaultPreferences);
  useEffect(() => {
    applyTheme(settingsOpen ? preferencesDraft.theme : preferences.theme);
  }, [settingsOpen, preferencesDraft.theme, preferences.theme]);
  const [adaptiveConcurrency, setAdaptiveConcurrencyState] = useState(false);
  const adaptiveConcurrencyRef = useRef(false);
  const activeSessionRef = useRef("");
  const adaptiveUpdateRef = useRef<Promise<void>>(Promise.resolve());
  const [encodingIds, setEncodingIds] = useState<string[]>([]);
  const [isImporting, setIsImporting] = useState(false);
  const [activeSessionId, setActiveSessionId] = useState("");
  const [activeJobIds, setActiveJobIds] = useState<string[]>([]);
  const [encodeStartedAt, setEncodeStartedAt] = useState<number | null>(null);
  const [clockNow, setClockNow] = useState(Date.now());
  const [marquee, setMarquee] = useState<{ left: number; top: number; width: number; height: number } | null>(null);
  const [lastError, setLastError] = useState("");
  const [notice, setNotice] = useState("");
  const [log, setLog] = useState<string[]>(["FFmpeg 环境检测中…", "等待添加媒体"]);
  const simulationTimer = useRef<number | null>(null);
  const importInProgress = useRef(false);
  const cancellingRef = useRef<Set<string>>(new Set());
  const queueBodyRef = useRef<HTMLDivElement>(null);
  const detailsControlRef = useRef<HTMLDivElement>(null);
  const detailsButtonRef = useRef<HTMLButtonElement>(null);
  const selectionAnchorId = useRef("");
  const suppressRowClick = useRef(false);
  const marqueeStart = useRef<{
    x: number;
    y: number;
    clientX: number;
    clientY: number;
    pointerId: number;
    additive: boolean;
    initialSelected: Set<string>;
    dragging: boolean;
  } | null>(null);

  const activePreset = outputPreset;
  const selectedItems = useMemo(() => queue.filter((item) => item.selected), [queue]);
  const visibleItems = useMemo(
    () => queue.filter((item) => item.fileName.toLowerCase().includes(filter.trim().toLowerCase())),
    [filter, queue]
  );
  const isEncoding = encodingIds.length > 0;
  const totalSourceSize = selectedItems.reduce((sum, item) => sum + item.sizeBytes, 0);
  const totalEstimatedSize = selectedItems.reduce(
    (sum, item) => sum + estimateOutputBytes(item, presetForItem(item, presets, activePreset)),
    0
  );
  const savings = totalSourceSize > 0 ? Math.max(0, Math.round((1 - totalEstimatedSize / totalSourceSize) * 100)) : 0;
  const completedCount = queue.filter((item) => item.status.includes("完成")).length;
  const failedCount = queue.filter((item) => item.status.includes("失败")).length;
  const activeDuration = activeJobIds.reduce((sum, id) => sum + (queue.find((item) => item.id === id)?.duration ?? 0), 0);
  const overallProgress = activeDuration > 0
    ? Math.round(activeJobIds.reduce((sum, id) => {
        const item = queue.find((entry) => entry.id === id);
        return sum + (item?.duration ?? 0) * (item?.progress ?? 0);
      }, 0) / activeDuration)
    : 0;
  const remainingTimeLabel = isEncoding && encodeStartedAt
    ? estimateRemainingTime(overallProgress, clockNow - encodeStartedAt)
    : "";

  useEffect(() => {
    let cancelled = false;
    getPlatformInfo().then(setPlatform);
    getToolStatus().then((status) => {
      setToolStatus(status);
      setLog(status.ok ? ["FFmpeg 与 ffprobe 已就绪", "等待添加媒体"] : ["编码工具检测失败"]);
    });
    (async () => {
      try {
        const loadedPreferences = await loadPreferences();
        if (cancelled) return;
        setPreferences(loadedPreferences);
        setPreferencesDraft(loadedPreferences);
        const items = await loadPresets();
        if (cancelled) return;
        const normalized = items.map(normalizePreset);
        const initial = normalized[0] ?? normalizePreset({
          id: crypto.randomUUID(),
          name: "未保存的输出设置",
          hardware: loadedPreferences.defaultHardware,
          outputMode: loadedPreferences.defaultOutputMode,
          outputDir: loadedPreferences.defaultOutputDir,
          keepTimes: loadedPreferences.keepTimesByDefault
        });
        setPresets(normalized);
        setActivePresetId(normalized[0]?.id ?? "");
        setOutputPreset(initial);
        if (!isTauriRuntime) {
          const demo = await probePaths(["demo"], initial.id);
          if (!cancelled) setQueue(demo.map((item) => ({ ...item, selected: true, status: "就绪", progress: 0 })));
        }
      } catch (error) {
        if (!cancelled) {
          const message = error instanceof Error ? error.message : String(error);
          setLastError(message);
          setLog((lines) => [`便携配置加载失败：${message}`, ...lines].slice(0, 120));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onEncodeProgress((payload) => {
      const terminal = payload.ok === true || payload.ok === false;
      const isCancelling = cancellingRef.current.has(payload.itemId);
      setQueue((items) =>
        items.map((item) => {
          if (item.id !== payload.itemId) return item;
          // 取消进行中：忽略非终态进度更新，避免把“正在取消”覆盖回“编码中”
          if (isCancelling && !terminal) {
            return { ...item, progress: Math.max(item.progress, payload.progress) };
          }
          return {
            ...item,
            progress: Math.max(item.progress, payload.progress),
            status: payload.status,
            output: payload.output ?? item.output,
            alphaOutput: payload.ok === false
              && !payload.status.includes("时间恢复失败")
              && (payload.status.includes("失败") || payload.status.includes("取消"))
              ? ""
              : payload.alphaOutput ?? item.alphaOutput
          };
        })
      );
      if (payload.message) setLog((lines) => [payload.message!, ...lines].slice(0, 120));
      if (payload.ok === false && payload.message) setLastError(payload.message);
      if (terminal) {
        cancellingRef.current.delete(payload.itemId);
        setEncodingIds((ids) => ids.filter((id) => id !== payload.itemId));
      }
    }).then((dispose) => {
      unlisten = dispose;
    });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onNativeDrop((paths) => addPaths(paths)).then((dispose) => {
      unlisten = dispose;
    });
    return () => unlisten?.();
  }, [activePresetId, activePreset]);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(""), 2200);
    return () => window.clearTimeout(timer);
  }, [notice]);

  useEffect(() => {
    if (!isEncoding) {
      setActiveSessionId("");
      activeSessionRef.current = "";
    }
  }, [isEncoding]);

  useEffect(() => {
    if (!isEncoding) return;
    const timer = window.setInterval(() => setClockNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [isEncoding]);

  useEffect(() => {
    if (!contextMenu) return;
    const closeOnPointerDown = (event: PointerEvent) => {
      const target = event.target as HTMLElement | null;
      if (!target?.closest(".context-menu")) setContextMenu(undefined);
    };
    const closeOnKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setContextMenu(undefined);
    };
    const closeOnViewportChange = () => setContextMenu(undefined);
    window.addEventListener("pointerdown", closeOnPointerDown);
    window.addEventListener("keydown", closeOnKeyDown);
    window.addEventListener("resize", closeOnViewportChange);
    window.addEventListener("scroll", closeOnViewportChange, true);
    return () => {
      window.removeEventListener("pointerdown", closeOnPointerDown);
      window.removeEventListener("keydown", closeOnKeyDown);
      window.removeEventListener("resize", closeOnViewportChange);
      window.removeEventListener("scroll", closeOnViewportChange, true);
    };
  }, [contextMenu]);

  useEffect(() => {
    if (!detailsOpen) return;
    const closeOnPointerDown = (event: PointerEvent) => {
      if (!detailsControlRef.current?.contains(event.target as Node)) setDetailsOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      setDetailsOpen(false);
      detailsButtonRef.current?.focus();
    };
    const closeOnFocusOutside = (event: FocusEvent) => {
      if (!detailsControlRef.current?.contains(event.target as Node)) setDetailsOpen(false);
    };
    window.addEventListener("pointerdown", closeOnPointerDown);
    window.addEventListener("keydown", closeOnEscape);
    window.addEventListener("focusin", closeOnFocusOutside);
    return () => {
      window.removeEventListener("pointerdown", closeOnPointerDown);
      window.removeEventListener("keydown", closeOnEscape);
      window.removeEventListener("focusin", closeOnFocusOutside);
    };
  }, [detailsOpen]);

  useEffect(() => () => {
    if (simulationTimer.current !== null) window.clearInterval(simulationTimer.current);
  }, []);

  async function addPaths(paths: string[]) {
    await importVideos(async () => paths);
  }

  async function importVideos(getPaths: () => Promise<string[]>) {
    if (importInProgress.current || !activePreset) return;
    importInProgress.current = true;
    setIsImporting(true);
    try {
      const paths = await getPaths();
      if (!paths.length) return;
      setLastError("");
      const items = await probePaths(paths, activePreset.id);
      setQueue((existing) => mergeQueue(existing, items.map((item) => ({ ...item, selected: true, status: "就绪" }))));
      setLog((lines) => ["已导入 " + items.length + " 个媒体", ...lines].slice(0, 120));
      if (!items.length) setLastError("没有找到可导入的媒体，或媒体信息读取失败。");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setLastError(message);
      setLog((lines) => [`导入失败：${message}`, ...lines].slice(0, 120));
    } finally {
      importInProgress.current = false;
      setIsImporting(false);
    }
  }

  async function importSequence(getPaths: () => Promise<string[]>) {
    if (importInProgress.current || !activePreset) return;
    importInProgress.current = true;
    setIsImporting(true);
    setSequenceImportOpen(false);
    try {
      const paths = await getPaths();
      if (!paths.length) return;
      setLastError("");
      const items = await probeSequencePaths(paths, activePreset.id, preferences.defaultSequenceFps);
      setQueue((existing) => mergeQueue(existing, items.map((item) => ({
        ...item,
        selected: true,
        status: "就绪",
        output: previewOutput(item, activePreset)
      }))));
      setLog((lines) => [`已导入 ${items.length} 个序列帧媒体`, ...lines].slice(0, 120));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setLastError(message);
      setLog((lines) => [`序列导入失败：${message}`, ...lines].slice(0, 120));
    } finally {
      importInProgress.current = false;
      setIsImporting(false);
    }
  }

  function saveSequenceDraft() {
    if (!sequenceDraft) return;
    const fps = Number.isFinite(sequenceDraft.sequenceFps) ? Math.max(0.001, sequenceDraft.sequenceFps) : preferences.defaultSequenceFps;
    const next = {
      ...sequenceDraft,
      width: Math.max(2, Math.round(sequenceDraft.width)),
      height: Math.max(2, Math.round(sequenceDraft.height)),
      sequenceFps: fps,
      sequencePixelAspect: Math.max(0.001, sequenceDraft.sequencePixelAspect),
      fps: `${Number(fps.toFixed(3))} fps`,
      duration: sequenceDraft.sequenceFrameCount / fps,
      output: activePreset ? previewOutput(sequenceDraft, activePreset) : sequenceDraft.output
    };
    setQueue((items) => items.map((item) => item.id === next.id ? next : item));
    setSequenceDraft(undefined);
    setNotice("序列设置已更新");
  }

  function updateActivePreset(patch: Partial<Preset>) {
    if (!activePreset) return;
    const next = normalizePreset({ ...activePreset, ...patch });
    setOutputPreset(next);
    setActivePresetId("");
    setQueue((items) => items.map((item) => item.selected && item.presetId === next.id
      ? { ...item, output: previewOutput(item, next), progress: 0, status: item.status.includes("编码") ? item.status : "就绪" }
      : item));
  }

  function choosePreset(id: string) {
    const next = presets.find((preset) => preset.id === id);
    if (!next) return;
    setActivePresetId(id);
    setOutputPreset({ ...next });
    setQueue((items) =>
      items.map((item) =>
        item.selected
          ? { ...item, presetId: id, output: previewOutput(item, next), progress: 0, status: "就绪" }
          : item
      )
    );
  }

  function createPreset() {
    const next = normalizePreset({
      ...(activePreset ?? {}),
      id: crypto.randomUUID(),
      name: "新预设",
      codec: activePreset?.codec ?? "h265",
      hardware: activePreset?.hardware ?? preferences.defaultHardware,
      outputMode: activePreset?.outputMode ?? preferences.defaultOutputMode,
      outputDir: activePreset?.outputDir ?? preferences.defaultOutputDir,
      keepTimes: activePreset?.keepTimes ?? preferences.keepTimesByDefault
    });
    setPresetDraft(next);
    setPresetEditorOpen(true);
  }

  function editPreset(preset: Preset) {
    setPresetDraft({ ...preset });
    setPresetEditorOpen(true);
  }

  async function persistPresetDraft() {
    if (!presetDraft) return;
    const name = presetDraft.name.trim();
    if (!name) {
      setNotice("请输入预设名称");
      return;
    }
    const normalized = normalizePreset({ ...presetDraft, name });
    const saved = await savePreset(normalized);
    setPresets(saved.map(normalizePreset));
    setPresetEditorOpen(false);
    setNotice("预设已保存");
  }

  async function deleteSavedPreset(preset: Preset) {
    if (!window.confirm(`删除预设“${preset.name}”？此操作不会影响当前输出设置。`)) return;
    const saved = await removePreset(preset.id);
    setPresets(saved.map(normalizePreset));
    if (activePresetId === preset.id) setActivePresetId("");
    setNotice("预设已删除");
  }

  function openContextMenu(kind: ContextMenuState["kind"], id: string, event: React.MouseEvent<HTMLElement>) {
    event.preventDefault();
    event.stopPropagation();
    if (kind === "media") {
      const item = queue.find((entry) => entry.id === id);
      if (item && !item.selected) {
        setQueue((items) => items.map((entry) => ({ ...entry, selected: entry.id === id })));
        selectionAnchorId.current = id;
      }
    }
    const anchor = event.currentTarget.getBoundingClientRect();
    const keyboardClick = event.type === "click" && event.detail === 0;
    const clientX = keyboardClick ? anchor.right : event.clientX;
    const clientY = keyboardClick ? anchor.bottom : event.clientY;
    const width = 208;
    const height = kind === "preset" ? 184 : 148;
    setContextMenu({
      kind,
      id,
      left: Math.min(Math.max(8, clientX), Math.max(8, window.innerWidth - width - 8)),
      top: Math.min(Math.max(8, clientY), Math.max(8, window.innerHeight - height - 8))
    });
  }

  function copyPreset(preset: Preset) {
    setContextMenu(undefined);
    setPresetDraft(normalizePreset({ ...preset, id: crypto.randomUUID(), name: `${preset.name} 副本` }));
    setPresetEditorOpen(true);
    setNotice("已创建预设副本，请保存");
  }

  function isItemEncoding(item: QueueItem) {
    return encodingIds.includes(item.id) || item.status.includes("编码") || item.status.includes("排队");
  }

  function resetQueueItem(item: QueueItem) {
    if (isItemEncoding(item)) return;
    const preset = presetForItem(item, presets, activePreset);
    setQueue((items) => items.map((entry) => entry.id === item.id
      ? { ...entry, status: "就绪", progress: 0, output: preset ? previewOutput(entry, preset) : entry.output }
      : entry));
    setContextMenu(undefined);
    setNotice("已重置媒体状态");
  }

  function deleteQueueItem(item: QueueItem) {
    if (isItemEncoding(item)) return;
    setQueue((items) => items.filter((entry) => entry.id !== item.id));
    setActiveJobIds((ids) => ids.filter((id) => id !== item.id));
    setMediaInfoItemId((id) => id === item.id ? "" : id);
    setContextMenu(undefined);
    setNotice(`已删除 ${item.fileName}`);
  }

  function openMediaInfo(item: QueueItem) {
    setMediaInfoItemId(item.id);
    setContextMenu(undefined);
  }

  function togglePanorama(itemId: string) {
    setQueue((items) => items.map((item) => item.id === itemId && !/\.insv$/i.test(item.source)
      ? { ...item, isPanorama: !item.isPanorama }
      : item));
  }

  /** 未标注 / 错标的 HLG、PQ 素材由用户确认后手动指定解释方式，随任务一起交给后端。 */
  function setSourceColorOverride(itemId: string, choice: SourceColorChoice) {
    setQueue((items) => items.map((item) => item.id === itemId
      ? { ...item, sourceColorOverride: choice }
      : item));
  }

  async function revealItemPath(path: string) {
    if (!path) return;
    try {
      await revealPath(path);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setLastError(message);
      setNotice("无法打开文件夹");
    }
  }

  async function chooseExternalAudio(itemId: string) {
    const paths = await pickAudioFiles();
    const audio = paths[0];
    if (!audio) return;
    setQueue((items) => items.map((item) => item.id === itemId
      ? { ...item, externalAudio: audio, status: item.status.includes("编码") ? item.status : "就绪", progress: 0 }
      : item));
    setNotice("已添加外接音频");
  }

  function clearExternalAudio(itemId: string) {
    setQueue((items) => items.map((item) => item.id === itemId ? { ...item, externalAudio: "" } : item));
  }

  function toggleAlphaMask(itemId: string, enabled: boolean) {
    setQueue((items) => items.map((item) => item.id === itemId && item.hasAlpha && item.mediaKind !== "audio"
      ? { ...item, exportAlphaMask: enabled, alphaOutput: enabled ? item.alphaOutput : "" }
      : item));
  }

  function updateAudioVisual(itemId: string, audioVisual: QueueItem["audioVisual"]) {
    setQueue((items) => items.map((item) => item.id === itemId && item.mediaKind === "audio"
      ? { ...item, audioVisual }
      : item));
  }

  async function chooseOutputFolder() {
    const folder = await pickOutputFolder();
    if (folder) updateActivePreset({ outputDir: folder, outputMode: "single_folder" });
  }

  async function chooseLut() {
    const paths = await pickLutFiles();
    if (paths[0]) {
      updateActivePreset({ lutEnabled: true, lutName: paths[0] });
      setNotice("LUT 已加入当前预设");
    }
  }

  async function choosePresetLut() {
    const paths = await pickLutFiles();
    if (paths[0]) setPresetDraft((draft) => draft ? normalizePreset({ ...draft, lutEnabled: true, lutName: paths[0] }) : draft);
  }

  async function choosePresetOutputFolder() {
    const folder = await pickOutputFolder();
    if (folder) setPresetDraft((draft) => draft ? normalizePreset({ ...draft, outputDir: folder, outputMode: "single_folder" }) : draft);
  }

  async function chooseDefaultOutputFolder() {
    const folder = await pickOutputFolder();
    if (folder) setPreferencesDraft((draft) => ({ ...draft, defaultOutputDir: folder, defaultOutputMode: "single_folder" }));
  }

  async function commitPreferences() {
    const next = {
      ...preferencesDraft,
      defaultSequenceFps: Number.isFinite(preferencesDraft.defaultSequenceFps) && preferencesDraft.defaultSequenceFps > 0
        ? preferencesDraft.defaultSequenceFps
        : defaultPreferences.defaultSequenceFps
    };
    try {
      await savePreferences(next);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setLastError(message);
      setNotice("应用设置保存失败");
      return;
    }
    setPreferences(next);
    getToolStatus().then(setToolStatus).catch(() => {});
    setPreferencesDraft(next);
    setSettingsOpen(false);
    setNotice("应用设置已保存");
  }

  function applyPreferenceDefaults() {
    updateActivePreset({
      hardware: preferencesDraft.defaultHardware,
      outputMode: preferencesDraft.defaultOutputMode,
      outputDir: preferencesDraft.defaultOutputDir,
      keepTimes: preferencesDraft.keepTimesByDefault
    });
    setNotice("默认设置已应用到当前任务");
  }

  function clearQueue() {
    if (preferences.confirmBeforeClear && !window.confirm("清空当前媒体列表？")) return;
    setQueue([]);
    setMediaInfoItemId("");
  }

  async function encodeSelected() {
    const jobs: EncodeJob[] = selectedItems
      .map((item) => ({ item, preset: presetForItem(item, presets, activePreset) }))
      .filter((job): job is EncodeJob => Boolean(job.preset));
    if (!jobs.length || !toolStatus.ok) return;

    if (preferences.autoOpenDetails) setDetailsOpen(true);
    setLastError("");
    setEncodingIds(jobs.map((job) => job.item.id));
    setActiveJobIds(jobs.map((job) => job.item.id));
    setEncodeStartedAt(Date.now());
    setClockNow(Date.now());
    setQueue((items) =>
      items.map((item) => (item.selected
        ? { ...item, status: "排队中", progress: 0, alphaOutput: "" }
        : item))
    );
    try {
      // Start serially so a switch changed during preflight cannot launch stale parallel work.
      const session = await startEncode(jobs, false);
      activeSessionRef.current = session;
      if (adaptiveConcurrencyRef.current) syncAdaptiveConcurrency(session);
      setActiveSessionId(session);
      setLog((lines) => [`编码会话已启动 · ${session}`, ...lines].slice(0, 120));
      if (!isTauriRuntime) simulateEncode(jobs.map((job) => job.item.id));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setLastError(message);
      setEncodingIds([]);
      setActiveJobIds([]);
      setEncodeStartedAt(null);
      setQueue((items) => items.map((item) => (item.selected ? { ...item, status: "失败" } : item)));
    }
  }

  function simulateEncode(ids: string[]) {
    const progressById = new Map(ids.map((id) => [id, 0]));
    simulationTimer.current = window.setInterval(() => {
      const running = ids.filter((id) => (progressById.get(id) ?? 0) > 0 && (progressById.get(id) ?? 0) < 100);
      const limit = adaptiveConcurrencyRef.current ? Math.max(1, Math.min(4, Math.floor((navigator.hardwareConcurrency || 1) / 4))) : 1;
      const selected = [...running];
      for (const id of ids) {
        if (selected.length >= limit) break;
        if (progressById.get(id) === 0) selected.push(id);
      }
      selected.forEach((id) => progressById.set(id, Math.min(100, (progressById.get(id) ?? 0) + 8)));
      setQueue((items) =>
        items.map((item) =>
          selected.includes(item.id)
            ? {
                ...item,
                progress: progressById.get(item.id) ?? 0,
                status: (progressById.get(item.id) ?? 0) >= 100
                  ? activePreset?.keepTimes ? "完成 · 已保留原始时间" : "完成"
                  : `编码中 ${progressById.get(item.id)}%`,
                alphaOutput: (progressById.get(item.id) ?? 0) >= 100 && item.exportAlphaMask && item.hasAlpha
                  ? previewAlphaOutput(item.output || previewOutput(item, activePreset ?? normalizePreset({})))
                  : item.alphaOutput
              }
            : item
        )
      );
      setEncodingIds(ids.filter((id) => (progressById.get(id) ?? 0) < 100));
      if (ids.every((id) => progressById.get(id) === 100)) {
        if (simulationTimer.current !== null) window.clearInterval(simulationTimer.current);
        simulationTimer.current = null;
        setEncodingIds([]);
        setLog((lines) => [`全部 ${ids.length} 个视频处理完成`, ...lines].slice(0, 120));
      }
    }, 260);
  }

  function syncAdaptiveConcurrency(session: string) {
    // Serialize updates so rapid clicks cannot leave the backend in an older state.
    adaptiveUpdateRef.current = adaptiveUpdateRef.current.then(async () => {
      if (activeSessionRef.current !== session) return;
      await setAdaptiveConcurrency(session, adaptiveConcurrencyRef.current);
    }).catch((error) => {
      setLog((lines) => [`并发设置更新：${String(error)}`, ...lines].slice(0, 120));
    });
  }

  function toggleAdaptiveConcurrency() {
    const enabled = !adaptiveConcurrencyRef.current;
    adaptiveConcurrencyRef.current = enabled;
    setAdaptiveConcurrencyState(enabled);
    if (activeSessionRef.current) syncAdaptiveConcurrency(activeSessionRef.current);
  }

  async function cancelCurrentEncoding() {
    if (!isEncoding) return;
    if (!isTauriRuntime) {
      if (simulationTimer.current !== null) window.clearInterval(simulationTimer.current);
      simulationTimer.current = null;
      setQueue((items) => items.map((item) => encodingIds.includes(item.id) ? { ...item, status: "已取消", progress: 0 } : item));
      setEncodingIds([]);
      setLog((lines) => ["编码任务已取消", ...lines].slice(0, 120));
      return;
    }
    if (!activeSessionId) return;
    const pending = new Set(encodingIds);
    try {
      await cancelEncode(activeSessionId);
      pending.forEach((id) => cancellingRef.current.add(id));
      setQueue((items) => items.map((item) => pending.has(item.id) ? { ...item, status: "正在取消" } : item));
      setLog((lines) => ["正在取消编码任务…", ...lines].slice(0, 120));
      // 兜底：若后端事件迟迟未到，确保任务最终进入“已取消”终态
      window.setTimeout(() => {
        setQueue((items) => items.map((item) => cancellingRef.current.has(item.id) ? { ...item, status: "已取消", progress: 0 } : item));
        pending.forEach((id) => cancellingRef.current.delete(id));
        setEncodingIds((ids) => ids.filter((id) => !pending.has(id)));
      }, 45000);
    } catch (error) {
      setLastError(error instanceof Error ? error.message : String(error));
    }
  }

  function updateItemSelection(itemId: string, selected: boolean, shiftKey: boolean) {
    const visibleIds = visibleItems.map((item) => item.id);
    const anchorIndex = visibleIds.indexOf(selectionAnchorId.current);
    const itemIndex = visibleIds.indexOf(itemId);
    if (shiftKey && anchorIndex >= 0 && itemIndex >= 0) {
      const first = Math.min(anchorIndex, itemIndex);
      const last = Math.max(anchorIndex, itemIndex);
      const range = new Set(visibleIds.slice(first, last + 1));
      setQueue((items) => items.map((item) => range.has(item.id) ? { ...item, selected } : item));
    } else {
      setQueue((items) => items.map((item) => item.id === itemId ? { ...item, selected } : item));
    }
    selectionAnchorId.current = itemId;
  }

  function handleRowClick(event: React.MouseEvent<HTMLElement>, itemId: string) {
    if (suppressRowClick.current) return;
    if ((event.target as HTMLElement).closest("button, input, label, a")) return;
    if (event.shiftKey) {
      updateItemSelection(itemId, true, true);
    } else if (event.ctrlKey || event.metaKey) {
      const item = queue.find((entry) => entry.id === itemId);
      updateItemSelection(itemId, !(item?.selected ?? false), false);
    } else {
      setQueue((items) => items.map((item) => ({ ...item, selected: item.id === itemId })));
      selectionAnchorId.current = itemId;
    }
  }

  function beginMarquee(event: React.PointerEvent<HTMLDivElement>) {
    if (event.button !== 0 || (event.target as HTMLElement).closest("button, input, label, a")) return;
    const body = queueBodyRef.current;
    if (!body) return;
    const bounds = body.getBoundingClientRect();
    marqueeStart.current = {
      x: event.clientX - bounds.left + body.scrollLeft,
      y: event.clientY - bounds.top + body.scrollTop,
      clientX: event.clientX,
      clientY: event.clientY,
      pointerId: event.pointerId,
      additive: event.ctrlKey || event.metaKey,
      initialSelected: new Set(queue.filter((item) => item.selected).map((item) => item.id)),
      dragging: false
    };
    body.setPointerCapture(event.pointerId);
  }

  function moveMarquee(event: React.PointerEvent<HTMLDivElement>) {
    const start = marqueeStart.current;
    const body = queueBodyRef.current;
    if (!start || !body || start.pointerId !== event.pointerId) return;
    if (!start.dragging && Math.hypot(event.clientX - start.clientX, event.clientY - start.clientY) < 5) return;
    start.dragging = true;
    event.preventDefault();
    const bounds = body.getBoundingClientRect();
    const currentX = event.clientX - bounds.left + body.scrollLeft;
    const currentY = event.clientY - bounds.top + body.scrollTop;
    const clientRect = {
      left: Math.min(start.clientX, event.clientX),
      right: Math.max(start.clientX, event.clientX),
      top: Math.min(start.clientY, event.clientY),
      bottom: Math.max(start.clientY, event.clientY)
    };
    const hitIds = new Set(
      Array.from(body.querySelectorAll<HTMLElement>("[data-item-id]"))
        .filter((row) => intersects(clientRect, row.getBoundingClientRect()))
        .map((row) => row.dataset.itemId!)
    );
    const visibleIds = new Set(visibleItems.map((item) => item.id));
    setQueue((items) => items.map((item) => {
      if (!visibleIds.has(item.id)) return item;
      const selected = hitIds.has(item.id) || (start.additive && start.initialSelected.has(item.id));
      return item.selected === selected ? item : { ...item, selected };
    }));
    setMarquee({
      left: Math.min(start.x, currentX),
      top: Math.min(start.y, currentY),
      width: Math.abs(currentX - start.x),
      height: Math.abs(currentY - start.y)
    });
  }

  function endMarquee(event: React.PointerEvent<HTMLDivElement>) {
    const start = marqueeStart.current;
    const body = queueBodyRef.current;
    if (!start || start.pointerId !== event.pointerId) return;
    if (body?.hasPointerCapture(event.pointerId)) body.releasePointerCapture(event.pointerId);
    if (start.dragging) {
      suppressRowClick.current = true;
      window.setTimeout(() => { suppressRowClick.current = false; }, 0);
    }
    marqueeStart.current = null;
    setMarquee(null);
  }

  const hardwareSummary = hardwareLabel(activePreset?.hardware ?? "auto", platform);
  const filteredPresets = presets.filter((preset) =>
    `${preset.name} ${codecLabels[preset.codec]} ${presetSummary(preset)}`.toLowerCase().includes(presetFilter.trim().toLowerCase())
  );

  const presetGroups = [
    { id: "common", label: "日常压缩" },
    { id: "hdr", label: "HDR 与色彩" },
    { id: "editing", label: "后期编辑" }
  ].map((group) => ({ ...group, items: filteredPresets.filter((preset) =>
    (preset.codec === "prores" ? "editing" : ["hlg", "hdr10", "dolby_vision"].includes(preset.hdrMode) ? "hdr" : "common") === group.id
  ) })).filter((group) => group.items.length);

  return (
    <main className="composer-shell" onDragOver={(event) => event.preventDefault()} onDrop={handleBrowserDrop(addPaths)}>
      <header className="titlebar">
        <div className="brand-lockup">
          <img className="brand-mark" src={appIcon} alt="" aria-hidden="true" />
          <strong>VideoSize Composer <span className="brand-version">{functionVersion}</span></strong>
        </div>
        <div className="titlebar-actions">
          <button className={`adaptive-toggle ${adaptiveConcurrency ? "is-on" : ""}`} role="switch" aria-checked={adaptiveConcurrency} aria-label="自适应并发" title={adaptiveConcurrency ? "自适应并发已开启，按硬件资源调度" : "自适应并发已关闭，任务依次执行"} onClick={toggleAdaptiveConcurrency}><span>自适应并发</span><span className="adaptive-toggle-track" aria-hidden="true"><span /></span></button>
          <button className="text-icon-button" onClick={() => { setPreferencesDraft(preferences); setSettingsOpen(true); }}><Settings size={17} />设置</button>
          <button className="text-icon-button" onClick={() => setHelpOpen(true)}><HelpCircle size={17} />帮助</button>
        </div>
      </header>

      <section className={`workspace-grid ${presetRailCollapsed ? "preset-rail-is-collapsed" : ""}`}>
        <aside className={`preset-rail ${presetRailCollapsed ? "is-collapsed" : ""}`}>
          <div className="rail-heading">
            {!presetRailCollapsed && <><h2>预设</h2><button className="quiet-action" onClick={createPreset}><Plus size={16} />新建</button></>}
            <button className="icon-only rail-collapse" aria-label={presetRailCollapsed ? "展开预设抽屉" : "折叠预设抽屉"} title={presetRailCollapsed ? "展开预设抽屉" : "折叠预设抽屉"} aria-expanded={!presetRailCollapsed} onClick={togglePresetRail}>{presetRailCollapsed ? <PanelLeftOpen size={17} /> : <PanelLeftClose size={17} />}</button>
          </div>
          {!presetRailCollapsed && <>
            <SearchField value={presetFilter} onChange={setPresetFilter} placeholder="搜索我的预设" />

            <button className="preset-section-title" aria-expanded={presetListOpen} onClick={togglePresetList}><span>我的预设 <small>{filteredPresets.length}</small></span><ChevronDown size={15} className={presetListOpen ? "rotated" : ""} /></button>
            {presetListOpen && <div className="preset-list">
              {presetGroups.map((group) => <section className="preset-group" key={group.id} aria-label={group.label}>
                <h3>{group.label}<small>{group.items.length}</small></h3>
                {group.items.map((preset) => (
                  <div key={preset.id} className={`preset-item ${preset.id === activePresetId ? "is-active" : ""}`} onContextMenu={(event) => openContextMenu("preset", preset.id, event)}>
                    <button className="preset-apply" title={`${preset.name}\n${codecLabels[preset.codec]} · ${presetSummary(preset)}`} aria-pressed={preset.id === activePresetId} onClick={() => choosePreset(preset.id)}>
                      <span className="preset-copy"><strong>{preset.name}</strong><small>{presetSummary(preset)}</small></span>
                    </button>
                    <button className="preset-more icon-only" aria-label={`操作预设 ${preset.name}`} title="预设操作" aria-haspopup="menu" aria-expanded={contextMenu?.kind === "preset" && contextMenu.id === preset.id} onClick={(event) => openContextMenu("preset", preset.id, event)}><MoreHorizontal size={15} /></button>
                  </div>
                ))}
              </section>)}
              {!filteredPresets.length && <span className="no-presets">没有匹配的预设，可点击“新建”创建。</span>}
            </div>}

          </>}
        </aside>

        <section className="main-workbench">
          <section className={`forecast-panel ${isEncoding ? "is-encoding" : ""}`} aria-label="压缩结果预测">
            <div className="forecast-size">
              <span className="eyebrow">预计效果 · 选中 {selectedItems.length} 个文件</span>
              <div><strong>{formatBytes(totalSourceSize)}</strong><span className="forecast-arrow">→</span><strong className="accent">{formatBytes(totalEstimatedSize)}</strong></div>
              <small>预计减少 {savings}% · 根据当前预设估算</small>
            </div>
            {isEncoding && <div className="forecast-metric encode-forecast"><Clock3 size={18} /><span><strong>{remainingTimeLabel}</strong><small>根据当前编码速度估算</small></span></div>}
          </section>

          <section className="queue-panel">
            <header className="queue-toolbar">
              <div className="queue-actions">
                <div className="add-media-control">
                  <button className="secondary-button" disabled={isImporting} onClick={() => setAddMenuOpen((open) => !open)}>{isImporting ? <LoaderCircle className="loading-spinner" size={17} /> : <Plus size={17} />}{isImporting ? "载入中…" : "添加媒体"}<ChevronDown size={14} /></button>
                  {addMenuOpen && <div className="add-media-menu">
                    <button onClick={() => { setAddMenuOpen(false); importVideos(pickVideoFiles); }}><Film size={17} /><span><strong>视频文件</strong><small>选择一个或多个视频媒体</small></span></button>
                    <button onClick={() => { setAddMenuOpen(false); importVideos(pickAudioFiles); }}><Zap size={17} /><span><strong>音频文件</strong><small>WAV、MP3、FLAC、AAC、M4A 等</small></span></button>
                    <button onClick={() => { setAddMenuOpen(false); setSequenceImportOpen(true); }}><Images size={17} /><span><strong>序列帧媒体</strong><small>选择文件夹或序列中的一帧</small></span></button>
                  </div>}
                </div>
                <button className="icon-only" aria-label="添加文件夹" title="添加文件夹" disabled={isImporting} onClick={() => importVideos(pickFolder)}><Folder size={17} /></button>
                <button className="quiet-action danger" disabled={!queue.length || isImporting} onClick={clearQueue}><Trash2 size={16} />清空列表</button>
              </div>
              <SearchField value={filter} onChange={setFilter} placeholder="搜索文件" compact />
              <div className="queue-summary">共 {queue.length} 个文件 · {formatBytes(queue.reduce((sum, item) => sum + item.sizeBytes, 0))}</div>
            </header>

            <div className="queue-head" aria-hidden="true"><span /><span>文件名</span><span>源文件信息</span><span>预计输出</span><span>状态</span><span /></div>
            <div
              className={`queue-body ${marquee ? "is-marquee-selecting" : ""} ${isImporting ? "is-importing" : ""}`}
              ref={queueBodyRef}
              aria-busy={isImporting}
              onPointerDown={beginMarquee}
              onPointerMove={moveMarquee}
              onPointerUp={endMarquee}
              onPointerCancel={endMarquee}
            >
              {marquee && <span className="selection-marquee" style={marquee} aria-hidden="true" />}
              {visibleItems.length ? visibleItems.map((item, index) => {
                const itemPreset = presetForItem(item, presets, activePreset);
                const presetName = presetNameForItem(item, presets, activePreset, activePresetId);
                const predicted = estimateOutputBytes(item, itemPreset);
                const outputPath = item.output || (itemPreset ? previewOutput(item, itemPreset) : "");
                return (
                  <article
                    className={`media-row ${item.selected ? "is-selected" : ""}`}
                    key={item.id}
                    data-item-id={item.id}
                    onClick={(event) => handleRowClick(event, item.id)}
                    onContextMenu={(event) => openContextMenu("media", item.id, event)}
                  >
                    <label className="row-check" title="按住 Shift 可连续选择" onClick={(event) => { event.preventDefault(); event.stopPropagation(); updateItemSelection(item.id, !item.selected, event.shiftKey); }}><input type="checkbox" checked={item.selected} readOnly /><span><Check size={12} /></span><em>{index + 1}</em></label>
                    <div className="media-identity">{item.thumbnail ? <img src={item.thumbnail} alt={`${item.fileName} 媒体缩略图`} draggable={false} /> : <span className="video-thumb-fallback">{item.mediaKind === "sequence" ? <Images size={19} /> : item.mediaKind === "audio" ? <Zap size={19} /> : <Film size={19} />}</span>}<span className="media-copy"><strong>{item.fileName}</strong><small>{item.mediaKind === "sequence" ? `${item.sequenceFrameCount} 帧 · ${formatDuration(item.duration)}` : item.mediaKind === "audio" ? `音频 · ${formatDuration(item.duration)}` : formatDuration(item.duration)}</small>{presetName && <span className="media-preset" title={`使用预设：${presetName}`}>预设：{presetName}</span>}<button className="path-link source-path" title={item.source} onClick={(event) => { event.stopPropagation(); revealItemPath(item.source); }}>{shortPath(item.source)}</button>{item.mediaKind === "sequence" && <button className="sequence-settings-button" onClick={(event) => { event.stopPropagation(); setSequenceDraft({ ...item }); }}><SlidersHorizontal size={12} />序列设置</button>}{item.mediaKind === "audio" && <label className="row-inline-control" title="音频素材输出视频时使用的画面模式" onClick={(event) => event.stopPropagation()}><span>画面</span><select value={item.audioVisual} onChange={(event) => updateAudioVisual(item.id, event.target.value as QueueItem["audioVisual"])}><option value="timecode">时间码</option><option value="black">纯黑</option><option value="white">纯白</option></select></label>}{item.mediaKind !== "audio" && item.audioTracks === 0 && <button className="audio-attach-control" title={item.externalAudio || "添加外接音频"} onClick={(event) => { event.stopPropagation(); chooseExternalAudio(item.id); }}>{item.externalAudio ? `已添加音频 · ${shortPath(item.externalAudio)}` : "无音轨 · 添加音频"}</button>}{item.externalAudio && item.mediaKind !== "audio" && <button className="audio-clear-control" aria-label="清除外接音频" title="清除外接音频" onClick={(event) => { event.stopPropagation(); clearExternalAudio(item.id); }}><X size={11} /></button>}</span></div>
                    <div className="source-spec"><span>{item.width}×{item.height} · {item.fps}{item.mediaKind === "sequence" && item.sequencePixelAspect !== 1 ? ` · 像素 ${item.sequencePixelAspect.toFixed(2)}x` : ""}</span><span>{item.codec} · {item.mediaKind === "audio" ? "纯音频" : `${item.bitDepth || 8}-bit · 4:2:${item.chroma.slice(-1)}`} · {formatBytes(item.sizeBytes)}</span><span className="media-badges">{item.hasAlpha && <em>Alpha</em>}{item.exportAlphaMask && <em>Alpha 遮罩已启用</em>}{item.mediaKind === "audio" && <em>音频素材</em>}{item.mediaKind === "sequence" && <em>序列帧</em>}{item.hdrMode !== "sdr" && <em>{hdrLabel(item.hdrMode)}</em>}{isSuspectHdr(item) && <em className="suspect" title="源没有写色彩元数据，但宽色域 + 10-bit 提示它可能是 HLG / HDR。打开源信息确认后再按 HDR 编码。">HDR（未标注）</em>}{item.panoramaTagged && <em>自动识别</em>}{item.audioTracks > 1 && <em>{item.audioTracks} 音轨</em>}{item.subtitleTracks > 0 && <em>{item.subtitleTracks} 字幕</em>}{item.mediaKind !== "audio" && <button className={`panorama-toggle ${item.isPanorama ? "is-on" : ""}`} disabled={/\.insv$/i.test(item.source)} title={/\.insv$/i.test(item.source) ? "INSV 原片必须先进行全景拼接" : "点击切换全景状态"} onClick={(event) => { event.stopPropagation(); togglePanorama(item.id); }}>{item.isPanorama ? "全景视频" : "非全景视频"}</button>}</span>{item.hasAlpha && item.mediaKind !== "audio" && <label className="alpha-mask-control" title="从源 Alpha 通道生成独立黑白视频" onClick={(event) => event.stopPropagation()}><input type="checkbox" checked={item.exportAlphaMask} onChange={(event) => toggleAlphaMask(item.id, event.target.checked)} /><span>单独导出 Alpha 遮罩</span></label>}</div>
                    <div className="output-spec"><strong>{formatBytes(predicted)}</strong><span>{itemPreset ? codecLabels[itemPreset.codec] : "未选择预设"}</span><span>{targetResolution(item, itemPreset)}</span></div>
                    <StatusCell item={item} outputPath={outputPath} onRevealPath={revealItemPath} />
                    <button className="row-disclosure" aria-label={`查看 ${item.fileName} 详情`} onClick={(event) => { event.stopPropagation(); openMediaInfo(item); }}><ChevronRight size={19} /></button>
                  </article>
                );
              }) : (
                <div className="empty-state"><FilePlus2 size={28} /><strong>添加媒体开始压缩</strong><span>支持视频、音频文件与序列帧媒体</span><button className="secondary-button" disabled={isImporting} onClick={() => setAddMenuOpen(true)}>选择媒体</button></div>
              )}
            </div>
            {isImporting && <div className="import-loading" role="status" aria-live="polite"><LoaderCircle className="loading-spinner" size={28} /><strong>正在载入视频…</strong><span>正在读取媒体信息，请稍候</span></div>}
            <footer className="queue-footer">
              <label className="select-all" title="支持 Shift 连选、Ctrl/Cmd 多选和拖拽框选"><input type="checkbox" checked={queue.length > 0 && selectedItems.length === queue.length} onChange={(event) => setQueue((items) => items.map((item) => ({ ...item, selected: event.target.checked })))} />已选择 {selectedItems.length} / {queue.length} 个文件</label>
              <div><span>总大小：{formatBytes(totalSourceSize)}</span><strong>预计输出：{formatBytes(totalEstimatedSize)} (-{savings}%)</strong></div>
            </footer>
          </section>

        </section>

        <aside className="settings-panel" aria-label="当前任务输出设置">
          <div className="settings-heading"><span><h2>输出设置</h2><small>用于当前选中的媒体</small></span><button className="quiet-action" onClick={createPreset}><Plus size={15} />存为预设</button></div>
          <div className="current-preset">
            <Setting label="应用预设"><select value={activePresetId} onChange={(event) => choosePreset(event.target.value)}>{!activePresetId && <option value="">自定义设置</option>}{presets.map((preset) => <option key={preset.id} value={preset.id}>{preset.name}</option>)}</select></Setting>
            {activePreset && <small className="current-output-summary" title={presetSummary(activePreset)}>{presetSummary(activePreset)}</small>}
          </div>
          {activePreset && <OutputSettings preset={activePreset} platform={platform} onChange={updateActivePreset} onChooseLut={chooseLut} onChooseOutput={chooseOutputFolder} />}
          <div className="settings-footer">
            <button className="folder-summary" title={outputDirectory(activePreset)} onClick={chooseOutputFolder}><span><small>输出位置</small><strong>{shortPath(outputDirectory(activePreset))}</strong></span><FolderOpen size={17} /></button>
            <div className="space-check"><ShieldCheck size={14} /><span>{activePreset?.keepTimes ? "保留原始时间 · " : ""}自动检查空间与重名</span></div>
          </div>
        </aside>
      </section>

      <footer className="action-bar">
        <div className="action-summary">
          {isEncoding ? <div className="overall-progress" aria-label={`总进度 ${overallProgress}%`}>
            <span><strong>总进度 {overallProgress}%</strong><small>预计剩余 {remainingTimeLabel}</small></span>
            <span className="overall-progress-track"><span style={{ width: `${overallProgress}%` }} /></span>
          </div> : <><span>{selectedItems.length} 个文件</span><span>{formatBytes(totalSourceSize)} → <strong>{formatBytes(totalEstimatedSize)}</strong></span></>}
          <span className="hardware-pill"><Zap size={14} />{hardwareSummary}</span>
        </div>
        <div className="details-control" ref={detailsControlRef}>
          <button className="review-button" ref={detailsButtonRef} aria-haspopup="dialog" aria-expanded={detailsOpen} aria-controls={detailsOpen ? "task-details" : undefined} onClick={() => setDetailsOpen((open) => !open)}>查看详情<ChevronDown size={16} /></button>
          {detailsOpen && <section id="task-details" className="details-popover" role="dialog" aria-modal="false" aria-labelledby="task-details-title">
            <header><h2 id="task-details-title">{isEncoding ? "正在压缩" : completedCount ? "最近一次任务" : "任务详情"}</h2><button className="icon-only" aria-label="关闭任务详情" onClick={() => { setDetailsOpen(false); detailsButtonRef.current?.focus(); }}><X size={16} /></button></header>
            <div className="details-counts" aria-live="polite"><span>完成 <strong>{completedCount}</strong></span><span>失败 <strong>{failedCount}</strong></span><span>总计 <strong>{queue.length}</strong></span></div>
            {lastError && <p className="details-error">{lastError}</p>}
            <h3>运行日志</h3>
            <ul className="details-log" aria-label="运行日志">{log.map((line, index) => <li key={`${index}-${line}`}>{line}</li>)}</ul>
          </section>}
        </div>
        {isEncoding && <button className="cancel-button" onClick={cancelCurrentEncoding}><Square size={15} />取消任务</button>}
        <button className="encode-button" disabled={isEncoding || !selectedItems.length || !toolStatus.ok} onClick={encodeSelected}><Play size={20} />{isEncoding ? "正在压缩" : completedCount ? "再次压缩" : "开始压缩"}</button>
      </footer>

      {presetEditorOpen && presetDraft && <PresetEditor
        draft={presetDraft}
        platform={platform}
        onChange={(patch) => setPresetDraft((draft) => draft ? normalizePreset({ ...draft, ...patch }) : draft)}
        onChooseLut={choosePresetLut}
        onChooseOutput={choosePresetOutputFolder}
        onClose={() => setPresetEditorOpen(false)}
        onSave={persistPresetDraft}
      />}

      {contextMenu && (
        <div
          className="context-menu"
          role="menu"
          aria-label={contextMenu.kind === "preset" ? "预设操作" : "媒体操作"}
          style={{ left: contextMenu.left, top: contextMenu.top }}
          onPointerDown={(event) => event.stopPropagation()}
          onContextMenu={(event) => event.preventDefault()}
        >
          {contextMenu.kind === "preset" ? (() => {
            const preset = presets.find((entry) => entry.id === contextMenu.id);
            if (!preset) return null;
            return <>
              <button role="menuitem" onClick={() => { choosePreset(preset.id); setContextMenu(undefined); }}>应用</button>
              <button role="menuitem" onClick={() => { editPreset(preset); setContextMenu(undefined); }}>编辑</button>
              <button role="menuitem" onClick={() => copyPreset(preset)}>复制</button>
              <button role="menuitem" className="danger" onClick={() => { setContextMenu(undefined); deleteSavedPreset(preset); }}>删除</button>
            </>;
          })() : (() => {
            const item = queue.find((entry) => entry.id === contextMenu.id);
            if (!item) return null;
            const disabled = isItemEncoding(item);
            return <>
              <button role="menuitem" onClick={() => openMediaInfo(item)}>查看源信息</button>
              <button role="menuitem" disabled={disabled} onClick={() => resetQueueItem(item)}>重置状态</button>
              <button role="menuitem" className="danger" disabled={disabled} onClick={() => deleteQueueItem(item)}>删除</button>
            </>;
          })()}
        </div>
      )}

      {settingsOpen && <Modal title="应用设置" onClose={() => setSettingsOpen(false)} footer={<><button className="secondary-button" onClick={() => setPreferencesDraft(defaultPreferences)}>恢复默认</button><button className="primary-button" onClick={commitPreferences}>保存设置</button></>}>
        <ThemeSettings colors={preferencesDraft.theme} onChange={(theme) => setPreferencesDraft((draft) => ({ ...draft, theme }))} />
        <div className="modal-grid two-columns">
          <Setting label="默认输出策略"><select value={preferencesDraft.defaultOutputMode} onChange={(event) => setPreferencesDraft((draft) => ({ ...draft, defaultOutputMode: event.target.value as Preset["outputMode"] }))}><option value="subfolder">原位子文件夹</option><option value="in_place">原位导出</option><option value="single_folder">全部到同一目录</option></select></Setting>
          <Setting label="序列帧默认帧率"><input type="number" min={0.001} step={0.001} value={preferencesDraft.defaultSequenceFps} onChange={(event) => setPreferencesDraft((draft) => ({ ...draft, defaultSequenceFps: Number(event.target.value) }))} /></Setting>
        </div>
        {preferencesDraft.defaultOutputMode === "single_folder" && <button className="secondary-button full" onClick={chooseDefaultOutputFolder}><FolderOpen size={15} />{preferencesDraft.defaultOutputDir ? shortPath(preferencesDraft.defaultOutputDir) : "选择默认输出目录"}</button>}
        <div className="modal-section hardware-settings-section">
          <h3>硬件与性能</h3>
          <Setting label="默认编码设备"><select value={preferencesDraft.defaultHardware} onChange={(event) => setPreferencesDraft((draft) => ({ ...draft, defaultHardware: event.target.value as Hardware }))}>{platform.accelerators.map((hardware) => <option key={hardware} value={hardware}>{hardware === "auto" ? "自动选择" : hardware.toUpperCase()}</option>)}</select></Setting>
          <label className="hardware-toggle-row settings-hardware-toggle"><span>启用硬件加速</span><input type="checkbox" checked={preferencesDraft.defaultHardware !== "cpu"} onChange={(event) => setPreferencesDraft((draft) => ({ ...draft, defaultHardware: event.target.checked ? (platform.accelerators.includes("cuda") ? "cuda" : platform.accelerators.includes("metal") ? "metal" : "auto") : "cpu" }))} /><span className={`switch-visual ${preferencesDraft.defaultHardware !== "cpu" ? "on" : ""}`}><span /></span></label>
          <div className="hardware-device-list"><span className="field-note">可用设备</span>{platform.accelerators.filter((item) => item !== "auto").map((accelerator) => <button className={`hardware-row ${preferencesDraft.defaultHardware === accelerator ? "selected" : ""}`} key={accelerator} onClick={() => setPreferencesDraft((draft) => ({ ...draft, defaultHardware: accelerator }))}><span className={`status-dot ${accelerator === "cpu" ? "blue" : ""}`} /><strong>{accelerator === "cuda" ? "CUDA" : accelerator === "metal" ? "Metal" : "CPU"}</strong><small>{accelerator === "cuda" ? "NVIDIA GPU" : accelerator === "metal" ? "Apple GPU" : "自动回退"}</small></button>)}</div>
          <div className="environment-card"><strong>编码环境</strong><span>{toolStatus.ok ? "FFmpeg 与 FFprobe 可用" : "编码工具不可用"}</span><small>{toolStatus.ffmpeg}</small><small>{toolStatus.ffprobe}</small><small>{toolStatus.encoders.length ? `可用编码器：${toolStatus.encoders.join("、")}` : "未检测到硬件编码器"}</small></div>
        </div>
        <div className="modal-section">
          <h3>Insta360 全景处理</h3>
          <Setting label="Desktop Media SDK 目录"><input value={preferencesDraft.insta360SdkPath} placeholder="留空时自动检测程序旁的 insta360/" onChange={(event) => setPreferencesDraft((draft) => ({ ...draft, insta360SdkPath: event.target.value }))} /></Setting>
          <button className="secondary-button full" onClick={async () => { const paths = await pickFolder(); if (paths[0]) setPreferencesDraft((draft) => ({ ...draft, insta360SdkPath: paths[0] })); }}><FolderOpen size={15} />选择 SDK 目录</button>
          <small className="field-note">{toolStatus.insta360Sdk ? `已找到程序（编码前检查参数与模型）：${toolStatus.insta360Sdk}` : "未找到 MediaSDKTest。INSV 可以导入，拼接导出需先配置官方 SDK。"}</small>
        </div>
        <div className="preference-list">
          <label className="inline-check"><input type="checkbox" checked={preferencesDraft.keepTimesByDefault} onChange={(event) => setPreferencesDraft((draft) => ({ ...draft, keepTimesByDefault: event.target.checked }))} /><span>新任务默认保留创建日期与修改时间</span></label>
          <label className="inline-check"><input type="checkbox" checked={preferencesDraft.confirmBeforeClear} onChange={(event) => setPreferencesDraft((draft) => ({ ...draft, confirmBeforeClear: event.target.checked }))} /><span>清空媒体列表前要求确认</span></label>
          <label className="inline-check"><input type="checkbox" checked={preferencesDraft.autoOpenDetails} onChange={(event) => setPreferencesDraft((draft) => ({ ...draft, autoOpenDetails: event.target.checked }))} /><span>开始编码时自动展开任务详情</span></label>
        </div>
        <div className="version-card" aria-label={`软件版本 ${displayVersion}`}><span><strong>软件版本</strong><small>功能版本 {functionVersion}</small></span><code>{displayVersion}</code></div>
        <button className="secondary-button full" onClick={applyPreferenceDefaults}>将这些默认值应用到当前任务</button>
      </Modal>}

      {mediaInfoItemId && (() => {
        const item = queue.find((entry) => entry.id === mediaInfoItemId);
        if (!item) return null;
        const itemPreset = presetForItem(item, presets, activePreset);
        return <MediaInfoModal item={item} presetName={presetNameForItem(item, presets, activePreset, activePresetId)} outputPath={item.output || (itemPreset ? previewOutput(item, itemPreset) : "")} onClose={() => setMediaInfoItemId("")} onTogglePanorama={() => togglePanorama(item.id)} onSourceColorChange={(choice) => setSourceColorOverride(item.id, choice)} onRevealSource={() => revealItemPath(item.source)} onRevealOutput={(path) => revealItemPath(path)} />;
      })()}

      {helpOpen && <Modal title="使用帮助" onClose={() => setHelpOpen(false)} footer={<button className="primary-button" onClick={() => setHelpOpen(false)}>知道了</button>}>
        <div className="help-steps">
          <div><span>1</span><p><strong>添加媒体</strong><small>可选择视频、导入文件夹，或把文件直接拖入队列。</small></p></div>
          <div><span>2</span><p><strong>设置当前任务</strong><small>右侧参数只影响当前选中文件；需要长期复用时，再“另存为预设”。</small></p></div>
          <div><span>3</span><p><strong>管理我的预设</strong><small>左侧预设可应用、独立编辑和删除，整个抽屉也可以折叠。</small></p></div>
          <div><span>4</span><p><strong>开始压缩</strong><small>任务启动时检查输出目录与可用空间，输出重名时自动编号。</small></p></div>
        </div>
        <div className="help-note"><ShieldCheck size={17} /><span>选择“原视频参数”时，会针对队列中的每个视频分别继承其色深和色度采样。</span></div>
      </Modal>}

      {sequenceImportOpen && <Modal title="添加序列帧媒体" onClose={() => setSequenceImportOpen(false)} footer={<button className="secondary-button" onClick={() => setSequenceImportOpen(false)}>取消</button>}>
        <div className="sequence-source-options">
          <button onClick={() => importSequence(pickSequenceFrame)}><Images size={25} /><span><strong>选择序列中的一帧</strong><small>自动识别同文件夹内、命名规则相同的全部帧</small></span><ChevronRight size={18} /></button>
          <button onClick={() => importSequence(pickSequenceFolder)}><FolderOpen size={25} /><span><strong>选择文件夹</strong><small>扫描文件夹并载入其中识别到的全部序列</small></span><ChevronRight size={18} /></button>
        </div>
      </Modal>}

      {sequenceDraft && <Modal title="序列设置" onClose={() => setSequenceDraft(undefined)} footer={<><button className="secondary-button" onClick={() => setSequenceDraft(undefined)}>取消</button><button className="primary-button" onClick={saveSequenceDraft}>应用</button></>}>
        <div className="sequence-summary"><Images size={22} /><span><strong>{sequenceDraft.fileName}</strong><small>{sequenceDraft.sequencePattern} · {sequenceDraft.sequenceFrameCount} 帧</small></span></div>
        <div className="modal-grid two-columns">
          <Setting label="帧率"><input type="number" min={0.001} step={0.001} value={sequenceDraft.sequenceFps} onChange={(event) => setSequenceDraft((draft) => draft ? { ...draft, sequenceFps: Number(event.target.value) } : draft)} /></Setting>
          <Setting label="像素尺寸（宽幅放大率）"><input type="number" min={0.001} step={0.01} value={sequenceDraft.sequencePixelAspect} onChange={(event) => setSequenceDraft((draft) => draft ? { ...draft, sequencePixelAspect: Number(event.target.value) } : draft)} /></Setting>
        </div>
        <div className="modal-section"><h3>分辨率</h3><div className="dimension-grid modal-dimensions"><Setting label="宽"><input type="number" min={2} value={sequenceDraft.width} onChange={(event) => setSequenceDraft((draft) => draft ? { ...draft, width: Number(event.target.value) } : draft)} /></Setting><span>×</span><Setting label="高"><input type="number" min={2} value={sequenceDraft.height} onChange={(event) => setSequenceDraft((draft) => draft ? { ...draft, height: Number(event.target.value) } : draft)} /></Setting></div></div>
      </Modal>}

      {notice && <div className="toast" role="status">{notice}</div>}
      {lastError && !detailsOpen && <button className="error-banner" onClick={() => setDetailsOpen(true)}><CircleAlert size={17} />{lastError}</button>}
    </main>
  );
}

function Modal({ title, children, footer, onClose }: { title: string; children: React.ReactNode; footer: React.ReactNode; onClose: () => void }) {
  return <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
    <section className="modal-card" role="dialog" aria-modal="true" aria-label={title}>
      <header><h2>{title}</h2><button className="icon-only" aria-label="关闭" onClick={onClose}><X size={18} /></button></header>
      <div className="modal-body">{children}</div>
      <footer>{footer}</footer>
    </section>
  </div>;
}

function PresetEditor({ draft, platform, onChange, onChooseLut, onChooseOutput, onClose, onSave }: {
  draft: Preset;
  platform: PlatformInfo;
  onChange: (patch: Partial<Preset>) => void;
  onChooseLut: () => void;
  onChooseOutput: () => void;
  onClose: () => void;
  onSave: () => void;
}) {
  return <Modal title="编辑我的预设" onClose={onClose} footer={<><button className="secondary-button" onClick={onClose}>取消</button><button className="primary-button" onClick={onSave}>保存预设</button></>}>
    <div className="preset-editor-intro"><Edit3 size={18} /><span><strong>独立预设编辑器</strong><small>此处修改预设库，不会直接改变右侧当前任务的输出设置。</small></span></div>
    <Setting label="预设名称"><input autoFocus value={draft.name} onChange={(event) => onChange({ name: event.target.value })} /></Setting>
    <OutputSettings preset={draft} platform={platform} onChange={onChange} onChooseLut={onChooseLut} onChooseOutput={onChooseOutput} />
  </Modal>;
}

function SearchField({ value, onChange, placeholder, compact = false }: { value: string; onChange: (value: string) => void; placeholder: string; compact?: boolean }) {
  return <label className={`search-field ${compact ? "compact" : ""}`}><Search size={15} /><input value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} /></label>;
}

function MediaInfoModal({ item, presetName, outputPath, onClose, onTogglePanorama, onSourceColorChange, onRevealSource, onRevealOutput }: {
  item: QueueItem;
  presetName: string;
  outputPath: string;
  onClose: () => void;
  onTogglePanorama: () => void;
  onSourceColorChange: (choice: SourceColorChoice) => void;
  onRevealSource: () => void;
  onRevealOutput: (path: string) => void;
}) {
  const mediaType = /\.insv$/i.test(item.source) ? "INSV 全景原片（待拼接）" : item.mediaKind === "sequence" ? "序列帧媒体" : item.mediaKind === "audio" ? "音频文件" : "视频文件";
  const color = [item.colorSpace || "未知", item.colorTransfer || "未知"].join(" · ");
  const tracks = `${item.audioTracks} 条音轨 · ${item.subtitleTracks} 条字幕`;
  return <Modal title={`源信息 · ${item.fileName}`} onClose={onClose} footer={<button className="primary-button" onClick={onClose}>关闭</button>}>
    <div className="media-info-hero">
      {item.thumbnail ? <img src={item.thumbnail} alt={`${item.fileName} 缩略图`} /> : <span className="video-thumb-fallback">{item.mediaKind === "sequence" ? <Images size={26} /> : item.mediaKind === "audio" ? <Zap size={26} /> : <Film size={26} />}</span>}
      <div><strong>{item.fileName}</strong><small>{mediaType} · {item.status} · 进度 {item.progress}%</small><button className="path-link info-path" title={item.source} onClick={onRevealSource}>{item.source}</button></div>
    </div>
    <div className="media-info-grid">
      <InfoField label="文件名" value={item.fileName} />
      <InfoField label="媒体类型" value={mediaType} />
      <InfoField label={/\.insv$/i.test(item.source) ? "预期拼接尺寸" : "分辨率"} value={`${item.width} × ${item.height}`} />
      <InfoField label="帧率" value={item.fps} />
      <InfoField label="时长" value={formatDuration(item.duration)} />
      <InfoField label="文件体积" value={formatBytes(item.sizeBytes)} />
      <InfoField label="码率" value={item.bitrate > 0 ? `${(item.bitrate / 1_000_000).toFixed(2)} Mbps` : "未知"} />
      <InfoField label="视频编码" value={item.codec || "未知"} />
      <InfoField label="位深" value={item.mediaKind === "audio" ? "不适用" : `${item.bitDepth || 8}-bit`} />
      <InfoField label="色度采样" value={item.mediaKind === "audio" ? "不适用" : `4:2:${item.chroma.slice(-1)}`} />
      <InfoField label="色彩空间 / 传递" value={color} />
      <InfoField label="HDR" value={hdrLabel(item.hdrMode)} />
      <InfoField label="HDR 探测" value={hdrSignalLabel(item.hdrSignal)} />
      <InfoField label="量化范围" value={item.mediaKind === "audio" ? "不适用" : item.colorRange === "pc" ? "pc（全范围）" : item.colorRange === "tv" ? "tv（有限范围）" : "未标注，按 tv 处理"} />
      <InfoField label="母版峰值" value={item.peakLuminance && item.peakLuminance > 0 ? `${Math.round(item.peakLuminance)} cd/m²` : "未携带 MDCV"} />
      <InfoField label="MaxCLL / MaxFALL" value={item.maxCll && item.maxCll > 0 ? `${item.maxCll} / ${item.maxFall ?? 0} cd/m²` : "未携带"} />
      <InfoField label="编码域峰值" value={item.mediaKind === "audio" ? "不适用" : "编码时可能抽样，不在此显示"} />
      <InfoField label="Alpha 通道" value={item.hasAlpha ? "有" : "无"} />
      <InfoField label="音轨 / 字幕" value={tracks} />
      <InfoField label="全景状态" value={item.isPanorama ? "全景视频" : "非全景视频"} />
      <InfoField label="自动识别" value={item.panoramaTagged ? "已识别全景元数据" : "未检测到标准全景标记"} />
      <InfoField label="当前状态" value={`${item.status} · ${item.progress}%`} />
      <InfoField label="预设" value={presetName || "未设置预设"} />
      <InfoField label="音频画面" value={item.mediaKind === "audio" ? audioVisualLabel(item.audioVisual) : "不适用"} />
      <InfoField label="外接音频" value={item.externalAudio || "未添加"} />
      <InfoField label="Alpha 遮罩" value={item.hasAlpha ? (item.exportAlphaMask ? "已启用独立导出" : "未启用") : "无 Alpha 通道"} />
    </div>
    {item.mediaKind === "video" && <div className="media-info-section"><h3>源色彩解释</h3>
      <div className="info-color-choice">
        <span>按什么色彩解释源像素</span>
        <select value={item.sourceColorOverride || "auto"} onChange={(event) => onSourceColorChange(event.target.value as SourceColorChoice)}>
          <option value="auto">自动（{hdrSignalLabel(item.hdrSignal)}）</option>
          <option value="sdr">按 {SOURCE_COLOR_LABELS.sdr}</option>
          <option value="hlg">按 {SOURCE_COLOR_LABELS.hlg}</option>
          <option value="hdr10">按 {SOURCE_COLOR_LABELS.hdr10}</option>
        </select>
        <small>{sourceColorHint(item)}</small>
      </div>
    </div>}
    <div className="media-info-section"><h3>全景与输出</h3>{item.mediaKind !== "audio" && <div className="media-info-actions"><button className={`panorama-toggle info-panorama-toggle ${item.isPanorama ? "is-on" : ""}`} disabled={/\.insv$/i.test(item.source)} onClick={onTogglePanorama}>{item.isPanorama ? "全景视频" : "非全景视频"}</button><small>{item.panoramaTagged ? "状态来自自动识别，也可以手动切换" : "可手动切换全景标记"}</small></div>}<button className="path-link info-output-path" title={outputPath || "未设置输出路径"} disabled={!outputPath} onClick={() => outputPath && onRevealOutput(outputPath)}>{outputPath || "未设置输出路径"}</button>{item.alphaOutput && <button className="path-link info-output-path alpha-output-path" title={item.alphaOutput} onClick={() => onRevealOutput(item.alphaOutput)}>Alpha 输出：{item.alphaOutput}</button>}</div>
    {item.mediaKind === "sequence" && <div className="media-info-section"><h3>序列信息</h3><div className="media-info-grid compact"><InfoField label="序列模式" value={item.sequencePattern || "未知"} /><InfoField label="起始帧号" value={String(item.sequenceStartNumber)} /><InfoField label="帧数" value={String(item.sequenceFrameCount)} /><InfoField label="序列帧率" value={`${item.sequenceFps} fps`} /><InfoField label="像素宽幅" value={`${item.sequencePixelAspect}x`} /></div></div>}
  </Modal>;
}

function InfoField({ label, value }: { label: string; value: string }) {
  return <div className="info-field"><small>{label}</small><strong title={value}>{value}</strong></div>;
}

function Setting({ label, children }: { label: string; children: React.ReactNode }) {
  return <label className="setting-field"><span>{label}</span>{children}</label>;
}

function StatusCell({ item, outputPath, onRevealPath }: { item: QueueItem; outputPath: string; onRevealPath: (path: string) => void }) {
  const cancelling = item.status === "正在取消";
  const failed = item.status.includes("失败");
  const complete = item.status.includes("完成");
  const cancelled = !cancelling && item.status.includes("取消");
  const active = !cancelling && (item.status.includes("编码") || item.status.includes("排队") || item.status.includes("Alpha"));
  const timePreserved = item.status.includes("原始时间");
  const stateClass = failed ? "failed" : complete ? "complete" : cancelled ? "cancelled" : cancelling ? "cancelling" : active ? "active" : "ready";
  return <div className={`status-cell ${stateClass}`}>
    <strong>{failed ? "失败" : complete ? "已完成" : cancelled ? "已取消" : cancelling ? "正在取消" : active ? item.progress > 0 && !item.status.includes("%") ? `${item.status} ${item.progress}%` : item.status : "就绪"}</strong>
    {complete ? <small>{timePreserved ? "创建与修改时间已保留" : "输出已生成"}</small> : cancelled ? <small>未生成输出文件</small> : cancelling ? <small>正在终止进程…</small> : active ? <span className="row-progress"><span style={{ width: `${item.progress}%` }} /></span> : <small>等待开始</small>}
    {outputPath && <button className="path-link status-output-path" title={outputPath} onClick={(event) => { event.stopPropagation(); onRevealPath(outputPath); }}>{shortPath(outputPath)}</button>}
  </div>;
}

function estimateRemainingTime(progress: number, elapsedMs: number) {
  if (progress <= 0 || elapsedMs < 1000) return "计算中…";
  const remainingSeconds = Math.max(0, Math.round((elapsedMs / 1000) * (100 - progress) / progress));
  if (remainingSeconds < 60) return `约 ${Math.max(1, remainingSeconds)} 秒`;
  const minutes = Math.floor(remainingSeconds / 60);
  const seconds = remainingSeconds % 60;
  if (minutes < 60) return `约 ${minutes} 分 ${seconds} 秒`;
  const hours = Math.floor(minutes / 60);
  return `约 ${hours} 小时 ${minutes % 60} 分`;
}

function intersects(a: { left: number; right: number; top: number; bottom: number }, b: { left: number; right: number; top: number; bottom: number }) {
  return a.left <= b.right && a.right >= b.left && a.top <= b.bottom && a.bottom >= b.top;
}

function presetForItem(item: QueueItem, presets: Preset[], fallback?: Preset) {
  if (fallback && item.presetId === fallback.id) return fallback;
  return presets.find((preset) => preset.id === item.presetId) ?? fallback;
}

function presetNameForItem(item: QueueItem, presets: Preset[], fallback?: Preset, activePresetId = "") {
  const preset = presetForItem(item, presets, fallback);
  if (!preset) return "";
  if (preset === fallback && preset.id !== activePresetId) {
    const saved = presets.find((entry) => entry.id === preset.id);
    return saved ? `${saved.name}（已调整）` : "自定义设置";
  }
  return preset.name;
}

function estimateOutputBytes(item: QueueItem, preset?: Preset) {
  if (!preset || !item.duration) return 0;
  const outputContainer = preset.outputContainer === "source" && item.mediaKind === "audio" ? "mp4" : preset.outputContainer;
  if (outputContainer === "wav") {
    const bytesPerSample = preset.wavEncoding === "float32" ? 4 : preset.wavEncoding === "pcm24" ? 3 : 2;
    // Approximate using 48 kHz stereo; actual sample rate/channels follow the source.
    return Math.max(44_100, Math.round(item.duration * 48_000 * 2 * bytesPerSample));
  }
  if (item.mediaKind === "audio" && outputContainer === "m4a") {
    return Math.max(32_000, Math.round(item.duration * 320_000 / 8));
  }
  if (preset.hdrMode === "dolby_vision") return item.sizeBytes;
  if (preset.codec === "prores") {
    const value = Math.max(item.sizeBytes, item.duration * 100_000_000 / 8);
    return item.exportAlphaMask && item.hasAlpha ? value * 1.3 : value;
  }
  if (preset.bitrateMode === "source_multiplier" && item.sizeBytes > 0) {
    const videoShare = item.sizeBytes * preset.bitrateMultiplier;
    const audioOverhead = item.audioTracks > 0 || item.externalAudio ? item.duration * 320_000 / 8 : 0;
    const value = Math.max(1_000_000, (videoShare + audioOverhead) * 1.025);
    return item.exportAlphaMask && item.hasAlpha ? value * 1.3 : value;
  }
  const videoBitrate = preset.bitrateMode === "source_multiplier" && item.bitrate > 0
    ? item.bitrate * preset.bitrateMultiplier
    : preset.targetBitrateMbps * 1_000_000;
  const audioBitrate = item.audioTracks > 0 || item.externalAudio ? 320_000 : 0;
  const value = Math.max(1_000_000, (videoBitrate + audioBitrate) * item.duration / 8 * 1.025);
  return item.exportAlphaMask && item.hasAlpha ? value * 1.3 : value;
}

function targetResolution(item: QueueItem, preset?: Preset) {
  if (!preset || preset.resolutionMode === "source") return `${item.width}×${item.height}`;
  if (preset.resolutionMode === "scale_percent") return `原尺寸 ${preset.scalePercent}%`;
  if (preset.resolutionMode === "custom") return `${preset.customWidth}×${preset.customHeight}`;
  return `短边 ${preset.shortEdge}p`;
}

function outputDirectory(preset?: Preset) {
  if (!preset) return "未设置";
  if (preset.outputMode === "single_folder") return preset.outputDir || "选择输出目录";
  if (preset.outputMode === "in_place") return "原文件所在位置";
  return "原位置 / VideoSizeComposer";
}

function hardwareLabel(hardware: Hardware, platform: PlatformInfo) {
  if (hardware === "auto") return platform.accelerators.includes("cuda") ? "CUDA 自动" : platform.accelerators.includes("metal") ? "Metal 自动" : "CPU 自动";
  return hardware.toUpperCase();
}

function hdrLabel(mode: QueueItem["hdrMode"]) {
  if (mode === "dolby_vision") return "Dolby Vision";
  if (mode === "hdr10") return "HDR10";
  if (mode === "hlg") return "HLG";
  return "SDR";
}

function hdrSignalLabel(signal?: QueueItem["hdrSignal"]) {
  switch (signal) {
    case "hlg": return "HLG / BT.2100";
    case "hdr10": return "HDR10 / PQ";
    case "dolby_vision": return "Dolby Vision";
    case "hlg_suspect": return "疑似 HLG（未标注）";
    case "hdr10_suspect": return "疑似 HDR10（未标注）";
    case "sdr": return "SDR";
    default: return "未探测";
  }
}

function isSuspectHdr(item: QueueItem) {
  return item.hdrSignal === "hlg_suspect" || item.hdrSignal === "hdr10_suspect";
}

const SOURCE_COLOR_LABELS: Record<SourceColorChoice, string> = {
  auto: "自动",
  sdr: "SDR / Rec.709",
  hlg: "HLG / BT.2100",
  hdr10: "HDR10 / PQ"
};

function sourceColorHint(item: QueueItem) {
  if (item.sourceColorOverride && item.sourceColorOverride !== "auto") {
    return `已手动指定按 ${SOURCE_COLOR_LABELS[item.sourceColorOverride]} 解释，编码链以该设置为准。`;
  }
  if (isSuspectHdr(item)) {
    return "源没写传递函数，但宽色域 + 10-bit 提示它可能是 HDR；为避免误判，目前仍按 SDR 编码（不改像素）。确认是手机 / 相机的 HLG 素材时选「按 HLG」。";
  }
  if (item.hdrMode !== "sdr") {
    return `源已标注 ${hdrLabel(item.hdrMode)}，编码链按标注解释；怀疑标签写错时可以手动改写。`;
  }
  return "源按 SDR 处理。若这是丢了标签的 HLG / PQ 素材，手动指定后才会按 BT.2100 转换。";
}

function audioVisualLabel(visual: QueueItem["audioVisual"]) {
  if (visual === "black") return "纯黑画面";
  if (visual === "white") return "纯白画面";
  return "居中时间码";
}

function deriveDisplayVersion(version: string) {
  const [year, feature, patch] = version.split(".");
  return /^\d{4}$/.test(year ?? "") && /^\d+$/.test(feature ?? "") && /^\d+$/.test(patch ?? "")
    ? `${year}v${feature}-${patch}`
    : "unknown";
}

function deriveFunctionVersion(version: string) {
  const match = /^(\d{4})v(\d+)-\d+$/.exec(version.trim());
  return match ? `${match[1]}v${match[2]}` : "unknown";
}

function previewOutput(item: QueueItem, preset: Preset) {
  const slash = Math.max(item.source.lastIndexOf("/"), item.source.lastIndexOf("\\"));
  const directory = slash >= 0 ? item.source.slice(0, slash) : ".";
  const fileName = slash >= 0 ? item.source.slice(slash + 1) : item.source;
  const dot = fileName.lastIndexOf(".");
  const hasExtension = dot > 0 && dot < fileName.length - 1;
  const rawStem = hasExtension ? fileName.slice(0, dot) : fileName;
  const stem = item.mediaKind === "sequence"
    ? sequenceBaseName(rawStem)
    : rawStem;
  const name = preset.namingMode === "original" ? stem : `${preset.prefix}${stem}${preset.suffix}`;
  const sourceExtension = hasExtension ? fileName.slice(dot).toLowerCase() : "";
  const extension = preset.outputContainer === "source"
    ? sourceExtension === ".insv" ? (preset.codec === "prores" ? ".mov" : ".mp4") : item.mediaKind === "sequence" || item.mediaKind === "audio" ? ".mp4" : sourceExtension || (preset.codec === "prores" ? ".mov" : ".mp4")
    : `.${preset.outputContainer}`;
  const outputDirectory = preset.outputMode === "single_folder" && preset.outputDir
    ? preset.outputDir
    : preset.outputMode === "in_place"
      ? directory
      : `${directory}/VideoSizeComposer`;
  return `${outputDirectory}/${name}${extension}`;
}

function previewAlphaOutput(output: string) {
  if (!output) return "";
  const slash = Math.max(output.lastIndexOf("/"), output.lastIndexOf("\\"));
  const directory = slash >= 0 ? output.slice(0, slash + 1) : "";
  const fileName = slash >= 0 ? output.slice(slash + 1) : output;
  const dot = fileName.lastIndexOf(".");
  return `${directory}${dot > 0 ? `${fileName.slice(0, dot)}_alpha${fileName.slice(dot)}` : `${fileName}_alpha`}`;
}

function sequenceBaseName(stem: string) {
  // Mirrors Rust split_sequence_stem: keep the last contiguous digit run,
  // so frame_001 -> frame, shot2_frame001 -> shot2_frame, 001_left -> left.
  const match = stem.match(/^(.*?)(\d+)([^\d]*)$/);
  if (!match) return stem;
  const prefix = match[1].replace(/[_\-. ]+$/, "");
  const suffix = match[3].replace(/^[_\-. ]+/, "");
  if (prefix && suffix) return `${prefix}_${suffix}`;
  return prefix || suffix || "sequence";
}

function mergeQueue(existing: QueueItem[], incoming: QueueItem[]) {
  const known = new Set(existing.map((item) => item.source.toLowerCase()));
  return [...existing, ...incoming.filter((item) => !known.has(item.source.toLowerCase()))];
}

function formatBytes(size: number) {
  if (!Number.isFinite(size) || size <= 0) return "0 B";
  if (size >= 1024 ** 3) return `${(size / 1024 ** 3).toFixed(1)} GB`;
  if (size >= 1024 ** 2) return `${Math.round(size / 1024 ** 2)} MB`;
  return `${Math.round(size / 1024)} KB`;
}

function formatDuration(seconds: number) {
  if (!seconds) return "00:00";
  const total = Math.max(0, Math.round(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  return hours ? `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}` : `${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
}

function shortPath(value: string) {
  if (value.length <= 25) return value;
  return `…${value.slice(-24)}`;
}

function handleBrowserDrop(addPaths: (paths: string[]) => void) {
  return (event: React.DragEvent) => {
    event.preventDefault();
    const paths = Array.from(event.dataTransfer.files).map((file) => (file as File & { path?: string }).path ?? file.name);
    addPaths(paths);
  };
}
