import { useId, useState } from "react";
import { CheckCircle2, ChevronDown, Clock3, FilePlus2, FolderOpen, ShieldCheck } from "lucide-react";
import type { AlphaBackground, Codec, FpsMode, Hardware, PlatformInfo, Preset } from "./types";

export const codecLabels: Record<Codec, string> = {
  h265: "HEVC (H.265)", h264: "H.264 (AVC)", av1: "AV1", prores: "ProRes 422 LT"
};

const tabs = [{ id: "common", label: "常用" }, { id: "color", label: "色彩" }, { id: "panorama", label: "全景" }, { id: "output", label: "输出" }] as const;
type SettingsTab = typeof tabs[number]["id"];
const commonFpsOptions = [23.976, 24, 25, 29.97, 30, 50, 59.94, 60];

export function presetSummary(preset: Preset) {
  if (preset.outputContainer === "mp3") return "MP3 · 320 kbps · 仅音频";
  if (preset.outputContainer === "m4a") return "M4A · AAC 320 kbps · 仅音频";
  if (preset.outputContainer === "wav") return `WAV · ${preset.wavEncoding === "float32" ? "32-bit Float" : preset.wavEncoding === "pcm24" ? "24-bit PCM" : "16-bit PCM"} · 仅音频`;
  const size = preset.resolutionMode === "source" ? "原尺寸"
    : preset.resolutionMode === "short_edge" ? `短边 ${preset.shortEdge}p`
    : preset.resolutionMode === "custom" ? `${preset.customWidth}×${preset.customHeight}` : `${preset.scalePercent}% 尺寸`;
  const color = preset.hdrMode === "source" ? "跟随源色彩" : preset.hdrMode === "dolby_vision" ? "Dolby Vision" : preset.hdrMode.toUpperCase();
  return `${size} · ${color}`;
}

