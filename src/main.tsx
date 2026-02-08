import { createRoot } from "react-dom/client";
import App from "./App.tsx";
import "./styles/index.css";
import { ThemeProvider } from "./components/theme-provider";

createRoot(document.getElementById("root")!).render(
  <ThemeProvider defaultTheme="system" enableSystem>
    <App />
  </ThemeProvider>
);
