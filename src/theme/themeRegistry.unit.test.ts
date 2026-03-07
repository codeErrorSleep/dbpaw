import { describe, expect, test } from "bun:test";
import {
  getThemeAppearance,
  getThemePreset,
  isThemeId,
  normalizeThemeId,
  THEME_PRESETS,
  type ThemeId,
} from "./themeRegistry";

describe("themeRegistry", () => {
  test("maps legacy github theme to github-light", () => {
    expect(normalizeThemeId("github")).toBe("github-light");
  });

  test("recognizes all preset keys as valid theme ids", () => {
    const ids = Object.keys(THEME_PRESETS) as ThemeId[];
    for (const id of ids) {
      expect(isThemeId(id)).toBe(true);
    }
  });

  test("returns correct appearance for new themes", () => {
    expect(getThemeAppearance("github-light")).toBe("light");
    expect(getThemeAppearance("github-dark")).toBe("dark");
    expect(getThemeAppearance("monokai-pro")).toBe("dark");
    expect(getThemeAppearance("night-owl")).toBe("dark");
    expect(getThemeAppearance("shades-of-purple")).toBe("dark");
    expect(getThemeAppearance("palenight")).toBe("dark");
    expect(getThemeAppearance("cyberpunk")).toBe("dark");
  });

  test("binds editorTheme to preset theme id", () => {
    const ids = Object.keys(THEME_PRESETS) as ThemeId[];
    for (const id of ids) {
      expect(getThemePreset(id).editorTheme).toBe(id);
    }
  });
});
