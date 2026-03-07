export type ThemeId =
  | "default"
  | "one-dark"
  | "github-light"
  | "github-dark"
  | "monokai-pro"
  | "night-owl"
  | "shades-of-purple"
  | "palenight"
  | "cyberpunk"
  | "nord"
  | "dracula";

export type ThemeAppearance = "light" | "dark";
export type EditorThemeKey = ThemeId;

export interface ThemePreset {
  id: ThemeId;
  label: string;
  appearance: ThemeAppearance;
  editorTheme: EditorThemeKey;
}

export const THEME_PRESETS: Record<ThemeId, ThemePreset> = {
  default: {
    id: "default",
    label: "Default",
    appearance: "light",
    editorTheme: "default",
  },
  "one-dark": {
    id: "one-dark",
    label: "One Dark",
    appearance: "dark",
    editorTheme: "one-dark",
  },
  "github-light": {
    id: "github-light",
    label: "GitHub Light",
    appearance: "light",
    editorTheme: "github-light",
  },
  "github-dark": {
    id: "github-dark",
    label: "GitHub Dark",
    appearance: "dark",
    editorTheme: "github-dark",
  },
  "monokai-pro": {
    id: "monokai-pro",
    label: "Monokai Pro",
    appearance: "dark",
    editorTheme: "monokai-pro",
  },
  "night-owl": {
    id: "night-owl",
    label: "Night Owl",
    appearance: "dark",
    editorTheme: "night-owl",
  },
  "shades-of-purple": {
    id: "shades-of-purple",
    label: "Shades of Purple",
    appearance: "dark",
    editorTheme: "shades-of-purple",
  },
  palenight: {
    id: "palenight",
    label: "Palenight",
    appearance: "dark",
    editorTheme: "palenight",
  },
  cyberpunk: {
    id: "cyberpunk",
    label: "Cyberpunk",
    appearance: "dark",
    editorTheme: "cyberpunk",
  },
  nord: {
    id: "nord",
    label: "Nord",
    appearance: "dark",
    editorTheme: "nord",
  },
  dracula: {
    id: "dracula",
    label: "Dracula",
    appearance: "dark",
    editorTheme: "dracula",
  },
};

const LEGACY_THEME_VALUES = new Set(["light", "dark", "system"]);

export function isThemeId(value: string): value is ThemeId {
  return value in THEME_PRESETS;
}

export function normalizeThemeId(rawValue: unknown): ThemeId {
  if (typeof rawValue !== "string") {
    return "default";
  }

  if (rawValue === "github") {
    return "github-light";
  }

  if (isThemeId(rawValue)) {
    return rawValue;
  }

  if (LEGACY_THEME_VALUES.has(rawValue)) {
    return "default";
  }

  return "default";
}

export function getThemePreset(themeId: ThemeId): ThemePreset {
  return THEME_PRESETS[themeId] ?? THEME_PRESETS.default;
}

export function getThemeAppearance(themeId: ThemeId): ThemeAppearance {
  return getThemePreset(themeId).appearance;
}
