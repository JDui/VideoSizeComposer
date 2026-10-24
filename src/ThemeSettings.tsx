import { contrastInk, defaultTheme, themeFields, themePresets, type ThemeColors } from "./theme";
import { Check } from "lucide-react";

export function ThemeSettings({ colors, onChange }: { colors: ThemeColors; onChange: (colors: ThemeColors) => void }) {
  const selected = themePresets.find((preset) => themeFields.every(({ key }) => preset.colors[key].toLowerCase() === colors[key].toLowerCase()));
  return <section className="theme-settings" aria-label="界面配色">
    <div className="theme-heading"><h3>界面配色</h3><span>{selected?.name ?? "自定义配色"}</span></div>
    <p className="field-note">选择预设后仍可自由搭配。实时预览，保存设置后生效；关闭窗口可放弃修改。</p>
    <div className="theme-presets">{themePresets.map((preset) => <button key={preset.id} type="button" aria-pressed={selected?.id === preset.id} className="theme-preset" onClick={() => onChange({ ...preset.colors })}>
      <span className="theme-swatches" aria-hidden="true" style={{ background: preset.colors.background }}><span style={{ background: preset.colors.surface, color: preset.colors.text }}>Aa</span><i style={{ background: preset.colors.accent }} /><i style={{ background: preset.colors.action }} /></span>
      <span className="theme-preset-name">{preset.name}<Check size={13} aria-hidden="true" /></span>
    </button>)}</div>
    <div className="theme-color-grid">{themeFields.map(({ key, label }) => <label className="theme-color-field" key={key}><span>{label}</span><span><input type="color" aria-label={label} value={colors[key]} onChange={(event) => onChange({ ...colors, [key]: event.target.value })} /><code>{colors[key].toUpperCase()}</code></span></label>)}</div>
    <div className="theme-preview" aria-label="配色预览"><span><strong>VideoSize Composer</strong><small>面板与文字预览</small></span><span style={{ color: colors.accent }}>已选中</span><span className="theme-preview-action" style={{ background: colors.action, color: contrastInk(colors.action) }}>开始编码</span></div>
    <button type="button" className="quiet-action" onClick={() => onChange({ ...defaultTheme })}>恢复爱蜜莉雅紫</button>
  </section>;
}
