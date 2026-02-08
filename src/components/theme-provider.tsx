import { createContext, useContext, useEffect, useState } from "react";
import { getSetting, saveSetting } from "@/services/store";

type Theme = "dark" | "light" | "system";

interface ThemeProviderState {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  accentColor: string;
  setAccentColor: (color: string) => void;
}

const initialState: ThemeProviderState = {
  theme: "system",
  setTheme: () => null,
  accentColor: "Zinc",
  setAccentColor: () => null,
};

const ThemeProviderContext = createContext<ThemeProviderState>(initialState);

// Define theme colors mapping for accent colors
const THEME_COLORS_MAP: Record<string, { light: string; dark: string }> = {
  Zinc: { light: "#09090b", dark: "#fafafa" },
  Blue: { light: "#2563eb", dark: "#3b82f6" },
  Violet: { light: "#7c3aed", dark: "#8b5cf6" },
  Green: { light: "#16a34a", dark: "#22c55e" },
  Orange: { light: "#ea580c", dark: "#f97316" },
};

export function ThemeProvider({
  children,
  defaultTheme = "system",
  ...props
}: {
  children: React.ReactNode;
  defaultTheme?: Theme;
}) {
  const [theme, setThemeState] = useState<Theme>(defaultTheme);
  const [accentColor, setAccentColorState] = useState<string>("Zinc");
  const [isLoaded, setIsLoaded] = useState(false);

  // Helper to apply theme to DOM
  const applyTheme = (t: Theme) => {
    const root = document.documentElement;
    root.classList.remove("light", "dark");

    if (t === "system") {
      const systemTheme = window.matchMedia("(prefers-color-scheme: dark)")
        .matches
        ? "dark"
        : "light";
      root.classList.add(systemTheme);
    } else {
      root.classList.add(t);
    }
  };

  // Helper to apply accent color
  const applyAccentColor = (colorName: string, currentTheme: Theme) => {
    const color = THEME_COLORS_MAP[colorName];
    if (!color) return;

    const root = document.documentElement;
    const isDark =
      currentTheme === "dark" ||
      (currentTheme === "system" &&
        window.matchMedia("(prefers-color-scheme: dark)").matches);

    const colorValue = isDark ? color.dark : color.light;
    root.style.setProperty("--primary", colorValue);
    root.style.setProperty("--ring", colorValue);
  };

  // 1. Load settings on mount
  useEffect(() => {
    const loadSettings = async () => {
      const savedTheme = await getSetting<Theme>("theme", defaultTheme);
      const savedAccent = await getSetting<string>("accentColor", "Zinc");
      
      setThemeState(savedTheme);
      setAccentColorState(savedAccent);
      
      // Initial application
      applyTheme(savedTheme);
      applyAccentColor(savedAccent, savedTheme);
      
      setIsLoaded(true);
    };

    loadSettings();
  }, [defaultTheme]);

  // 2. Listen for system theme changes
  useEffect(() => {
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => {
      if (theme === "system") {
        applyTheme("system");
        applyAccentColor(accentColor, "system");
      }
    };

    mediaQuery.addEventListener("change", handleChange);
    return () => mediaQuery.removeEventListener("change", handleChange);
  }, [theme, accentColor]);

  const setTheme = (t: Theme) => {
    setThemeState(t);
    applyTheme(t);
    applyAccentColor(accentColor, t); // Re-apply accent because it depends on light/dark
    saveSetting("theme", t);
  };

  const setAccentColor = (color: string) => {
    setAccentColorState(color);
    applyAccentColor(color, theme);
    saveSetting("accentColor", color);
  };

  if (!isLoaded) {
    // Return null or a loader to prevent flash of wrong theme
    return null; 
  }

  const value = {
    theme,
    setTheme,
    accentColor,
    setAccentColor,
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
