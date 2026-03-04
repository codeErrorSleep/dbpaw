import { createRoot } from "react-dom/client";
import App from "./App.tsx";
import "./styles/index.css";
import { ThemeProvider } from "./components/theme-provider";
import { Toaster } from "./components/ui/sonner";
import "./lib/i18n";
import { initI18nFromStore } from "./lib/i18n";

const renderApp = async () => {
  await initI18nFromStore();
  createRoot(document.getElementById("root")!).render(
    <ThemeProvider defaultTheme="default">
      <App />
      <Toaster />
    </ThemeProvider>,
  );
};

void renderApp();
