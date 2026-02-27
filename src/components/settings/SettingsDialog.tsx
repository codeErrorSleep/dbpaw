import { Bot, Info, Palette, RefreshCw, Settings2 } from "lucide-react";
import {
  useTheme,
  Theme,
  MIN_FONT_SIZE_PX,
  MAX_FONT_SIZE_PX,
} from "@/components/theme-provider";
import { useState, useEffect } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getSetting, saveSetting } from "@/services/store";
import { AIProviderConfig, AIProviderType, api } from "@/services/api";
import { toast } from "sonner";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import packageJson from "../../../package.json";

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type SettingsSection = "general" | "ai" | "about";
type AIProviderPreset = {
  type: AIProviderType;
  label: string;
  baseUrl: string;
  model: string;
};

const THEME_COLORS = [
  { name: "Zinc", value: "#09090b" },
  { name: "Blue", value: "#3b82f6" },
  { name: "Violet", value: "#8b5cf6" },
  { name: "Green", value: "#22c55e" },
  { name: "Orange", value: "#f97316" },
];

const AI_PROVIDER_OPTIONS: AIProviderPreset[] = [
  {
    type: "openai",
    label: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    model: "gpt-4.1-mini",
  },
  {
    type: "anthropic",
    label: "Anthropic",
    baseUrl: "https://api.anthropic.com/v1",
    model: "claude-3-5-sonnet-20241022",
  },
  {
    type: "gemini",
    label: "Gemini",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai",
    model: "gemini-2.0-flash",
  },
  {
    type: "groq",
    label: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    model: "llama-3.3-70b-versatile",
  },
  {
    type: "deepseek",
    label: "DeepSeek",
    baseUrl: "https://api.deepseek.com/v1",
    model: "deepseek-chat",
  },
  {
    type: "qwen",
    label: "Qwen",
    baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    model: "qwen-plus",
  },
  {
    type: "kimi",
    label: "Kimi",
    baseUrl: "https://api.moonshot.cn/v1",
    model: "moonshot-v1-8k",
  },
  {
    type: "glm",
    label: "GLM",
    baseUrl: "https://open.bigmodel.cn/api/paas/v4",
    model: "glm-4-flash",
  },
  {
    type: "siliconflow",
    label: "SiliconFlow",
    baseUrl: "https://api.siliconflow.cn/v1",
    model: "Qwen/Qwen2.5-72B-Instruct",
  },
  {
    type: "openrouter",
    label: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    model: "openai/gpt-4o-mini",
  },
];

const AI_PROVIDER_OPTIONS_BY_TYPE = AI_PROVIDER_OPTIONS.reduce(
  (acc, item) => ({ ...acc, [item.type]: item }),
  {} as Record<string, AIProviderPreset>,
);

