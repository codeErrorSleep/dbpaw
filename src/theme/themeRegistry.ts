export type ThemeId =
  | "default"
  | "one-dark"
  | "github"
  | "nord"
  | "dracula";

export type ThemeAppearance = "light" | "dark";

export interface ThemePreset {
  id: ThemeId;
  label: string;
  appearance: ThemeAppearance;
  editorTheme: "default" | "one-dark";
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
  github: {
    id: "github",
    label: "GitHub",
    appearance: "light",
    editorTheme: "default",
  },
  nord: {
    id: "nord",
    label: "Nord",
    appearance: "dark",
    editorTheme: "one-dark",
  },
  dracula: {
    id: "dracula",
    label: "Dracula",
    appearance: "dark",
    editorTheme: "one-dark",
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
