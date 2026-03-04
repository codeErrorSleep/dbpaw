"use client";

import { Toaster as Sonner, ToasterProps } from "sonner";
import { useTheme } from "@/components/theme-provider";
import { getThemeAppearance } from "@/theme/themeRegistry";

const Toaster = ({ ...props }: ToasterProps) => {
  const { theme } = useTheme();
  const appearance = getThemeAppearance(theme);

  return (
    <Sonner
      theme={appearance as ToasterProps["theme"]}
      position="top-right"
      className="toaster group"
      closeButton
      duration={5000}
      visibleToasts={4}
      style={
        {
          "--normal-bg": "var(--popover)",
          "--normal-text": "var(--popover-foreground)",
          "--normal-border": "var(--border)",
        } as React.CSSProperties
      }
      {...props}
    />
  );
};

export { Toaster };