const GITHUB_URL = "https://github.com/codeErrorSleep/dbpaw";
const APP_VERSION = packageJson.version;

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const { theme, setTheme, accentColor, setAccentColor, fontSizePx, setFontSizePx } =
    useTheme();
  const [activeSection, setActiveSection] = useState<SettingsSection>("general");
  const [autoUpdate, setAutoUpdate] = useState(true);
  const [checking, setChecking] = useState(false);
  const [providers, setProviders] = useState<AIProviderConfig[]>([]);
  const [selectedProviderType, setSelectedProviderType] = useState<AIProviderType>(
    AI_PROVIDER_OPTIONS[0].type,
  );
  const [providerBaseUrl, setProviderBaseUrl] = useState(
    AI_PROVIDER_OPTIONS[0].baseUrl,
  );
  const [providerModel, setProviderModel] = useState(
    AI_PROVIDER_OPTIONS[0].model,
  );
  const [providerApiKey, setProviderApiKey] = useState("");
  const [fontSizeInput, setFontSizeInput] = useState(String(fontSizePx));

  const clampFontSize = (size: number) => {
    const rounded = Math.round(size);
    return Math.min(MAX_FONT_SIZE_PX, Math.max(MIN_FONT_SIZE_PX, rounded));
  };

  useEffect(() => {
    if (open) {
      setActiveSection("general");
      setFontSizeInput(String(fontSizePx));
      getSetting("autoUpdate", true).then(setAutoUpdate);
      api.ai.providers.list().then((list) => {
        setProviders(list);
        const selected = list.find((p) => p.isDefault) ?? list[0];
        if (selected && AI_PROVIDER_OPTIONS_BY_TYPE[selected.providerType]) {
          applyProviderToForm(selected.providerType, list);
        } else {
          applyProviderToForm(AI_PROVIDER_OPTIONS[0].type, list);
        }
      }).catch((e) => {
        console.error(e);
        toast.error("Failed to load AI providers");
      });
    }
  }, [open]);

  useEffect(() => {
    setFontSizeInput(String(fontSizePx));
  }, [fontSizePx]);

  function applyProviderToForm(providerType: AIProviderType, source: AIProviderConfig[]) {
    const option = AI_PROVIDER_OPTIONS_BY_TYPE[providerType] ?? AI_PROVIDER_OPTIONS[0];
    const existing = source.find((p) => p.providerType === providerType);
    setSelectedProviderType(option.type);
    setProviderBaseUrl(existing?.baseUrl ?? option.baseUrl);
    setProviderModel(existing?.model ?? option.model);
    setProviderApiKey(existing?.apiKey ?? "");
  }

  const reloadProviders = async () => {
    const list = await api.ai.providers.list();
    setProviders(list);
    return list;
  };

  const handleProviderTypeChange = (value: string) => {
    applyProviderToForm(value, providers);
  };

  const handleCheckUpdate = async () => {
    setChecking(true);
    try {
      const update = await check();
      if (update?.available) {
        toast.info(`New version ${update.version} available!`, {
          action: {
            label: "Update",
            onClick: async () => {
              try {
                toast.info("Downloading update...");
                await update.downloadAndInstall();
                toast.success("Update installed, restarting...");
                await relaunch();
              } catch (e) {
                toast.error("Failed to update");
              }
            }
          }
        });
      } else {
        toast.success("You are on the latest version.");
      }
    } catch (error) {
      console.error(error);
      toast.error("Failed to check for updates");
    } finally {
      setChecking(false);
    }
  };

  const toggleAutoUpdate = async (checked: boolean) => {
    setAutoUpdate(checked);
    await saveSetting("autoUpdate", checked);
  };

  const handleSaveProvider = async () => {
    if (!providerBaseUrl.trim() || !providerModel.trim() || !providerApiKey.trim()) {
      toast.error("Please fill all provider fields");
      return;
    }
    try {
      const selectedOption =
        AI_PROVIDER_OPTIONS_BY_TYPE[selectedProviderType] ?? AI_PROVIDER_OPTIONS[0];
      const existing = providers.find((p) => p.providerType === selectedProviderType);
      const payload = {
        name: selectedOption.label,
        providerType: selectedProviderType,
        baseUrl: providerBaseUrl.trim(),
        model: providerModel.trim(),
        apiKey: providerApiKey.trim(),
        enabled: true,
        isDefault: true,
      } as const;

      if (existing) {
        await api.ai.providers.update(existing.id, payload);
      } else {
        await api.ai.providers.create({
          ...payload,
        });
      }
      const updated = await reloadProviders();
      applyProviderToForm(selectedProviderType, updated);
      toast.success("AI provider saved");
    } catch (e) {
      toast.error("Failed to save AI provider", {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const commitFontSizeInput = () => {
    const trimmed = fontSizeInput.trim();
    if (!trimmed) {
      setFontSizeInput(String(fontSizePx));
      return;
    }

    const parsed = Number(trimmed);
    if (!Number.isFinite(parsed)) {
      setFontSizeInput(String(fontSizePx));
      return;
    }

    const normalized = clampFontSize(parsed);
    setFontSizePx(normalized);
    setFontSizeInput(String(normalized));
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[860px] w-[92vw] h-[80vh] max-h-[80vh] flex flex-col overflow-hidden">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>
            Manage your app appearance and preferences.
          </DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-1 sm:grid-cols-[190px_1fr] gap-4 py-2 min-h-0 flex-1">
          <div className="border rounded-lg p-2 bg-muted/25 h-fit">
            <div className="space-y-1">
              <button
                className={`w-full text-left rounded-md px-3 py-2 text-sm transition-colors flex items-center gap-2 ${activeSection === "general"
                  ? "bg-background shadow-sm text-foreground"
                  : "text-muted-foreground hover:bg-muted/60"
                  }`}
                onClick={() => setActiveSection("general")}
              >
                <Settings2 className="w-4 h-4" />
                General
              </button>
              <button
                className={`w-full text-left rounded-md px-3 py-2 text-sm transition-colors flex items-center gap-2 ${activeSection === "ai"
                  ? "bg-background shadow-sm text-foreground"
                  : "text-muted-foreground hover:bg-muted/60"
                  }`}
                onClick={() => setActiveSection("ai")}
              >
                <Bot className="w-4 h-4" />
                AI
              </button>
              <button
                className={`w-full text-left rounded-md px-3 py-2 text-sm transition-colors flex items-center gap-2 ${activeSection === "about"
                  ? "bg-background shadow-sm text-foreground"
                  : "text-muted-foreground hover:bg-muted/60"
                  }`}
                onClick={() => setActiveSection("about")}
              >
                <Info className="w-4 h-4" />
                About
              </button>
            </div>
          </div>

          <div className="border rounded-lg p-4 overflow-y-auto min-h-0">
            {activeSection === "general" && (
              <div className="space-y-6">
                <div className="space-y-4">
                  <h3 className="text-lg font-medium flex items-center gap-2">
                    <Palette className="w-5 h-5" /> Appearance
                  </h3>

                  <div className="grid grid-cols-2 gap-4 items-center">
                    <div className="space-y-1">
                      <Label className="text-base">Theme Mode</Label>
                      <p className="text-xs text-muted-foreground">
                        Choose your interface style
                      </p>
                    </div>
                    <Select value={theme} onValueChange={(v) => setTheme(v as Theme)}>
                      <SelectTrigger>
                        <SelectValue placeholder="Select theme" />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="light">☀️ Light Mode</SelectItem>
                        <SelectItem value="dark">🌙 Dark Mode</SelectItem>
                        <SelectItem value="system">🖥️ System</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="grid grid-cols-2 gap-4 items-center">
                    <div className="space-y-1">
                      <Label className="text-base">Font Size</Label>
                      <p className="text-xs text-muted-foreground">
                        Adjust global text size across the app (Range: 10-24px)
                      </p>
                    </div>
                    <div className="flex items-center gap-2">
                      <Input
                        type="number"
                        min={MIN_FONT_SIZE_PX}
                        max={MAX_FONT_SIZE_PX}
                        step={1}
                        value={fontSizeInput}
                        onChange={(e) => setFontSizeInput(e.target.value)}
                        onBlur={commitFontSizeInput}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            commitFontSizeInput();
                          }
                        }}
                      />
                      <span className="text-sm text-muted-foreground">px</span>
                    </div>
                  </div>

                  <div className="space-y-3 pt-2">
                    <div className="flex items-center justify-between">
                      <Label className="text-base">Accent Color</Label>
                    </div>
                    <div className="flex flex-wrap gap-3">
                      {THEME_COLORS.map((color) => (
                        <button
                          key={color.name}
                          className={`h-8 w-8 rounded-full border-2 flex items-center justify-center transition-all ${accentColor === color.name
                            ? "border-primary ring-2 ring-ring ring-offset-2 scale-110"
                            : "border-transparent hover:scale-105"
                            }`}
                          style={{ backgroundColor: color.value }}
                          onClick={() => setAccentColor(color.name)}
                          title={color.name}
                        >
                          {accentColor === color.name && (
                            <div className="w-2 h-2 bg-white rounded-full" />
                          )}
                        </button>
                      ))}
                    </div>
                  </div>
                </div>

                <Separator />

                <div className="space-y-4">
                  <h3 className="text-lg font-medium flex items-center gap-2">
                    <RefreshCw className="w-5 h-5" /> Updates
                  </h3>
                  <div className="flex items-center justify-between">
                    <div className="space-y-1">
                      <Label className="text-base">Auto Update</Label>
                      <p className="text-xs text-muted-foreground">
                        Check for updates automatically
                      </p>
                    </div>
                    <Switch
                      checked={autoUpdate}
                      onCheckedChange={toggleAutoUpdate}
                    />
                  </div>
                  <Button
                    variant="outline"
                    className="w-full"
                    onClick={handleCheckUpdate}
                    disabled={checking}
                  >
                    {checking ? "Checking..." : "Check for updates now"}
                  </Button>
                </div>
              </div>
            )}

            {activeSection === "ai" && (
              <div className="space-y-4">
                <h3 className="text-lg font-medium flex items-center gap-2">
                  <Bot className="w-5 h-5" /> AI Providers
                </h3>

                <div className="space-y-2 border rounded-md p-3">
                  <div className="grid grid-cols-1 gap-2">
                    <Select
                      value={selectedProviderType}
                      onValueChange={handleProviderTypeChange}
                    >
                      <SelectTrigger>
                        <SelectValue placeholder="Select provider" />
                      </SelectTrigger>
                      <SelectContent>
                        {AI_PROVIDER_OPTIONS.map((item) => (
                          <SelectItem key={item.type} value={item.type}>
                            {item.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <Input
                      placeholder="Base URL (OpenAI-compatible)"
                      value={providerBaseUrl}
                      onChange={(e) => setProviderBaseUrl(e.target.value)}
                    />
                    <Input
                      placeholder="Model"
                      value={providerModel}
                      onChange={(e) => setProviderModel(e.target.value)}
                    />
                    <Input
                      placeholder="API Key"
                      value={providerApiKey}
                      onChange={(e) => setProviderApiKey(e.target.value)}
                    />
                  </div>
                  <div className="flex gap-2">
                    <Button onClick={handleSaveProvider} className="flex-1">
                      Save Provider
                    </Button>
                  </div>
                </div>

                <div className="rounded-md border p-3 text-xs text-muted-foreground">
                  <div>Configured providers: {providers.length}</div>
                  <div className="mt-2 border-t border-border/60 pt-2">
                    <div className="mb-1 text-[11px] font-medium uppercase tracking-wide text-muted-foreground/90">
                      Configured details
                    </div>
                    {providers.length > 0 ? (
                      <div className="space-y-1">
                        {providers.map((provider) => {
                          const label =
                            AI_PROVIDER_OPTIONS_BY_TYPE[provider.providerType]?.label ||
                            provider.name ||
                            provider.providerType;
                          return (
                            <div
                              key={provider.id}
                              className="flex items-center justify-between gap-2 rounded-sm bg-muted/40 px-2 py-1"
                            >
                              <span className="truncate">
                                {label} · {provider.model}
                              </span>
                              {provider.isDefault && (
                                <span className="shrink-0 rounded border border-primary/40 bg-primary/10 px-1.5 py-0.5 text-[10px] font-medium text-primary">
                                  Default
                                </span>
                              )}
                            </div>
                          );
                        })}
                      </div>
                    ) : (
                      <div>No providers configured yet</div>
                    )}
                  </div>
                </div>
              </div>
            )}

            {activeSection === "about" && (
              <div className="space-y-4">
                <h3 className="text-lg font-medium flex items-center gap-2">
                  <Info className="w-5 h-5" /> About
                </h3>
                <div className="bg-muted/50 rounded-lg p-4 space-y-2">
                  <div className="flex items-center justify-between">
                    <span className="font-medium">DbPaw</span>
                    <span className="text-sm text-muted-foreground">v{APP_VERSION}</span>
                  </div>
                  <p className="text-sm text-muted-foreground">
                    A modern database management tool providing a smooth development experience.
                  </p>
                  <div className="grid grid-cols-[88px_1fr] gap-x-2 gap-y-1 text-xs text-muted-foreground pt-1">
                    <span className="font-medium text-foreground/90">GitHub</span>
                    <a
                      href={GITHUB_URL}
                      target="_blank"
                      rel="noreferrer noopener"
                      className="truncate underline-offset-4 hover:underline"
                    >
                      {GITHUB_URL}
                    </a>
                    <span className="font-medium text-foreground/90">Tech</span>
                    <span>Tauri + React + TypeScript</span>
                    <span className="font-medium text-foreground/90">License</span>
                    <span>MIT</span>
                    <span className="font-medium text-foreground/90">Platforms</span>
                    <span>macOS / Windows / Linux</span>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
