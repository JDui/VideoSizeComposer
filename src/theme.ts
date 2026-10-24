export interface ThemeColors {
  background: string;
  surface: string;
  text: string;
  muted: string;
  accent: string;
  action: string;
}

export const themeFields: { key: keyof ThemeColors; label: string }[] = [
  { key: "background", label: "窗口背景" }, { key: "surface", label: "面板底色" },
  { key: "text", label: "主要文字" }, { key: "muted", label: "次要文字" },
  { key: "accent", label: "强调与选中" }, { key: "action", label: "操作按钮" }
];

export const themePresets: { id: string; name: string; colors: ThemeColors }[] = [
  { id: "emilia", name: "爱蜜莉雅紫", colors: { background: "#111116", surface: "#202027", text: "#f0eff4", muted: "#aaa7b5", accent: "#c2ace9", action: "#b69cde" } },
  { id: "ocean", name: "深海青蓝", colors: { background: "#10151b", surface: "#1c252e", text: "#edf2f7", muted: "#a0afbd", accent: "#8cc8e0", action: "#8abbd6" } },
  { id: "sakura", name: "樱花暮色", colors: { background: "#181417", surface: "#292227", text: "#f5eff2", muted: "#b7a6ae", accent: "#e6b1c4", action: "#dca3ba" } },
  { id: "forest", name: "林间薄荷", colors: { background: "#111714", surface: "#1f2924", text: "#eef4f0", muted: "#a1b2a8", accent: "#a2cbb8", action: "#94bfa7" } },
  { id: "sunset", name: "落日琥珀", colors: { background: "#181613", surface: "#2b2620", text: "#f5f0e9", muted: "#b8ad9e", accent: "#dfbf91", action: "#d4af7e" } },
  { id: "silver", name: "银白紫罗兰", colors: { background: "#edeef2", surface: "#ffffff", text: "#292832", muted: "#686676", accent: "#76579f", action: "#7958a4" } }
];

export const defaultTheme = themePresets[0].colors;

// Upgrade exact built-in palettes from the first theme release; preserve custom mixes.
const originalPalettes = [
  ["#151020", "#251d35", "#f4efff", "#b5a7c9", "#c4a1ff", "#b494ef"],
  ["#08141b", "#132b37", "#e7eef2", "#8fa5ae", "#54c8e8", "#69dda2"],
  ["#1c1119", "#32212e", "#fff0f5", "#c2a3b3", "#ffa9cc", "#ee9fb9"],
  ["#0d1814", "#1b3028", "#edf8f1", "#9bb6a8", "#87d9b0", "#b6d88c"],
  ["#1c1510", "#33271e", "#fff4e8", "#c1ad98", "#f5be7a", "#efaa86"],
  ["#eeebf5", "#faf8ff", "#292136", "#71657f", "#7650ac", "#7954b0"]
];

export function normalizeTheme(value: unknown): ThemeColors {
  const colors = value && typeof value === "object" ? value as Partial<ThemeColors> : {};
  const original = originalPalettes.findIndex((palette) => themeFields.every(({ key }, index) => colors[key]?.toLowerCase() === palette[index]));
  if (original >= 0) return { ...themePresets[original].colors };
  return Object.fromEntries(themeFields.map(({ key }) => [key,
    typeof colors[key] === "string" && /^#[0-9a-f]{6}$/i.test(colors[key]!) ? colors[key] : defaultTheme[key]
  ])) as unknown as ThemeColors;
}

export function contrastInk(hex: string): string {
  const rgb = hex.slice(1).match(/../g)!.map((part) => {
    const v = parseInt(part, 16) / 255;
    return v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4;
  });
  return rgb[0] * .2126 + rgb[1] * .7152 + rgb[2] * .0722 > .179 ? "#16121d" : "#ffffff";
}

export function applyTheme(colors: ThemeColors) {
  const root = document.documentElement;
  for (const { key } of themeFields) root.style.setProperty(`--theme-${key}`, colors[key]);
  root.style.setProperty("--action-ink", contrastInk(colors.action));
  root.style.setProperty("--accent-ink", contrastInk(colors.accent));
  const dark = contrastInk(colors.background) === "#ffffff";
  root.style.colorScheme = dark ? "dark" : "light";
  root.style.setProperty("--success", dark ? "#98ccb0" : "#33714f");
  root.style.setProperty("--amber", dark ? "#dfbf91" : "#89612f");
  root.style.setProperty("--danger", dark ? "#e799a0" : "#af424e");
}
