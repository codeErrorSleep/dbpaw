import { createContext, useContext, useEffect, useState } from "react";
import { getSetting, saveSetting } from "@/services/store";
import {
  ThemeId,
  getThemeAppearance,
  normalizeThemeId,
} from "@/theme/themeRegistry";

export type Theme = ThemeId;
export const MIN_FONT_SIZE_PX = 10;
export const MAX_FONT_SIZE_PX = 24;
export const DEFAULT_FONT_SIZE_PX = 14;

interface ThemeProviderState {
  theme: ThemeId;
  setTheme: (theme: ThemeId) => void;
  accentColor: string;
  setAccentColor: (color: string) => void;
  fontSizePx: number;
  setFontSizePx: (size: number) => void;
}

const initialState: ThemeProviderState = {
  theme: "default",
  setTheme: () => null,
  accentColor: "Zinc",
  setAccentColor: () => null,
  fontSizePx: DEFAULT_FONT_SIZE_PX,
  setFontSizePx: () => null,
};

const ThemeProviderContext = createContext<ThemeProviderState>(initialState);

const THEME_COLORS_MAP: Record<string, { light: string; dark: string }> = {
  Zinc: { light: "#09090b", dark: "#fafafa" },
  Blue: { light: "#2563eb", dark: "#3b82f6" },
  Violet: { light: "#7c3aed", dark: "#8b5cf6" },
  Green: { light: "#16a34a", dark: "#22c55e" },
  Orange: { light: "#ea580c", dark: "#f97316" },
};

export function ThemeProvider({
  children,
  defaultTheme = "default",
  ...props
}: {
  children: React.ReactNode;
  defaultTheme?: ThemeId;
}) {
  const [theme, setThemeState] = useState<ThemeId>(defaultTheme);
  const [accentColor, setAccentColorState] = useState<string>("Zinc");
  const [fontSizePx, setFontSizePxState] =
    useState<number>(DEFAULT_FONT_SIZE_PX);
  const [isLoaded, setIsLoaded] = useState(false);

  const clampFontSize = (size: number) => {
    if (!Number.isFinite(size)) {
      return DEFAULT_FONT_SIZE_PX;
    }

    const rounded = Math.round(size);
    return Math.min(MAX_FONT_SIZE_PX, Math.max(MIN_FONT_SIZE_PX, rounded));
  };

  const applyTheme = (themeId: ThemeId) => {
    const root = document.documentElement;
    const appearance = getThemeAppearance(themeId);

    root.setAttribute("data-theme", themeId);
    root.classList.remove("light", "dark");
    root.classList.add(appearance);
    root.style.colorScheme = appearance;
  };

  const applyAccentColor = (colorName: string, currentTheme: ThemeId) => {
    const color = THEME_COLORS_MAP[colorName];
    if (!color) return;

    const root = document.documentElement;
    const appearance = getThemeAppearance(currentTheme);
    const colorValue = appearance === "dark" ? color.dark : color.light;
    root.style.setProperty("--primary", colorValue);
    root.style.setProperty("--ring", colorValue);
  };

  const applyFontSizePx = (size: number) => {
    const root = document.documentElement;
    root.style.setProperty("--font-size", `${size}px`);
  };

  useEffect(() => {
    const loadSettings = async () => {
      const rawTheme = await getSetting<string>("theme", defaultTheme);
      const savedTheme = normalizeThemeId(rawTheme);
      const savedAccent = await getSetting<string>("accentColor", "Zinc");
      const savedFontSize = await getSetting<number>(
        "fontSizePx",
        DEFAULT_FONT_SIZE_PX,
      );
      const normalizedFontSize = clampFontSize(savedFontSize);

      setThemeState(savedTheme);
      setAccentColorState(savedAccent);
      setFontSizePxState(normalizedFontSize);

      applyTheme(savedTheme);
      applyAccentColor(savedAccent, savedTheme);
      applyFontSizePx(normalizedFontSize);

      if (savedTheme !== rawTheme) {
        void saveSetting("theme", savedTheme);
      }

      setIsLoaded(true);
    };

    void loadSettings();
  }, [defaultTheme]);

  const setTheme = (themeId: ThemeId) => {
    setThemeState(themeId);
    applyTheme(themeId);
    applyAccentColor(accentColor, themeId);
    void saveSetting("theme", themeId);
  };

  const setAccentColor = (color: string) => {
    setAccentColorState(color);
    applyAccentColor(color, theme);
    void saveSetting("accentColor", color);
  };

  const setFontSizePx = (size: number) => {
    const normalizedSize = clampFontSize(size);
    setFontSizePxState(normalizedSize);
    applyFontSizePx(normalizedSize);
    void saveSetting("fontSizePx", normalizedSize);
  };

  if (!isLoaded) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-background">
        <div className="text-muted-foreground text-sm">Loading...</div>
      </div>
    );
  }

  const value = {
    theme,
    setTheme,
    accentColor,
    setAccentColor,
    fontSizePx,
    setFontSizePx,
  };

  return (
    <ThemeProviderContext.Provider {...props} value={value}>
      {children}
    </ThemeProviderContext.Provider>
  );
}

export const useTheme = () => {
  const context = useContext(ThemeProviderContext);

  if (context === undefined)
    throw new Error("useTheme must be used within a ThemeProvider");

  return context;
};
