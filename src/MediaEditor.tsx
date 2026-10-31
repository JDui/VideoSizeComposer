import { useEffect, useId, useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent } from "react";
import { Check, ChevronDown, ChevronLeft, ChevronRight, Clock3, Crop, Film, FolderOpen, Frame, LoaderCircle, LockKeyhole, Music2, Palette, RotateCcw, X } from "lucide-react";
import { CropPreview } from "./CropPreview";
import { pickLutFiles, previewMediaEdit } from "./tauri";
import type { MediaEdits, Preset, QueueItem } from "./types";
import "./media-editor.css";

export function defaultMediaEdits(): MediaEdits {
  return { trimStart: 0, trimEnd: 0, crop: null, canvasWidth: 0, canvasHeight: 0, fit: "contain", lutEnabled: null, lutName: "", lutIntensity: 80 };
}

export const isPanoramaMedia = (item: QueueItem) => item.isPanorama || /\.insv$/i.test(item.source);

export function constrainMediaEdits(item: QueueItem, edits: MediaEdits): MediaEdits {
  return isPanoramaMedia(item) ? { ...edits, crop: null, canvasWidth: 0, canvasHeight: 0, fit: "contain" } : edits;
}

export function mediaEditSummary(item: QueueItem): string {
  const edit = item.edits;
  if (!edit) return "";
  const parts: string[] = [];
  if (edit.trimStart > 0 || edit.trimEnd > 0) parts.push(`片段 ${clock(edit.trimStart)}–${clock(edit.trimEnd || item.duration)}`);
  if (edit.crop) parts.push("已裁切");
  if (edit.canvasWidth > 0 && edit.canvasHeight > 0) parts.push(`${edit.canvasWidth}×${edit.canvasHeight}`);
  if (edit.lutEnabled !== null) parts.push(edit.lutEnabled ? "单项 LUT" : "LUT 关闭");
  return parts.join(" · ");
}

const ratios: Record<string, number> = { "16:9": 16 / 9, "9:16": 9 / 16, "1:1": 1, "4:3": 4 / 3 };
const even = (value: number) => Math.max(2, Math.floor(value / 2) * 2);
const finite = (value: number) => Number.isFinite(value);
const rounded = (value: number) => Math.round(value * 1000) / 1000;
const precise = (value: number) => Math.round(value * 1_000_000) / 1_000_000;
const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(value, max));
type TimelinePart = "start" | "end" | "playhead";

