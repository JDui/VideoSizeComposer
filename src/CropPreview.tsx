import { useEffect, useRef, useState } from "react";
import type { MediaEdits } from "./types";
import "./crop-preview.css";

type Crop = NonNullable<MediaEdits["crop"]>;
type Handle = "move" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "nw";
const handles: Exclude<Handle, "move">[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];
const names = { nw: "左上角", n: "上边", ne: "右上角", e: "右边", se: "右下角", s: "下边", sw: "左下角", w: "左边" };
const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value));
const even = (value: number) => Math.max(2, Math.round(value / 2) * 2);

export function CropPreview({ src, sourceWidth, sourceHeight, crop, onChange, editable, aspectRatio, zoom }: {
  src: string;
  sourceWidth: number;
  sourceHeight: number;
  crop: MediaEdits["crop"];
  onChange: (crop: Crop) => void;
  editable: boolean;
  aspectRatio?: number;
  /** Null fits the viewport; 1 means original pixel size. */
  zoom?: number | null;
}) {
  const viewport = useRef<HTMLDivElement>(null);
  const frame = useRef<HTMLDivElement>(null);
  const drag = useRef<{ handle: Handle; x: number; y: number; crop: Crop; scale: number; pointer: number }>();
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [natural, setNatural] = useState({ width: sourceWidth, height: sourceHeight });

  useEffect(() => {
    const element = viewport.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => setSize({ width: entry.contentRect.width, height: entry.contentRect.height }));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const width = editable ? sourceWidth : natural.width || sourceWidth;
  const height = editable ? sourceHeight : natural.height || sourceHeight;
  const scale = zoom && zoom > 0 ? zoom : Math.max(0.001, Math.min(Math.max(0, size.width - 16) / Math.max(1, width), Math.max(0, size.height - 16) / Math.max(1, height)));
  const active = editable && crop && src && sourceWidth >= 2 && sourceHeight >= 2;

  function resize(initial: Crop, handle: Handle, dx: number, dy: number): Crop {
    const maxWidth = Math.min(16384, sourceWidth);
    const maxHeight = Math.min(16384, sourceHeight);
    if (handle === "move") {
      return { ...initial, x: Math.round(clamp(initial.x + dx, 0, sourceWidth - initial.width)), y: Math.round(clamp(initial.y + dy, 0, sourceHeight - initial.height)) };
    }
    let left = initial.x;
    let top = initial.y;
    let right = left + initial.width;
    let bottom = top + initial.height;
    if (handle.includes("w")) left = clamp(left + dx, Math.max(0, right - maxWidth), right - 2);
    if (handle.includes("e")) right = clamp(right + dx, left + 2, Math.min(sourceWidth, left + maxWidth));
    if (handle.includes("n")) top = clamp(top + dy, Math.max(0, bottom - maxHeight), bottom - 2);
    if (handle.includes("s")) bottom = clamp(bottom + dy, top + 2, Math.min(sourceHeight, top + maxHeight));
    let cropWidth = right - left;
    let cropHeight = bottom - top;
    const ratio = aspectRatio && Number.isFinite(aspectRatio) && aspectRatio > 0 ? aspectRatio : 0;
    if (ratio) {
      const horizontal = handle.includes("e") || handle.includes("w");
      const vertical = handle.includes("n") || handle.includes("s");
      if (horizontal && (!vertical || Math.abs(dx) >= Math.abs(dy) * ratio)) cropHeight = cropWidth / ratio;
      else cropWidth = cropHeight * ratio;
      const anchorX = handle.includes("w") ? initial.x + initial.width : handle.includes("e") ? initial.x : initial.x + initial.width / 2;
      const anchorY = handle.includes("n") ? initial.y + initial.height : handle.includes("s") ? initial.y : initial.y + initial.height / 2;
      const availableWidth = horizontal ? (handle.includes("w") ? anchorX : sourceWidth - anchorX) : 2 * Math.min(anchorX, sourceWidth - anchorX);
      const availableHeight = vertical ? (handle.includes("n") ? anchorY : sourceHeight - anchorY) : 2 * Math.min(anchorY, sourceHeight - anchorY);
      const shrink = Math.min(1, Math.min(maxWidth, availableWidth) / cropWidth, Math.min(maxHeight, availableHeight) / cropHeight);
      cropWidth *= shrink;
      cropHeight *= shrink;
      cropWidth = even(cropWidth);
      cropHeight = even(cropHeight);
      left = handle.includes("w") ? anchorX - cropWidth : handle.includes("e") ? anchorX : anchorX - cropWidth / 2;
      top = handle.includes("n") ? anchorY - cropHeight : handle.includes("s") ? anchorY : anchorY - cropHeight / 2;
    }
    const resultWidth = Math.min(even(cropWidth), Math.floor(maxWidth / 2) * 2);
    const resultHeight = Math.min(even(cropHeight), Math.floor(maxHeight / 2) * 2);
    return { x: Math.round(clamp(left, 0, sourceWidth - resultWidth)), y: Math.round(clamp(top, 0, sourceHeight - resultHeight)), width: resultWidth, height: resultHeight };
  }

  function start(event: React.PointerEvent<HTMLElement>, handle: Handle) {
    if (!active || !crop || event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    event.currentTarget.focus();
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { handle, x: event.clientX, y: event.clientY, crop: { ...crop }, scale: (frame.current?.getBoundingClientRect().width || sourceWidth) / sourceWidth, pointer: event.pointerId };
  }

  function move(event: React.PointerEvent<HTMLElement>) {
    const current = drag.current;
    if (!current || current.pointer !== event.pointerId) return;
    event.preventDefault();
    event.stopPropagation();
    onChange(resize(current.crop, current.handle, (event.clientX - current.x) / current.scale, (event.clientY - current.y) / current.scale));
  }

  function stop(event: React.PointerEvent<HTMLElement>) {
    if (drag.current?.pointer !== event.pointerId) return;
    drag.current = undefined;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  }

  function nudge(event: React.KeyboardEvent<HTMLElement>, handle: Handle) {
    if (!crop || !["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
    event.preventDefault();
    event.stopPropagation();
    const step = event.shiftKey ? 10 : handle === "move" ? 1 : 2;
    const dx = event.key === "ArrowLeft" ? -step : event.key === "ArrowRight" ? step : 0;
    const dy = event.key === "ArrowUp" ? -step : event.key === "ArrowDown" ? step : 0;
    onChange(resize(crop, handle, dx, dy));
  }

  return <div className="crop-preview" ref={viewport}>
    {src && <div ref={frame} className="crop-preview-frame" style={{ width: width * scale, height: height * scale }}>
      <img src={src} alt={editable ? "源视频画面与裁切范围" : "视频静帧预览"} draggable={false} onLoad={(event) => setNatural({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight })} />
      {active && crop && <div className="crop-preview-selection" role="group" aria-label="裁切框" tabIndex={0}
        style={{ left: `${crop.x / sourceWidth * 100}%`, top: `${crop.y / sourceHeight * 100}%`, width: `${crop.width / sourceWidth * 100}%`, height: `${crop.height / sourceHeight * 100}%` }}
        onPointerDown={(event) => start(event, "move")} onPointerMove={move} onPointerUp={stop} onPointerCancel={stop} onLostPointerCapture={() => { drag.current = undefined; }} onKeyDown={(event) => nudge(event, "move")}>
        {handles.map((handle) => <button key={handle} type="button" className={`crop-preview-handle handle-${handle}`} aria-label={`拖动裁切${names[handle]}`} title={`拖动裁切${names[handle]}`} onPointerDown={(event) => start(event, handle)} onPointerMove={move} onPointerUp={stop} onPointerCancel={stop} onKeyDown={(event) => nudge(event, handle)} />)}
      </div>}
    </div>}
  </div>;
}