export function OutputSettings({ preset, platform, onChange, onChooseLut, onChooseOutput }: {
  preset: Preset;
  platform: PlatformInfo;
  onChange: (patch: Partial<Preset>) => void;
  onChooseLut: () => void;
  onChooseOutput: () => void;
}) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("common");
  const uid = useId();
  const audioOnly = ["m4a", "wav", "mp3"].includes(preset.outputContainer);
  const streamCopy = preset.hdrMode === "dolby_vision";
  const targetRate = preset.bitrateMode === "target_mbps";
  const rate = targetRate ? preset.targetBitrateMbps : Number((preset.bitrateMultiplier * 100).toFixed(2));
  const updateRate = (value: number) => onChange(targetRate ? { targetBitrateMbps: value } : { bitrateMultiplier: value / 100 });
  const resolution = preset.resolutionMode === "source" ? "source" : preset.resolutionMode === "short_edge" ? String(preset.shortEdge) : preset.resolutionMode;

  return <div className="output-settings">
    <div className="output-tabs" role="tablist" aria-label="输出设置分类">
      {tabs.map((tab, index) => <button key={tab.id} id={`${uid}-${tab.id}-tab`} type="button" role="tab"
        aria-selected={activeTab === tab.id} aria-controls={`${uid}-${tab.id}-panel`} tabIndex={activeTab === tab.id ? 0 : -1}
        onClick={() => setActiveTab(tab.id)} onKeyDown={(event) => {
          let next = index;
          if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
          else if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
          else if (event.key === "Home") next = 0;
          else if (event.key === "End") next = tabs.length - 1;
          else return;
          event.preventDefault();
          setActiveTab(tabs[next].id);
          event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
        }}>{tab.label}{tab.id === "color" && !audioOnly && (preset.lutEnabled || !["source", "sdr"].includes(preset.hdrMode) || preset.toneMap !== "auto") && <span className="tab-indicator" aria-label="已启用色彩处理" />}</button>)}
    </div>
    {tabs.map((tab) => <div key={tab.id} id={`${uid}-${tab.id}-panel`} className="output-tabpanel" role="tabpanel"
      aria-labelledby={`${uid}-${tab.id}-tab`} hidden={activeTab !== tab.id} tabIndex={0}>
      {tab.id === "common" && <>
        <section className="output-group"><h3>编码格式</h3><div className="settings-pair">
          <Field label="视频编码"><select disabled={audioOnly} title={audioOnly ? "仅音频输出不使用视频编码" : codecLabels[preset.codec]} value={preset.codec} onChange={(event) => onChange(event.target.value === "prores"
            ? { codec: "prores", bitDepth: 10, chroma: "422", hardware: "cpu", outputContainer: "mov" }
            : { codec: event.target.value as Codec })}>{Object.entries(codecLabels).map(([value, label]) => <option key={value} value={value}>{value === "h265" ? "H.265" : value === "h264" ? "H.264" : value === "prores" ? "ProRes LT" : label}</option>)}</select></Field>
          <Field label="封装格式"><select value={preset.outputContainer} onChange={(event) => onChange({ outputContainer: event.target.value as Preset["outputContainer"] })}>
            <option value="source">跟随源文件</option>{["mp4", "mov", "avi", "mkv", "webm", "m4v"].map((value) => <option key={value} value={value}>{value.toUpperCase()}</option>)}<option value="m4a">M4A（仅音频）</option><option value="wav">WAV（仅音频）</option><option value="mp3">MP3（仅音频 · 320 kbps）</option>
          </select></Field>
        </div>{(preset.outputContainer === "source" || audioOnly) && <small className="field-note">{preset.outputContainer === "source" ? "INSV 默认输出 MP4（ProRes 使用 MOV）；序列帧和音频默认使用 MP4。" : preset.outputContainer === "mp3" ? "使用 MP3 编码，以 320 kbps 导出音频。源文件需包含音轨，或为任务附加外接音频。" : "仅导出音频，不包含视频画面。"}</small>}</section>
        {preset.outputContainer === "wav" && <section className="output-group"><h3>WAV 音频</h3>
          <Field label="采样格式"><select value={preset.wavEncoding} onChange={(event) => onChange({ wavEncoding: event.target.value as Preset["wavEncoding"] })}>
            <option value="pcm16">16-bit PCM（默认，优先兼容）</option><option value="pcm24">24-bit PCM（后期处理）</option><option value="float32">32-bit Float（浮点）</option>
          </select></Field><small className="field-note">遇到导入兼容问题时优先选择 16-bit PCM。部分软件不支持 32-bit Float；提高位深不会恢复源文件已丢失的细节。</small>
        </section>}
        {!audioOnly && <><section className="output-group"><h3>画面尺寸</h3><div className="settings-pair">
          <Field label="分辨率"><select disabled={streamCopy} value={resolution} onChange={(event) => {
            const value = event.target.value;
            onChange(value === "source" || value === "scale_percent" || value === "custom" ? { resolutionMode: value } : { resolutionMode: "short_edge", shortEdge: Number(value) });
          }}><option value="source">保持原尺寸</option><option value="1080">短边 1080p</option><option value="720">短边 720p</option><option value="scale_percent">按比例缩放</option><option value="custom">自定义尺寸</option></select></Field>
          <Field label="帧率"><select disabled={streamCopy} value={preset.fpsMode} onChange={(event) => onChange({ fpsMode: event.target.value as FpsMode })}><option value="source">保持原帧率</option><option value="preset">常见帧率</option><option value="custom">自定义帧率</option></select></Field>
        </div>
        {preset.resolutionMode === "scale_percent" && <label className="range-setting"><span>缩放倍率 <strong>{preset.scalePercent}%</strong></span><input type="range" min={10} max={90} value={preset.scalePercent} onChange={(event) => onChange({ scalePercent: Number(event.target.value) })} /></label>}
        {preset.resolutionMode === "custom" && <div className="dimension-grid"><Field label="宽"><input type="number" min={2} value={preset.customWidth} onChange={(event) => onChange({ customWidth: Number(event.target.value) })} /></Field><span>×</span><Field label="高"><input type="number" min={2} value={preset.customHeight} onChange={(event) => onChange({ customHeight: Number(event.target.value) })} /></Field></div>}
        {preset.fpsMode === "preset" && <Field label="目标帧率"><select value={preset.fpsValue} onChange={(event) => onChange({ fpsValue: Number(event.target.value) })}>{commonFpsOptions.map((fps) => <option key={fps} value={fps}>{fps} fps</option>)}</select></Field>}
        {preset.fpsMode === "custom" && <Field label="目标帧率 (fps)"><input type="number" min={0.001} step={0.001} value={preset.fpsValue} onChange={(event) => onChange({ fpsValue: Number(event.target.value) })} /></Field>}
        </section>
        {streamCopy ? <div className="source-color-note"><ShieldCheck size={15} />杜比视界直接复制源视频流，保留 RPU；尺寸、帧率与码率保持源文件。</div>
          : preset.codec === "prores" ? <div className="source-color-note"><ShieldCheck size={15} />ProRes 422 LT 使用固定编码档位，适合后期编辑。</div>
          : <section className="output-group"><h3>压缩画质</h3>
            <Field label="码率模式"><select value={preset.bitrateMode} onChange={(event) => onChange({ bitrateMode: event.target.value as Preset["bitrateMode"] })}><option value="source_multiplier">智能压缩 · 按源码率比例</option><option value="target_mbps">指定目标码率</option></select></Field>
            <div className="rate-control"><span>{targetRate ? "目标码率" : "源码率比例"}</span><label><input aria-label={targetRate ? "目标码率 Mbps" : "源码率比例 %"} type="number" min={targetRate ? .1 : 1} max={targetRate ? undefined : 95} step={.1} value={rate} onChange={(event) => updateRate(Number(event.target.value))} /><span>{targetRate ? "Mbps" : "%"}</span></label></div>
            <input className="quality-range" aria-label="调整压缩码率" type="range" min={targetRate ? .1 : 1} max={targetRate ? 150 : 95} step={targetRate ? .1 : 1} value={Math.min(targetRate ? 150 : 95, Math.max(targetRate ? .1 : 1, rate))} onChange={(event) => updateRate(Number(event.target.value))} />
            <div className="range-hints"><span>体积更小</span><span>画质更高</span></div>
          </section>}
        <details className="encoding-details"><summary><span>编码加速</span><small>{preset.hardware === "auto" ? "自动选择" : preset.hardware.toUpperCase()}</small><ChevronDown size={14} /></summary>
          <Field label="硬件"><select value={preset.hardware} onChange={(event) => onChange({ hardware: event.target.value as Hardware })}>{platform.accelerators.map((hardware) => <option key={hardware} value={hardware}>{hardware === "auto" ? "自动选择" : hardware.toUpperCase()}</option>)}</select></Field>
          <label className="inline-check"><input type="checkbox" checked={preset.cpuFallback} onChange={(event) => onChange({ cpuFallback: event.target.checked })} /><span>硬件失败时回退 CPU</span></label>
        </details></>}
      </>}
      {(tab.id === "color" || tab.id === "panorama") && audioOnly && <div className="source-color-note"><ShieldCheck size={15} />仅音频输出无需画面、色彩、LUT 或全景处理。切换为视频封装后可继续设置。</div>}
      {tab.id === "color" && !audioOnly && <>
        <section className="output-group"><h3>色彩与 HDR</h3>
          <Field label="HDR 模式"><select value={preset.hdrMode} onChange={(event) => {
            const hdrMode = event.target.value as Preset["hdrMode"];
            onChange(hdrMode === "hlg" || hdrMode === "hdr10" ? { hdrMode, colorSpace: "rec2020", bitDepth: 10, hardware: "cpu", codec: preset.codec === "h264" ? "h265" : preset.codec }
              : hdrMode === "dolby_vision" ? { hdrMode, codec: "h265", bitDepth: 10, chroma: "420", resolutionMode: "source", lutEnabled: false, fpsMode: "source" }
              : { hdrMode, colorSpace: hdrMode === "sdr" ? "rec709" : "source" });
          }}><option value="source">跟随源文件</option><option value="sdr">SDR / Rec.709</option><option value="hlg">HLG / Rec.2100</option><option value="hdr10">HDR10 / PQ</option><option value="dolby_vision">杜比视界（保留源流）</option></select></Field>
          <Field label="色彩空间"><select value={preset.colorSpace} onChange={(event) => onChange({ colorSpace: event.target.value as Preset["colorSpace"] })}><option value="source">跟随源文件</option><option value="rec709">Rec.709</option><option value="rec2020">Rec.2020</option></select></Field>
          {preset.hdrMode === "sdr" && <Field label="色调映射"><select value={preset.toneMap} onChange={(event) => onChange({ toneMap: event.target.value as Preset["toneMap"] })}><option value="auto">自动（推荐）</option><option value="bt2408">BT.2408 直映射</option><option value="tonemap">软膝压缩高光</option><option value="clip">硬截顶</option></select></Field>}
          {preset.hdrMode === "sdr" && <small className="field-note">自动：HLG 按源亮度与编码域峰值选择直映射或软膝；PQ / 杜比视界始终压缩高光。</small>}
          <div className="settings-pair"><Field label="色深"><select value={preset.bitDepth} onChange={(event) => onChange({ bitDepth: event.target.value === "source" ? "source" : Number(event.target.value) as 8 | 10 })}><option value="source">跟随源文件</option><option value={8}>8-bit</option><option value={10}>10-bit</option></select></Field>
          <Field label="色度采样"><select value={preset.chroma} onChange={(event) => onChange({ chroma: event.target.value as Preset["chroma"] })}><option value="source">跟随源文件</option><option value="420">4:2:0</option><option value="422">4:2:2</option></select></Field></div>
          {streamCopy && <div className="source-color-note"><ShieldCheck size={15} />仅保留源文件已有的杜比视界，不生成新的元数据。</div>}
        </section>
        <section className="output-group"><h3>调色 LUT</h3><label className="inline-check"><input type="checkbox" disabled={streamCopy} checked={preset.lutEnabled} onChange={(event) => onChange({ lutEnabled: event.target.checked })} /><span>启用 LUT 调色</span></label>
          {preset.lutEnabled && <><button className="secondary-button full" title={preset.lutName} onClick={onChooseLut}><FilePlus2 size={15} />{preset.lutName ? shortPath(preset.lutName) : "选择 .cube / .3dl / .lut"}</button><label className="range-setting"><span>LUT 强度 <strong>{preset.lutIntensity}%</strong></span><input type="range" min={0} max={100} value={preset.lutIntensity} onChange={(event) => onChange({ lutIntensity: Number(event.target.value) })} /></label></>}
          {streamCopy && <small className="field-note">保留杜比视界源流时无法套用 LUT。</small>}
        </section>
        <section className="output-group"><h3>透明通道</h3><Field label="Alpha 背景"><select value={preset.alphaBackground} onChange={(event) => onChange({ alphaBackground: event.target.value as AlphaBackground })}><option value="checkerboard">棋盘格（默认）</option><option value="black">黑底</option><option value="white">白底</option></select></Field><small className="field-note">仅对含 Alpha 通道的素材合成背景。</small></section>
      </>}
      {tab.id === "panorama" && !audioOnly && <>
        <div className="source-color-note"><ShieldCheck size={15} />INSV 先拼接为 2:1 全景，再按“常用”和“色彩”设置输出。以下处理仅适用于 INSV 原片。</div>
        <section className="output-group"><h3>防抖与朝向</h3>
          <Field label="防抖模式"><select value={preset.panorama.stabilization} onChange={(event) => onChange({ panorama: { ...preset.panorama, stabilization: event.target.value as Preset["panorama"]["stabilization"] } })}>
            <option value="off">关闭防抖</option><option value="flowstate">FlowState 防抖</option><option value="direction_lock">FlowState + 方向锁定</option>
          </select></Field>
          <small className="field-note">方向锁定用于固定全景朝向；已拼接 MP4 不会重复应用防抖。</small>
        </section>
        <section className="output-group"><h3>镜头拼接</h3>
          <Field label="拼接算法"><select value={preset.panorama.stitching} onChange={(event) => onChange({ panorama: { ...preset.panorama, stitching: event.target.value as Preset["panorama"]["stitching"] } })}>
            <option value="optflow">光流拼接</option><option value="dynamicstitch">动态拼接</option><option value="aistitch">AI 拼接</option>
          </select></Field>
          <label className="inline-check"><input type="checkbox" checked={preset.panorama.stitchFusion} onChange={(event) => onChange({ panorama: { ...preset.panorama, stitchFusion: event.target.checked } })} /><span>镜头间色彩与亮度融合</span></label>
          <label className="inline-check"><input type="checkbox" checked={preset.panorama.denoise} onChange={(event) => onChange({ panorama: { ...preset.panorama, denoise: event.target.checked } })} /><span>SDK 视频降噪</span></label>
          <small className="field-note">AI 拼接和降噪需要匹配的 SDK 模型。自定义全景尺寸须保持 2:1，宽为 4 的倍数。</small>
        </section>
        <section className="output-group"><h3>全景标记</h3>
          <label className="inline-check"><input type="checkbox" checked={preset.keepPanorama} onChange={(event) => onChange({ keepPanorama: event.target.checked })} /><span>识别并保留全景元数据</span></label>
          <small className="field-note">MP4 / MOV / M4V 写入并验证标准全景标记。其他封装的全景识别取决于播放器。</small>
        </section>
        <small className="field-note">需要官方 Desktop Media SDK。在应用设置中指定 SDK 目录，或将完整 bin 内容放入程序旁的 insta360/。拼接会生成临时中间视频，任务结束后自动清理。</small>
      </>}
      {tab.id === "output" && <>
        <section className="output-group"><h3>保存位置</h3><Field label="输出策略"><select value={preset.outputMode} onChange={(event) => onChange({ outputMode: event.target.value as Preset["outputMode"] })}><option value="subfolder">原位置的子文件夹</option><option value="in_place">原文件所在位置</option><option value="single_folder">统一保存到指定目录</option></select></Field>
          {preset.outputMode === "single_folder" ? <button className="secondary-button full" title={preset.outputDir} onClick={onChooseOutput}><FolderOpen size={15} />{preset.outputDir ? shortPath(preset.outputDir) : "选择输出目录"}</button> : <small className="field-note">{preset.outputMode === "subfolder" ? "保存到源文件旁的 VideoSizeComposer 文件夹。" : "保存到源文件所在文件夹，重名时自动添加序号。"}</small>}
        </section>
        <section className="output-group"><h3>文件命名</h3><Field label="命名方式"><select value={preset.namingMode} onChange={(event) => onChange({ namingMode: event.target.value as Preset["namingMode"] })}><option value="original">保持原名</option><option value="suffix_prefix">添加前后缀</option></select></Field>
          {preset.namingMode === "suffix_prefix" && <div className="naming-grid"><Field label="前缀"><input value={preset.prefix} onChange={(event) => onChange({ prefix: event.target.value })} placeholder="可选" /></Field><Field label="后缀"><input value={preset.suffix} onChange={(event) => onChange({ suffix: event.target.value })} placeholder="_compressed" /></Field></div>}
          <div className="naming-preview"><small>命名示例</small><span>{preset.namingMode === "suffix_prefix" ? `${preset.prefix}原文件名${preset.suffix}` : "原文件名"}<em>{preset.outputContainer === "source" ? ".原后缀" : `.${preset.outputContainer}`}</em></span></div>
        </section>
        <section className="output-group"><h3>文件信息</h3><label className="time-preservation-card"><input type="checkbox" checked={preset.keepTimes} onChange={(event) => onChange({ keepTimes: event.target.checked })} /><Clock3 size={17} /><span><strong>保留原始时间</strong><small>恢复创建日期和修改时间</small></span><CheckCircle2 size={16} className={preset.keepTimes ? "is-on" : ""} /></label>
          {!audioOnly && <label className="inline-check"><input type="checkbox" checked={preset.keepPanorama} onChange={(event) => onChange({ keepPanorama: event.target.checked })} /><span>识别并保留全景元数据</span></label>}
        </section>
      </>}
    </div>)}
  </div>;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return <label className="setting-field"><span>{label}</span>{children}</label>;
}

function shortPath(path: string) {
  return path.length > 32 ? `…${path.slice(-31)}` : path;
}
