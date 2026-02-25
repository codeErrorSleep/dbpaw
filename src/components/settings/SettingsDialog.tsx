import { Bot, Info, Palette, RefreshCw, Settings2 } from "lucide-react";
import { useTheme, Theme } from "@/components/theme-provider";
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

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type SettingsSection = "general" | "ai" | "about";

const THEME_COLORS = [
  { name: "Zinc", value: "#09090b" },
  { name: "Blue", value: "#3b82f6" },
  { name: "Violet", value: "#8b5cf6" },
  { name: "Green", value: "#22c55e" },
  { name: "Orange", value: "#f97316" },
];

const AI_PROVIDER_OPTIONS: {
  type: AIProviderType;
  label: string;
  baseUrl: string;
  model: string;
}[] = [
    {
      type: "openai",
      label: "OpenAI",
      baseUrl: "https://api.openai.com/v1",
      model: "gpt-4.1-mini",
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
  ];

const AI_PROVIDER_OPTIONS_BY_TYPE = AI_PROVIDER_OPTIONS.reduce(
  (acc, item) => ({ ...acc, [item.type]: item }),
  {} as Record<AIProviderType, (typeof AI_PROVIDER_OPTIONS)[number]>,
);

const isAIProviderType = (value: string): value is AIProviderType =>
  value === "openai" || value === "kimi" || value === "glm";

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const { theme, setTheme, accentColor, setAccentColor } = useTheme();
  const [activeSection, setActiveSection] = useState<SettingsSection>("general");
  const [autoUpdate, setAutoUpdate] = useState(true);
  const [checking, setChecking] = useState(false);
  const [providers, setProviders] = useState<AIProviderConfig[]>([]);
  const [selectedProviderType, setSelectedProviderType] = useState<AIProviderType>("openai");
  const [providerBaseUrl, setProviderBaseUrl] = useState(
    AI_PROVIDER_OPTIONS_BY_TYPE.openai.baseUrl,
  );
  const [providerModel, setProviderModel] = useState(
    AI_PROVIDER_OPTIONS_BY_TYPE.openai.model,
  );
  const [providerApiKey, setProviderApiKey] = useState("");

  useEffect(() => {
    if (open) {
      setActiveSection("general");
      getSetting("autoUpdate", true).then(setAutoUpdate);
      api.ai.providers.list().then((list) => {
        const validProviders = list.filter((p) => isAIProviderType(p.providerType));
        setProviders(validProviders);
        const selected = validProviders.find((p) => p.isDefault) ?? validProviders[0];
        if (selected) {
          applyProviderToForm(selected.providerType, validProviders);
        } else {
          applyProviderToForm("openai", validProviders);
        }
      }).catch((e) => {
        console.error(e);
        toast.error("Failed to load AI providers");
      });
    }
  }, [open]);

  function applyProviderToForm(providerType: AIProviderType, source: AIProviderConfig[]) {
    const option = AI_PROVIDER_OPTIONS_BY_TYPE[providerType];
    const existing = source.find((p) => p.providerType === providerType);
    setSelectedProviderType(providerType);
    setProviderBaseUrl(existing?.baseUrl ?? option.baseUrl);
    setProviderModel(existing?.model ?? option.model);
    setProviderApiKey(existing?.apiKey ?? "");
  }

  const reloadProviders = async () => {
    const list = await api.ai.providers.list();
    const validProviders = list.filter((p) => isAIProviderType(p.providerType));
    setProviders(validProviders);
    return validProviders;
  };

  const handleProviderTypeChange = (value: string) => {
    if (!isAIProviderType(value)) return;
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
      const selectedOption = AI_PROVIDER_OPTIONS_BY_TYPE[selectedProviderType];
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
                  Configured providers: {providers.length}/3
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
                    <span className="text-sm text-muted-foreground">v0.1.0</span>
                  </div>
                  <p className="text-sm text-muted-foreground">
                    A modern database management tool providing a smooth development experience.
                  </p>
                </div>
              </div>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