function clock(seconds: number) {
  const milliseconds = Math.round(Math.max(0, finite(seconds) ? seconds : 0) * 1000);
  const whole = Math.floor(milliseconds / 1000);
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}.${String(milliseconds % 1000).padStart(3, "0")}`;
}

function timecode(seconds: number, fps: number) {
  const safe = Math.max(0, finite(seconds) ? seconds : 0);
  const whole = Math.floor(safe + 0.000001);
  const frames = Math.min(Math.ceil(fps) - 1, Math.floor((safe - whole) * fps + 0.0001));
  return [Math.floor(whole / 3600), Math.floor(whole / 60) % 60, whole % 60, frames].map((value) => String(value).padStart(2, "0")).join(":");
}

function frameMode(edit: MediaEdits, item: QueueItem) {
  if (!edit.canvasWidth && !edit.canvasHeight) return "follow";
  if (edit.canvasWidth === item.width && edit.canvasHeight === item.height) return "source";
  return Object.keys(ratios).find((key) => Math.abs(edit.canvasWidth / edit.canvasHeight - ratios[key]) < 0.003) ?? "custom";
}

function validate(edit: MediaEdits, item: QueueItem) {
  if (isPanoramaMedia(item) && (edit.crop || edit.canvasWidth || edit.canvasHeight)) return "全景视频已禁用裁切和画幅修改，可截取片段与预览 LUT。";
  if (!finite(edit.trimStart) || !finite(edit.trimEnd) || edit.trimStart < 0 || edit.trimEnd < 0) return "请输入有效的片段起止时间。";
  const end = edit.trimEnd || item.duration;
  if (item.duration > 0 && (edit.trimStart >= end || end > item.duration + 0.001)) return "出点必须晚于入点，且不能超过源片总时长。";
  if (!item.duration && (edit.trimStart > 0 || edit.trimEnd > 0)) return "源片时长未知，暂时无法设置片段。";
  if (item.mediaKind === "audio") return "";
  if (![edit.canvasWidth, edit.canvasHeight].every(finite) || (edit.canvasWidth !== 0 || edit.canvasHeight !== 0) && (edit.canvasWidth < 2 || edit.canvasHeight < 2 || edit.canvasWidth > 16384 || edit.canvasHeight > 16384 || edit.canvasWidth % 2 !== 0 || edit.canvasHeight % 2 !== 0)) return "画幅宽高需为 2–16384 范围内的偶数。";
  if (edit.crop) {
    const { x, y, width, height } = edit.crop;
    if (![x, y, width, height].every(finite) || ![x, y, width, height].every(Number.isInteger) || x < 0 || y < 0 || width < 2 || height < 2 || width > 16384 || height > 16384 || width % 2 !== 0 || height % 2 !== 0 || x + width > item.width || y + height > item.height) return `裁切范围需在源画面 ${item.width}×${item.height} 内，宽高为 2–16384 的偶数。`;
  }
  if (edit.lutEnabled && !edit.lutName.trim()) return "请先选择本项使用的 LUT 文件。";
  return "";
}

function effectiveCrop(crop: MediaEdits["crop"], item: QueueItem) {
  return crop && crop.x === 0 && crop.y === 0 && crop.width === item.width && crop.height === item.height ? null : crop;
}

export function MediaEditor({ item, preset, onApply, onClose }: {
  item: QueueItem;
  preset: Preset;
  onApply: (edits: MediaEdits) => void;
  onClose: () => void;
}) {
  const [draft, setDraft] = useState<MediaEdits>(() => constrainMediaEdits(item, { ...defaultMediaEdits(), ...item.edits, crop: item.edits?.crop ? { ...item.edits.crop } : null }));
  const [canvasMode, setCanvasMode] = useState(() => isPanoramaMedia(item) ? "follow" : frameMode({ ...defaultMediaEdits(), ...item.edits }, item));
  const [cropRatio, setCropRatio] = useState("none");
  const [cropEditing, setCropEditing] = useState(false);
  const [zoom, setZoom] = useState("fit");
  const [customRange, setCustomRange] = useState(Boolean(item.edits?.trimStart || item.edits?.trimEnd));
  const [middleSeconds, setMiddleSeconds] = useState(() => Math.min(10, item.duration || 10));
  const [atSeconds, setAtSeconds] = useState(() => Math.min(item.edits?.trimStart || 0, Math.max(0, item.duration - 0.001)));
  const [preview, setPreview] = useState("");
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState("");
  const [lutLoading, setLutLoading] = useState(false);
  const [actionError, setActionError] = useState("");
  const [retry, setRetry] = useState(0);
  const dialogRef = useRef<HTMLElement>(null);
  const timelineRef = useRef<HTMLDivElement>(null);
  const timelineDrag = useRef<TimelinePart | null>(null);
  const previewCache = useRef(new Map<string, string>());
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const uid = useId();
  const audioOnly = item.mediaKind === "audio";
  const panoramaLocked = isPanoramaMedia(item);
  const duration = Math.max(0, item.duration || 0);
  const safeStart = finite(draft.trimStart) ? draft.trimStart : 0;
  const end = finite(draft.trimEnd) && draft.trimEnd > 0 ? draft.trimEnd : duration;
  const selectedDuration = Math.max(0, end - safeStart);
  const validationError = validate(draft, item);
  const fraction = /^(\d+(?:\.\d+)?)\/(\d+(?:\.\d+)?)/.exec(item.fps);
  const parsedFps = item.mediaKind === "sequence" ? item.sequenceFps : fraction ? Number(fraction[1]) / Number(fraction[2]) : parseFloat(item.fps);
  const fps = parsedFps > 0 && finite(parsedFps) ? parsedFps : 30;
  const frameStep = Math.min(1 / fps, duration || 1 / fps);
  const lastFrameTime = Math.max(0, duration - Math.min(frameStep / 2, 0.001));
  const sourceIdentity = JSON.stringify([item.source, item.sourceColorOverride, item.sequenceFps, item.sequencePixelAspect, item.width, item.height]);
  // The crop tool needs a full-size frame with the same color/LUT treatment.
  // Its geometry changes only the overlay, so dragging does not decode again.
  const previewEdits: MediaEdits = cropEditing ? { ...draft, trimStart: 0, trimEnd: 0, crop: null, canvasWidth: 0, canvasHeight: 0 } : draft;
  const previewPreset: Preset = cropEditing ? { ...preset, resolutionMode: "source" } : preset;
  const previewKey = JSON.stringify([sourceIdentity, atSeconds, previewEdits, previewPreset, retry]);
  const canPreview = !audioOnly && duration > 0 && (cropEditing || !validationError);
  const percent = (time: number) => duration > 0 ? clamp(time / duration * 100, 0, 100) : 0;
  const thumbnailFallback = !preview && Boolean(item.thumbnail);
  const displayedImage = preview || item.thumbnail || "";

  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialogRef.current?.focus();
    const handleKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); closeRef.current(); }
      if (event.key !== "Tab") return;
      const focusable = Array.from(dialogRef.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), summary, [tabindex="0"]') ?? []).filter((element) => element.getClientRects().length > 0);
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (!first) { event.preventDefault(); return; }
      if (event.shiftKey && (document.activeElement === first || document.activeElement === dialogRef.current)) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && (document.activeElement === last || document.activeElement === dialogRef.current)) { event.preventDefault(); first.focus(); }
    };
    document.addEventListener("keydown", handleKey, true);
    return () => { document.removeEventListener("keydown", handleKey, true); previous?.focus(); };
  }, []);

  useEffect(() => {
    if (!canPreview) { setPreviewLoading(false); return; }
    const cached = previewCache.current.get(previewKey);
    if (cached) { setPreview(cached); setPreviewError(""); setPreviewLoading(false); return; }
    let stale = false;
    setPreviewLoading(true);
    setPreviewError("");
    // A frame with a different crop or canvas must not masquerade as the new result.
    setPreview("");
    const timer = window.setTimeout(() => {
      previewMediaEdit({ ...item, edits: previewEdits }, previewPreset, clamp(atSeconds, 0, lastFrameTime))
        .then((result) => {
          previewCache.current.set(previewKey, result);
          if (previewCache.current.size > 12) previewCache.current.delete(previewCache.current.keys().next().value!);
          if (!stale) setPreview(result);
        })
        .catch((error: unknown) => { if (!stale) { setPreview(""); setPreviewError(String(error)); } })
        .finally(() => { if (!stale) setPreviewLoading(false); });
    }, 280);
    return () => { stale = true; window.clearTimeout(timer); };
  }, [previewKey, canPreview]);

  const patch = (changes: Partial<MediaEdits>) => {
    const showResult = ["canvasWidth", "canvasHeight", "fit", "lutEnabled", "lutName", "lutIntensity"].some((key) => key in changes);
    if (showResult) setCropEditing(false);
    setDraft((current) => {
      const next = constrainMediaEdits(item, { ...current, ...changes });
      return showResult ? { ...next, crop: effectiveCrop(next.crop, item) } : next;
    });
    setActionError("");
  };
  const seek = (time: number) => setAtSeconds(clamp(time, 0, lastFrameTime));
  const setTrim = (part: "start" | "end", value: number) => {
    setCustomRange(true);
    if (part === "start") patch({ trimStart: precise(clamp(value, 0, Math.max(0, end - frameStep))) });
    else patch({ trimEnd: Math.min(duration, precise(clamp(value, Math.min(duration, safeStart + frameStep), duration))) });
  };
  const changeTimeline = (part: TimelinePart, value: number) => {
    const snapped = Math.round(value * fps) / fps;
    if (part === "playhead") seek(snapped);
    else setTrim(part, snapped);
  };
  const timeAtPointer = (event: PointerEvent) => {
    const rect = timelineRef.current!.getBoundingClientRect();
    return clamp((event.clientX - rect.left) / rect.width, 0, 1) * duration;
  };
  const startTimelineDrag = (event: PointerEvent<HTMLElement>, part: TimelinePart) => {
    if (duration <= 0 || event.button !== 0) return;
    event.preventDefault(); event.stopPropagation();
    timelineDrag.current = part;
    timelineRef.current?.setPointerCapture(event.pointerId);
    changeTimeline(part, timeAtPointer(event));
  };
  const timelineKey = (event: KeyboardEvent<HTMLElement>, part: TimelinePart) => {
    let value = part === "start" ? safeStart : part === "end" ? end : atSeconds;
    if (event.key === "ArrowLeft" || event.key === "ArrowDown") value -= frameStep * (event.shiftKey ? 10 : 1);
    else if (event.key === "ArrowRight" || event.key === "ArrowUp") value += frameStep * (event.shiftKey ? 10 : 1);
    else if (event.key === "Home") value = 0;
    else if (event.key === "End") value = duration;
    else return;
    event.preventDefault(); event.stopPropagation(); changeTimeline(part, value);
  };
  const setFrame = (mode: string) => {
    if (panoramaLocked) return;
    setCanvasMode(mode);
    if (mode === "follow") { patch({ canvasWidth: 0, canvasHeight: 0 }); return; }
    if (mode === "source") { patch({ canvasWidth: even(item.width || 1920), canvasHeight: even(item.height || 1080) }); return; }
    if (mode === "custom") { patch({ canvasWidth: draft.canvasWidth || even(item.width || 1920), canvasHeight: draft.canvasHeight || even(item.height || 1080) }); return; }
    const ratio = ratios[mode];
    const width = Math.min(item.width || 1920, (item.height || 1080) * ratio);
    patch({ canvasWidth: even(width), canvasHeight: even(width / ratio) });
  };
  const enableCrop = () => {
    if (panoramaLocked) return;
    if (cropEditing) { patch({ crop: effectiveCrop(draft.crop, item) }); setCropEditing(false); return; }
    if (!draft.crop) patch({ crop: { x: 0, y: 0, width: even(Math.min(item.width, 16384)), height: even(Math.min(item.height, 16384)) } });
    setCropEditing(true);
  };
  const setRatio = (ratioName: string) => {
    if (panoramaLocked) return;
    setCropRatio(ratioName);
    if (ratioName === "none") return;
    setCropEditing(true);
    const ratio = ratios[ratioName];
    const width = even(Math.min(item.width, 16384, Math.min(item.height, 16384) * ratio));
    const height = even(Math.min(item.height, 16384, width / ratio));
    patch({ crop: { x: Math.floor((item.width - width) / 2), y: Math.floor((item.height - height) / 2), width, height } });
  };
  const setCropMargin = (side: "left" | "top" | "right" | "bottom", value: number) => {
    if (panoramaLocked) return;
    if (!draft.crop || !finite(value)) return;
    setCropEditing(true);
    const crop = draft.crop;
    const right = item.width - crop.x - crop.width;
    const bottom = item.height - crop.y - crop.height;
    const margin = Math.max(0, Math.round(value));
    setCropRatio("none");
    if (side === "left") { const x = Math.min(margin, item.width - right - 2); patch({ crop: { ...crop, x, width: even(Math.min(16384, item.width - x - right)) } }); }
    if (side === "right") patch({ crop: { ...crop, width: even(clamp(item.width - crop.x - margin, 2, Math.min(16384, item.width - crop.x))) } });
    if (side === "top") { const y = Math.min(margin, item.height - bottom - 2); patch({ crop: { ...crop, y, height: even(Math.min(16384, item.height - y - bottom)) } }); }
    if (side === "bottom") patch({ crop: { ...crop, height: even(clamp(item.height - crop.y - margin, 2, Math.min(16384, item.height - crop.y))) } });
  };
  const chooseLut = async () => {
    setLutLoading(true); setActionError("");
    try { const paths = await pickLutFiles(); if (paths[0]) patch({ lutEnabled: true, lutName: paths[0] }); }
    catch (error) { setActionError(String(error)); }
    finally { setLutLoading(false); }
  };
  const keepMiddle = () => {
    if (!finite(middleSeconds) || middleSeconds <= 0 || duration <= 0) { setActionError("请输入大于 0 的居中片段时长。"); return; }
    const length = Math.min(middleSeconds, duration);
    const start = rounded((duration - length) / 2);
    patch({ trimStart: start, trimEnd: Math.min(duration, rounded(start + length)) });
    setCustomRange(true); seek(duration / 2);
  };
  const apply = () => {
    if (validationError || lutLoading) return;
    onApply(audioOnly ? { ...defaultMediaEdits(), trimStart: draft.trimStart, trimEnd: draft.trimEnd } : { ...draft, crop: effectiveCrop(draft.crop, item) });
  };
  const cropMargins = draft.crop ? { left: draft.crop.x, top: draft.crop.y, right: item.width - draft.crop.x - draft.crop.width, bottom: item.height - draft.crop.y - draft.crop.height } : { left: 0, top: 0, right: 0, bottom: 0 };

  return <div className="modal-backdrop media-editor-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
    <section className={`modal-card media-editor ${audioOnly ? "media-editor-audio" : ""}`} ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby={`${uid}-title ${uid}-file`} tabIndex={-1}>
      <header><div className="media-editor-heading"><span className="media-editor-heading-icon" aria-hidden="true">{audioOnly ? <Music2 size={19} /> : <Film size={19} />}</span><div><h2 id={`${uid}-title`}>编辑素材</h2><span id={`${uid}-file`} title={item.source}>{item.fileName}</span></div></div><button className="icon-only" aria-label="关闭编辑窗口" onClick={onClose}><X size={18} /></button></header>
      <div className="modal-body media-editor-body">
        {!audioOnly && <>
          <div className="media-editor-crop-toolbar">
            {panoramaLocked ? <><button className="media-editor-crop-toggle" aria-label="全景视频禁止裁切" disabled><LockKeyhole size={14} /><span>裁切已禁用</span></button><span className="media-editor-panorama-note">全景保留完整画幅 · 可截取片段与预览 LUT</span><span className="media-editor-crop-size">{item.width} × {item.height}</span></> : <>
            <button className={`media-editor-crop-toggle ${cropEditing ? "active" : ""}`} aria-label={cropEditing ? "完成裁切" : "编辑裁切"} aria-pressed={cropEditing} title={cropEditing ? "完成裁切并预览处理后的画面" : "在画面中调整裁切范围"} onClick={enableCrop}><Crop size={16} /><span>{cropEditing ? "完成裁切" : "裁切"}</span></button>
            {draft.crop && <button className="icon-only media-editor-clear-crop" aria-label="清除裁切" title="恢复完整画面" onClick={() => { patch({ crop: null }); setCropEditing(false); setCropRatio("none"); }}><RotateCcw size={14} /></button>}
            <div className="media-editor-crop-margins">{([ ["left", "左侧"], ["top", "顶部"], ["right", "右侧"], ["bottom", "底部"] ] as const).map(([side, label]) => <label className="media-editor-margin" key={side}><span>{label}</span><input aria-label={`裁切${label}像素`} type="number" min={0} max={(side === "left" || side === "right" ? item.width : item.height) - 2} step={1} value={cropMargins[side]} disabled={!draft.crop} onChange={(event) => setCropMargin(side, Number(event.target.value))} /><small>px</small></label>)}</div>
            <label className="media-editor-ratio"><span>裁切比例</span><select aria-label="裁切比例" value={cropRatio} onChange={(event) => setRatio(event.target.value)}><option value="none">无</option>{Object.keys(ratios).map((ratio) => <option key={ratio} value={ratio}>{ratio}</option>)}</select></label>
            <span className="media-editor-crop-size">{draft.crop ? `${cropEditing ? "保留" : "已裁切"} ${draft.crop.width} × ${draft.crop.height}` : `${item.width} × ${item.height}`}</span>
            </>}
          </div>
        </>}
        <div className="media-editor-preview" id={`${uid}-preview`} role="region" aria-label={audioOnly ? "音频片段" : "画面预览"} aria-busy={previewLoading}>
          {!audioOnly && displayedImage && <CropPreview src={displayedImage} sourceWidth={item.width} sourceHeight={item.height} crop={cropEditing ? draft.crop : null} editable={cropEditing && Boolean(draft.crop)} aspectRatio={cropRatio === "none" ? undefined : ratios[cropRatio]} zoom={zoom === "fit" ? null : Number(zoom)} onChange={(crop) => patch({ crop })} />}
          {audioOnly && <div className="media-editor-preview-placeholder"><Music2 size={36} /><strong>{item.fileName}</strong><span>拖动时间轴入点与出点，保留所需音频片段</span></div>}
          {!audioOnly && !displayedImage && !previewLoading && !previewError && <div className="media-editor-preview-placeholder"><Film size={30} /><span>静帧预览</span></div>}
          {previewLoading && <div className="media-editor-preview-status" role="status"><LoaderCircle className="spin" size={15} /><span>更新预览</span></div>}
          {(previewError && !thumbnailFallback || !cropEditing && validationError) && <div className="media-editor-preview-failure" role="alert"><strong>暂时无法预览</strong><span>{previewError || validationError}</span><button className="secondary-button" onClick={() => setRetry((value) => value + 1)}>重试</button></div>}
          {!audioOnly && !(thumbnailFallback && previewError) && <span className="media-editor-preview-caption">{thumbnailFallback ? "源缩略图 · 当前帧预览尚未载入" : cropEditing ? "拖动白框移动裁切区域 · 拖动角点调整大小" : "画面预览"}</span>}
          {thumbnailFallback && previewError && <div className="media-editor-preview-fallback" role="status"><span title={previewError}>预览不可用 · 暂用源缩略图</span><button onClick={() => setRetry((value) => value + 1)}>重试</button></div>}
        </div>
        <div className="media-editor-transport">
          <output className="media-editor-current-time" aria-label="当前播放头时间码">{timecode(atSeconds, fps)}</output>
          <div className="media-editor-step-buttons"><button aria-label="上一帧" title="上一帧" disabled={duration <= 0 || atSeconds <= 0} onClick={() => seek(atSeconds - frameStep)}><ChevronLeft size={19} /></button><button aria-label="下一帧" title="下一帧" disabled={duration <= 0 || atSeconds >= lastFrameTime} onClick={() => seek(atSeconds + frameStep)}><ChevronRight size={19} /></button></div>
          <button className="media-editor-mark-button" title="将播放头设为入点" disabled={duration <= 0} onClick={() => setTrim("start", atSeconds)}>设入点</button><button className="media-editor-mark-button" title="将播放头设为出点" disabled={duration <= 0} onClick={() => setTrim("end", atSeconds + frameStep)}>设出点</button>
          <span className="media-editor-still-note">{audioOnly ? "音频片段" : "静帧预览"}</span>
          {!audioOnly && <label className="media-editor-zoom">缩放<select aria-label="预览缩放" value={zoom} onChange={(event) => setZoom(event.target.value)}><option value="fit">适合</option><option value="0.5">50%</option><option value="1">100%</option><option value="2">200%</option></select></label>}
          <span className="media-editor-source-duration">{timecode(duration, fps)}</span>
        </div>
        <div className="media-editor-timeline-wrap">
          <div className="media-editor-ruler" aria-hidden="true">{Array.from({ length: 7 }, (_, index) => <span key={index}>{timecode(duration * index / 6, fps)}</span>)}</div>
          <div className="media-editor-timeline" ref={timelineRef} role="slider" aria-label="预览播放头" aria-valuemin={0} aria-valuemax={duration} aria-valuenow={atSeconds} aria-valuetext={timecode(atSeconds, fps)} tabIndex={0} onKeyDown={(event) => timelineKey(event, "playhead")} onPointerDown={(event) => startTimelineDrag(event, "playhead")} onPointerMove={(event) => { if (timelineDrag.current) changeTimeline(timelineDrag.current, timeAtPointer(event)); }} onPointerUp={(event) => { timelineDrag.current = null; if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId); }} onPointerCancel={() => { timelineDrag.current = null; }}>
            <div className="media-editor-timeline-selection" style={{ left: `${percent(safeStart)}%`, width: `${Math.max(0, percent(end) - percent(safeStart))}%` }} />
            <button className="media-editor-trim-handle start" role="slider" aria-label="片段入点" aria-valuemin={0} aria-valuemax={Math.max(0, end - frameStep)} aria-valuenow={safeStart} aria-valuetext={clock(safeStart)} title={`入点 ${clock(safeStart)}`} style={{ left: `${percent(safeStart)}%` }} onPointerDown={(event) => startTimelineDrag(event, "start")} onKeyDown={(event) => timelineKey(event, "start")}><span /></button>
            <button className="media-editor-trim-handle end" role="slider" aria-label="片段出点" aria-valuemin={Math.min(duration, safeStart + frameStep)} aria-valuemax={duration} aria-valuenow={end} aria-valuetext={clock(end)} title={`出点 ${clock(end)}`} style={{ left: `${percent(end)}%` }} onPointerDown={(event) => startTimelineDrag(event, "end")} onKeyDown={(event) => timelineKey(event, "end")}><span /></button>
            <div className="media-editor-playhead" style={{ left: `${percent(atSeconds)}%` }}><span /></div>
          </div>
        </div>
        <div className="media-editor-range-controls">
          <label>源范围<select aria-label="源范围" value={customRange ? "custom" : "full"} onChange={(event) => { setCustomRange(event.target.value === "custom"); if (event.target.value === "full") patch({ trimStart: 0, trimEnd: 0 }); }}><option value="full">完整素材</option><option value="custom">自定义</option></select></label>
          <label>入点<input aria-label="入点（秒）" type="number" min={0} max={duration} step={0.001} value={finite(draft.trimStart) ? draft.trimStart : ""} onChange={(event) => { setCustomRange(true); patch({ trimStart: event.target.value === "" ? NaN : Number(event.target.value) }); }} /><small>秒</small></label>
          <label>出点<input aria-label="出点（秒，留空至结尾）" type="number" min={0} max={duration} step={0.001} placeholder={String(rounded(duration))} value={draft.trimEnd > 0 ? draft.trimEnd : ""} onChange={(event) => { setCustomRange(true); patch({ trimEnd: event.target.value === "" ? 0 : Number(event.target.value) }); }} /><small>秒</small></label>
          <span className="media-editor-selected-duration"><Clock3 size={13} />保留 <strong>{clock(selectedDuration)}</strong></span>
          <div className="media-editor-middle"><label>居中<input aria-label="居中片段时长（秒）" type="number" min={0.001} max={duration || undefined} step={0.1} value={finite(middleSeconds) ? middleSeconds : ""} onChange={(event) => setMiddleSeconds(event.target.value === "" ? NaN : Number(event.target.value))} /><small>秒</small></label><button className="secondary-button" onClick={keepMiddle} disabled={duration <= 0}>取中间片段</button></div>
        </div>
        {!audioOnly && <div className="media-editor-options">
          {panoramaLocked ? <div className="media-editor-option-group media-editor-locked-canvas" title="全景视频保持完整投影，禁止裁切或改变画幅"><span className="media-editor-option-title"><LockKeyhole size={14} />全景画幅</span><span className="media-editor-option-value">已锁定完整画面</span></div> : <details className="media-editor-option-group"><summary><span className="media-editor-option-title"><Frame size={15} />输出画幅</span><span className="media-editor-option-value">{canvasMode === "follow" ? "跟随输出设置" : `${draft.canvasWidth} × ${draft.canvasHeight}`}</span><ChevronDown size={14} /></summary><div className="media-editor-option-fields"><label>画幅<select aria-label="输出画幅" value={canvasMode} onChange={(event) => setFrame(event.target.value)}><option value="follow">跟随输出设置</option><option value="source">原始尺寸</option>{Object.keys(ratios).map((ratio) => <option key={ratio} value={ratio}>{ratio}</option>)}<option value="custom">自定义</option></select></label>{canvasMode !== "follow" && <><label>宽<input aria-label="输出画幅宽" type="number" min={2} max={16384} step={2} value={finite(draft.canvasWidth) ? draft.canvasWidth : ""} onChange={(event) => { setCanvasMode("custom"); patch({ canvasWidth: event.target.value === "" ? NaN : Number(event.target.value) }); }} /></label><label>高<input aria-label="输出画幅高" type="number" min={2} max={16384} step={2} value={finite(draft.canvasHeight) ? draft.canvasHeight : ""} onChange={(event) => { setCanvasMode("custom"); patch({ canvasHeight: event.target.value === "" ? NaN : Number(event.target.value) }); }} /></label></>}<label>适配<select aria-label="画面适配" value={draft.fit} disabled={canvasMode === "follow"} onChange={(event) => patch({ fit: event.target.value as MediaEdits["fit"] })}><option value="contain">完整显示 · 留黑边</option><option value="cover">居中填满 · 裁切</option></select></label></div></details>}
          <details className="media-editor-option-group"><summary><span className="media-editor-option-title"><Palette size={15} />LUT 调色</span><span className="media-editor-option-value">{draft.lutEnabled === null ? "跟随输出设置" : draft.lutEnabled ? draft.lutName.split(/[\\/]/).pop() || "未选择文件" : "本项关闭"}</span><ChevronDown size={14} /></summary><div className="media-editor-option-fields"><label>来源<select aria-label="LUT 来源" value={draft.lutEnabled === null ? "follow" : draft.lutEnabled ? "custom" : "off"} onChange={(event) => patch({ lutEnabled: event.target.value === "follow" ? null : event.target.value === "custom" })}><option value="follow">跟随输出设置</option><option value="off">本项关闭</option><option value="custom">单独设置</option></select></label>{draft.lutEnabled === true && <><button className="secondary-button media-editor-lut-file" onClick={chooseLut} disabled={lutLoading} title={draft.lutName}><FolderOpen size={14} /><span>{lutLoading ? "导入中…" : draft.lutName.split(/[\\/]/).pop() || "选择 LUT"}</span></button><label className="media-editor-lut-strength">强度<input aria-label="LUT 强度" type="range" min={0} max={100} step={1} value={draft.lutIntensity} onChange={(event) => patch({ lutIntensity: Number(event.target.value) })} /><strong>{draft.lutIntensity}%</strong></label></>}</div></details>
        </div>}
      </div>
      <footer><button className="secondary-button media-editor-reset" onClick={() => { setDraft(defaultMediaEdits()); setCanvasMode("follow"); setCropRatio("none"); setCropEditing(false); setCustomRange(false); setActionError(""); }}><RotateCcw size={14} />重置</button><div className="media-editor-footer-message" role={validationError || actionError ? "alert" : undefined}>{validationError || actionError || (preset.hdrMode === "dolby_vision" ? "Dolby Vision 保留模式不支持片段、画幅、裁切和 LUT 修改" : cropEditing && !audioOnly ? "拖动白框调整区域，完成裁切后预览效果" : "修改仅应用到当前素材")}</div><button className="secondary-button" onClick={onClose}>取消</button><button className="primary-button" disabled={Boolean(validationError) || lutLoading} onClick={apply}><Check size={15} />应用修改</button></footer>
    </section>
  </div>;
}
