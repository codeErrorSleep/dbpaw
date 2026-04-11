import { createRoot } from "react-dom/client";
import App from "./App.tsx";
import "./styles/index.css";
import { ThemeProvider } from "./components/theme-provider";
import { Toaster } from "./components/ui/sonner";
import "./lib/i18n";
import { initI18nFromStore } from "./lib/i18n";
import { ErrorBoundary } from "./components/ErrorBoundary";

const renderApp = async () => {
  await initI18nFromStore();
  if (import.meta.env.PROD) {
    document.addEventListener("contextmenu", (event) => {
      const target = event.target as HTMLElement | null;
      const allowNative =
        event.altKey ||
        target?.closest('input, textarea, [contenteditable="true"]');
      if (!allowNative) {
        event.preventDefault();
      }
    });
  }
  createRoot(document.getElementById("root")!).render(
    <ErrorBoundary>
      <ThemeProvider defaultTheme="default">
        <App />
        <Toaster />
      </ThemeProvider>
    </ErrorBoundary>,
  );
};

void renderApp();
