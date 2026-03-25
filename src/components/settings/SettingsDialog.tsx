import {
  Bot,
  Command,
  Info,
  LayoutPanelLeft,
  Palette,
  RefreshCw,
  Settings2,
  Trash2,
} from "lucide-react";
import {
  useTheme,
  MIN_FONT_SIZE_PX,
  MAX_FONT_SIZE_PX,
} from "@/components/theme-provider";
import { ThemeId, THEME_PRESETS } from "@/theme/themeRegistry";
import { useState, useEffect } from "react";
import { getSetting, saveSetting } from "@/services/store";
import {
  checkForUpdates,
  getUpdateTaskSnapshot,
  relaunchAfterUpdate,
  startBackgroundInstall,
  subscribeUpdateTask,
  UpdateTaskState,
} from "@/services/updater";
import { AIProviderConfig, AIProviderForm, AIProviderType, api } from "@/services/api";
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
import { LanguageSelector } from "./LanguageSelector";
import { useTranslation } from "react-i18next";

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  sidebarLayout?: "tabs" | "tree";
  onSidebarLayoutChange?: (layout: "tabs" | "tree") => void;
}

type SettingsSection = "general" | "layout" | "ai" | "shortcuts" | "about";
type AIProviderPreset = {
  type: AIProviderType;
  label: string;
  baseUrl: string;
  model: string;
};
type ShortcutItem = {
  action: string;
  keys: string;
  scope: string;
  note?: string;
};
type ShortcutGroup = {
  title: string;
  items: ShortcutItem[];
};

const AI_PROVIDER_OPTIONS: AIProviderPreset[] = [
  {
    type: "openai",
    label: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    model: "gpt-4.1-mini",
  },
  {
    type: "gemini",
    label: "Gemini",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai",
    model: "gemini-2.0-flash",
  },
  {
    type: "anthropic",
    label: "Anthropic",
    baseUrl: "https://api.anthropic.com/v1",
    model: "claude-3-5-sonnet-20241022",
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
    type: "siliconflow",
    label: "SiliconFlow",
    baseUrl: "https://api.siliconflow.cn/v1",
    model: "Qwen/Qwen2.5-72B-Instruct",
  },
  {
    type: "groq",
    label: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    model: "llama-3.3-70b-versatile",
  },
  {
    type: "glm",
    label: "GLM",
    baseUrl: "https://open.bigmodel.cn/api/paas/v4",
    model: "glm-4-flash",
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
const SHORTCUT_GROUPS: ShortcutGroup[] = [
  {
    title: "Global",
    items: [
      { action: "Open settings", keys: "Cmd/Ctrl + ,", scope: "App / Menu" },
      { action: "Toggle AI sidebar", keys: "Cmd/Ctrl + \\", scope: "App" },
      { action: "Toggle main sidebar", keys: "Cmd/Ctrl + B", scope: "Sidebar" },
      { action: "Create new query tab", keys: "Cmd/Ctrl + N", scope: "App" },
      { action: "Close current tab", keys: "Cmd/Ctrl + W", scope: "App" },
      {
        action: "Next tab",
        keys: "Cmd/Ctrl + Shift + ]",
        scope: "App",
      },
      {
        action: "Previous tab",
        keys: "Cmd/Ctrl + Shift + [",
        scope: "App",
      },
    ],
  },
  {
    title: "SQL Editor",
    items: [
      { action: "Execute SQL", keys: "Cmd/Ctrl + Enter", scope: "Editor" },
      { action: "Save query", keys: "Cmd/Ctrl + S", scope: "Editor" },
      { action: "Format SQL", keys: "Shift + Alt + F", scope: "Editor" },
      {
        action: "Accept completion / insert tab",
        keys: "Tab",
        scope: "Editor",
      },
    ],
  },
  {
    title: "Table View",
    items: [
      {
        action: "Save pending changes",
        keys: "Cmd/Ctrl + S",
        scope: "Table",
      },
      { action: "Open table search", keys: "Cmd/Ctrl + F", scope: "Table" },
      {
        action: "Copy selected rows",
        keys: "Cmd/Ctrl + C",
        scope: "Table",
      },
      {
        action: "Cancel edit / discard pending changes",
        keys: "Esc",
        scope: "Table",
      },
    ],
  },
  {
    title: "Input Panels",
    items: [
      {
        action: "Send AI message",
        keys: "Enter",
        scope: "AI input",
        note: "Shift + Enter inserts a newline",
      },
      {
        action: "Save query from description input",
        keys: "Enter",
        scope: "Save Query dialog",
        note: "Shift + Enter inserts a newline",
      },
    ],
  },
];

export function SettingsDialog({
  open,
  onOpenChange,
  sidebarLayout = "tabs",
  onSidebarLayoutChange,
}: SettingsDialogProps) {
  const { t } = useTranslation();
  const {
    theme,
    setTheme,
    fontSizePx,
    setFontSizePx,
  } = useTheme();
  const [activeSection, setActiveSection] =
    useState<SettingsSection>("general");
  const [autoUpdate, setAutoUpdate] = useState(true);
  const [checking, setChecking] = useState(false);
  const [updateTaskState, setUpdateTaskState] = useState<UpdateTaskState>(
    getUpdateTaskSnapshot().state,
  );
  const [providers, setProviders] = useState<AIProviderConfig[]>([]);
  const [deletingProviderId, setDeletingProviderId] = useState<number | null>(null);
  const [selectedProviderType, setSelectedProviderType] =
    useState<AIProviderType>(AI_PROVIDER_OPTIONS[0].type);
  const [providerBaseUrl, setProviderBaseUrl] = useState(
    AI_PROVIDER_OPTIONS[0].baseUrl,
  );
  const [providerModel, setProviderModel] = useState(
    AI_PROVIDER_OPTIONS[0].model,
  );
  const [providerApiKeyInput, setProviderApiKeyInput] = useState("");
  const [providerHasApiKey, setProviderHasApiKey] = useState(false);
  const [showProviderApiKey, setShowProviderApiKey] = useState(false);
  const [fontSizeInput, setFontSizeInput] = useState(String(fontSizePx));
  const [layoutMode, setLayoutMode] = useState<"tabs" | "tree">(sidebarLayout);

  const clampFontSize = (size: number) => {
    const rounded = Math.round(size);
    return Math.min(MAX_FONT_SIZE_PX, Math.max(MIN_FONT_SIZE_PX, rounded));
  };

  useEffect(() => {
    const unsubscribe = subscribeUpdateTask((snapshot) => {
      setUpdateTaskState(snapshot.state);
    });
    return unsubscribe;
  }, []);

  useEffect(() => {
    if (open) {
      setActiveSection("general");
      setFontSizeInput(String(fontSizePx));
      setLayoutMode(sidebarLayout);
      getSetting("autoUpdate", true).then(setAutoUpdate);
      api.ai.providers
        .list()
        .then((list) => {
          setProviders(list);
          const selected = list.find((p) => p.isDefault) ?? list[0];
          if (selected && AI_PROVIDER_OPTIONS_BY_TYPE[selected.providerType]) {
            applyProviderToForm(selected.providerType, list);
          } else {
            applyProviderToForm(AI_PROVIDER_OPTIONS[0].type, list);
          }
        })
        .catch((e) => {
          console.error(e);
          toast.error(t("settings.aiProviders.loadFailed"));
        });
    }
  }, [fontSizePx, open, sidebarLayout, t]);

  useEffect(() => {
    setFontSizeInput(String(fontSizePx));
  }, [fontSizePx]);

  function applyProviderToForm(
    providerType: AIProviderType,
    source: AIProviderConfig[],
  ) {
    const option =
      AI_PROVIDER_OPTIONS_BY_TYPE[providerType] ?? AI_PROVIDER_OPTIONS[0];
    const existing = source.find((p) => p.providerType === providerType);
    setSelectedProviderType(option.type);
    setProviderBaseUrl(existing?.baseUrl ?? option.baseUrl);
    setProviderModel(existing?.model ?? option.model);
    setProviderHasApiKey(existing?.hasApiKey ?? false);
    setProviderApiKeyInput("");
    setShowProviderApiKey(false);
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
    if (checking) return;
    if (updateTaskState === "ready_to_restart") {
      await relaunchAfterUpdate();
      return;
    }
    if (
      updateTaskState === "checking" ||
      updateTaskState === "downloading" ||
      updateTaskState === "installing"
    ) {
      toast.info(t("settings.updates.inBackgroundProgress"));
      return;
    }

    setChecking(true);
    try {
      const result = await checkForUpdates();
      if (result.state === "available" && result.update) {
        toast.info(t("settings.updates.available", { version: result.update.version }), {
          action: {
            label: t("settings.updates.updateAction"),
            onClick: () => {
              const startResult = startBackgroundInstall(result.update);
              if (!startResult.started) {
                toast.info(t("settings.updates.inBackgroundProgress"));
                return;
              }
              toast.success(t("settings.updates.backgroundStarted"));
            },
          },
        });
      } else {
        toast.success(result.message ?? t("settings.updates.latest"));
      }
    } catch (error) {
      console.error(error);
      toast.error(t("settings.updates.failedCheck"));
    } finally {
      setChecking(false);
    }
  };

  const toggleAutoUpdate = async (checked: boolean) => {
    setAutoUpdate(checked);
    await saveSetting("autoUpdate", checked);
  };

  const handleSaveProvider = async () => {
    try {
      const selectedOption =
        AI_PROVIDER_OPTIONS_BY_TYPE[selectedProviderType] ??
        AI_PROVIDER_OPTIONS[0];
      const existing = providers.find(
        (p) => p.providerType === selectedProviderType,
      );
      const apiKey = providerApiKeyInput.trim();
      const requireApiKey = !existing || !existing.hasApiKey;
      if (!providerBaseUrl.trim() || !providerModel.trim() || (requireApiKey && !apiKey)) {
        toast.error(t("settings.aiProviders.fillRequired"));
        return;
      }

      const payload: AIProviderForm = {
        name: selectedOption.label,
        providerType: selectedProviderType,
        baseUrl: providerBaseUrl.trim(),
        model: providerModel.trim(),
        enabled: true,
        isDefault: true,
        ...(apiKey ? { apiKey } : {}),
      };

      if (existing) {
        await api.ai.providers.update(existing.id, payload);
      } else {
        await api.ai.providers.create({
          ...payload,
        });
      }
      const updated = await reloadProviders();
      applyProviderToForm(selectedProviderType, updated);
      toast.success(t("settings.aiProviders.saveSuccess"));
    } catch (e) {
      toast.error(t("settings.aiProviders.saveFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const handleClearProviderApiKey = async () => {
    if (!providerHasApiKey) return;
    try {
      await api.ai.providers.clearApiKey(selectedProviderType);
      const updated = await reloadProviders();
      applyProviderToForm(selectedProviderType, updated);
      toast.success(t("settings.aiProviders.clearSuccess"));
    } catch (e) {
      toast.error(t("settings.aiProviders.clearFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const handleDeleteProvider = async (id: number) => {
    if (deletingProviderId != null) return;
    setDeletingProviderId(id);
    try {
      await api.ai.providers.delete(id);
      const updated = await reloadProviders();
      applyProviderToForm(selectedProviderType, updated);
      toast.success(t("settings.aiProviders.deleteSuccess"));
    } catch (e) {
      toast.error(t("settings.aiProviders.deleteFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setDeletingProviderId(null);
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

  const handleLayoutChange = async (value: "tabs" | "tree") => {
    setLayoutMode(value);
    await saveSetting("sidebarLayout", value);
    onSidebarLayoutChange?.(value);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[860px] w-[92vw] h-[80vh] max-h-[80vh] flex flex-col overflow-hidden">
        <DialogHeader>
          <DialogTitle>{t("settings.title")}</DialogTitle>
          <DialogDescription>
            {t("settings.description")}
          </DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-1 sm:grid-cols-[190px_1fr] gap-4 py-2 min-h-0 flex-1">
          <div className="border rounded-lg p-2 bg-muted/25 h-fit">
            <div className="space-y-1">
              <button
                className={`w-full text-left rounded-md px-3 py-2 text-sm transition-colors flex items-center gap-2 ${
                  activeSection === "general"
                    ? "bg-background shadow-sm text-foreground"
                    : "text-muted-foreground hover:bg-muted/60"
                }`}
                onClick={() => setActiveSection("general")}
              >
                <Settings2 className="w-4 h-4" />
                {t("settings.sections.general")}
              </button>
              <button
                className={`w-full text-left rounded-md px-3 py-2 text-sm transition-colors flex items-center gap-2 ${
                  activeSection === "layout"
                    ? "bg-background shadow-sm text-foreground"
                    : "text-muted-foreground hover:bg-muted/60"
                }`}
                onClick={() => setActiveSection("layout")}
              >
                <LayoutPanelLeft className="w-4 h-4" />
                {t("settings.sections.layout")}
              </button>
              <button
                className={`w-full text-left rounded-md px-3 py-2 text-sm transition-colors flex items-center gap-2 ${
                  activeSection === "ai"
                    ? "bg-background shadow-sm text-foreground"
                    : "text-muted-foreground hover:bg-muted/60"
                }`}
                onClick={() => setActiveSection("ai")}
              >
                <Bot className="w-4 h-4" />
                {t("settings.sections.ai")}
              </button>
              <button
                className={`w-full text-left rounded-md px-3 py-2 text-sm transition-colors flex items-center gap-2 ${
                  activeSection === "shortcuts"
                    ? "bg-background shadow-sm text-foreground"
                    : "text-muted-foreground hover:bg-muted/60"
                }`}
                onClick={() => setActiveSection("shortcuts")}
              >
                <Command className="w-4 h-4" />
                {t("settings.sections.shortcuts")}
              </button>
              <button
                className={`w-full text-left rounded-md px-3 py-2 text-sm transition-colors flex items-center gap-2 ${
                  activeSection === "about"
                    ? "bg-background shadow-sm text-foreground"
                    : "text-muted-foreground hover:bg-muted/60"
                }`}
                onClick={() => setActiveSection("about")}
              >
                <Info className="w-4 h-4" />
                {t("settings.sections.about")}
              </button>
            </div>
          </div>

          <div className="border rounded-lg p-4 overflow-y-auto min-h-0">
            {activeSection === "general" && (
              <div className="space-y-6">
                <div className="space-y-4">
                  <LanguageSelector />
                  <h3 className="text-lg font-medium flex items-center gap-2">
                    <Palette className="w-5 h-5" /> {t("settings.appearance.title")}
                  </h3>

                  <div className="grid grid-cols-2 gap-4 items-center">
                    <div className="space-y-1">
                      <Label className="text-base">{t("settings.appearance.themeTitle")}</Label>
                      <p className="text-xs text-muted-foreground">
                        {t("settings.appearance.themeDescription")}
                      </p>
                    </div>
                    <Select
                      value={theme}
                      onValueChange={(v) => setTheme(v as ThemeId)}
                    >
                      <SelectTrigger>
                        <SelectValue placeholder={t("settings.appearance.selectTheme")} />
                      </SelectTrigger>
                      <SelectContent>
                        {/* Light themes */}
                        {Object.values(THEME_PRESETS)
                          .filter((preset) => preset.appearance === "light")
                          .map((preset) => (
                            <SelectItem key={preset.id} value={preset.id}>
                              {preset.label}
                            </SelectItem>
                          ))}
                        {/* Dark themes */}
                        {Object.values(THEME_PRESETS)
                          .filter((preset) => preset.appearance === "dark")
                          .map((preset) => (
                            <SelectItem key={preset.id} value={preset.id}>
                              {preset.label}
                            </SelectItem>
                          ))}
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="grid grid-cols-2 gap-4 items-center">
                    <div className="space-y-1">
                      <Label className="text-base">{t("settings.appearance.fontSizeTitle")}</Label>
                      <p className="text-xs text-muted-foreground">
                        {t("settings.appearance.fontSizeDescription", {
                          min: MIN_FONT_SIZE_PX,
                          max: MAX_FONT_SIZE_PX,
                        })}
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


                </div>

                <Separator />

                <div className="space-y-4">
                  <h3 className="text-lg font-medium flex items-center gap-2">
                    <RefreshCw className="w-5 h-5" /> {t("settings.updates.title")}
                  </h3>
                  <div className="flex items-center justify-between">
                    <div className="space-y-1">
                      <Label className="text-base">{t("settings.updates.autoUpdate")}</Label>
                      <p className="text-xs text-muted-foreground">
                        {t("settings.updates.autoUpdateDescription")}
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
                    disabled={
                      checking ||
                      updateTaskState === "checking" ||
                      updateTaskState === "downloading" ||
                      updateTaskState === "installing"
                    }
                  >
                    {updateTaskState === "ready_to_restart"
                      ? t("settings.updates.restartNow")
                      : checking
                      ? t("settings.updates.checking")
                      : updateTaskState === "checking" ||
                          updateTaskState === "downloading" ||
                          updateTaskState === "installing"
                        ? t("settings.updates.updating")
                        : t("settings.updates.checkNow")}
                  </Button>
                </div>
              </div>
            )}

            {activeSection === "layout" && (
              <div className="space-y-4">
                <h3 className="text-lg font-medium flex items-center gap-2">
                  <LayoutPanelLeft className="w-5 h-5" /> {t("settings.layout.title")}
                </h3>
                <div className="grid grid-cols-2 gap-4 items-center">
                  <div className="space-y-1">
                    <Label className="text-base">{t("settings.layout.modeTitle")}</Label>
                    <p className="text-xs text-muted-foreground">
                      {t("settings.layout.modeDescription")}
                    </p>
                  </div>
                  <Select
                    value={layoutMode}
                    onValueChange={(value) =>
                      void handleLayoutChange(value as "tabs" | "tree")
                    }
                  >
                    <SelectTrigger>
                      <SelectValue placeholder={t("settings.layout.modeTitle")} />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="tabs">{t("settings.layout.modeTabs")}</SelectItem>
                      <SelectItem value="tree">{t("settings.layout.modeTree")}</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>
            )}

            {activeSection === "ai" && (
              <div className="space-y-4">
                <h3 className="text-lg font-medium flex items-center gap-2">
                  <Bot className="w-5 h-5" /> {t("settings.aiProviders.title")}
                </h3>

                <div className="space-y-2 border rounded-md p-3">
                  <div className="grid grid-cols-1 gap-2">
                    <Select
                      value={selectedProviderType}
                      onValueChange={handleProviderTypeChange}
                    >
                      <SelectTrigger>
                        <SelectValue placeholder={t("settings.aiProviders.selectProvider")} />
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
                      placeholder={t("settings.aiProviders.baseUrl")}
                      value={providerBaseUrl}
                      onChange={(e) => setProviderBaseUrl(e.target.value)}
                    />
                    <Input
                      placeholder={t("settings.aiProviders.model")}
                      value={providerModel}
                      onChange={(e) => setProviderModel(e.target.value)}
                    />
                    <div className="flex gap-2">
                      <Input
                        placeholder={t("settings.aiProviders.apiKey")}
                        type={showProviderApiKey ? "text" : "password"}
                        value={providerApiKeyInput}
                        onChange={(e) => setProviderApiKeyInput(e.target.value)}
                      />
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={() => setShowProviderApiKey((v) => !v)}
                      >
                        {showProviderApiKey
                          ? t("settings.aiProviders.hide")
                          : t("settings.aiProviders.show")}
                      </Button>
                    </div>
                    {providerHasApiKey && !providerApiKeyInput.trim() && (
                      <div className="text-xs text-muted-foreground">
                        {t("settings.aiProviders.keySavedHint")}
                      </div>
                    )}
                  </div>
                  <div className="flex gap-2">
                    <Button
                      type="button"
                      variant="outline"
                      onClick={handleClearProviderApiKey}
                      disabled={!providerHasApiKey}
                    >
                      {t("settings.aiProviders.clearKey")}
                    </Button>
                    <Button onClick={handleSaveProvider} className="flex-1">
                      {t("settings.aiProviders.saveProvider")}
                    </Button>
                  </div>
                </div>

                <div className="rounded-md border p-3 text-xs text-muted-foreground">
                  <div>{t("settings.aiProviders.configured", { count: providers.length })}</div>
                  <div className="mt-2 border-t border-border/60 pt-2">
                    <div className="mb-1 text-[11px] font-medium uppercase tracking-wide text-muted-foreground/90">
                      {t("settings.aiProviders.configuredDetails")}
                    </div>
                    {providers.length > 0 ? (
                      <div className="space-y-1">
                        {providers.map((provider) => {
                          const label =
                            AI_PROVIDER_OPTIONS_BY_TYPE[provider.providerType]
                              ?.label ||
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
                              <div className="flex shrink-0 items-center gap-1">
                                {provider.isDefault && (
                                  <span className="rounded border border-primary/40 bg-primary/10 px-1.5 py-0.5 text-[10px] font-medium text-primary">
                                    {t("settings.aiProviders.default")}
                                  </span>
                                )}
                                <button
                                  type="button"
                                  disabled={deletingProviderId === provider.id}
                                  onClick={() =>
                                    handleDeleteProvider(provider.id)
                                  }
                                  className="rounded-sm p-0.5 text-muted-foreground/60 transition-colors hover:bg-destructive/10 hover:text-destructive disabled:opacity-40 disabled:pointer-events-none"
                                  title={t("settings.aiProviders.deleteProvider")}
                                >
                                  <Trash2 className="h-3.5 w-3.5" />
                                </button>
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    ) : (
                      <div>{t("settings.aiProviders.empty")}</div>
                    )}
                  </div>
                </div>
              </div>
            )}

            {activeSection === "shortcuts" && (
              <div className="space-y-4">
                <h3 className="text-lg font-medium flex items-center gap-2">
                  <Command className="w-5 h-5" /> {t("settings.shortcuts.title")}
                </h3>
                <div className="rounded-md border p-3 text-xs text-muted-foreground">
                  {t("settings.shortcuts.readonlyHint")}
                </div>
                <div className="space-y-4">
                  {SHORTCUT_GROUPS.map((group) => (
                    <div key={group.title} className="rounded-md border">
                      <div className="border-b bg-muted/40 px-3 py-2 text-sm font-medium text-foreground">
                        {group.title}
                      </div>
                      <div className="divide-y">
                        {group.items.map((item) => (
                          <div
                            key={`${group.title}-${item.action}`}
                            className="grid grid-cols-1 gap-2 px-3 py-2 sm:grid-cols-[1.2fr_220px_140px]"
                          >
                            <div className="space-y-0.5">
                              <div className="text-sm text-foreground">{item.action}</div>
                              {item.note && (
                                <div className="text-xs text-muted-foreground">{item.note}</div>
                              )}
                            </div>
                            <div className="text-sm font-mono text-foreground">
                              {item.keys}
                            </div>
                            <div className="text-xs text-muted-foreground sm:text-right">
                              {item.scope}
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {activeSection === "about" && (
              <div className="space-y-4">
                <h3 className="text-lg font-medium flex items-center gap-2">
                  <Info className="w-5 h-5" /> {t("settings.about.title")}
                </h3>
                <div className="bg-muted/50 rounded-lg p-4 space-y-2">
                  <div className="flex items-center justify-between">
                    <span className="font-medium">DbPaw</span>
                    <span className="text-sm text-muted-foreground">
                      v{APP_VERSION}
                    </span>
                  </div>
                  <p className="text-sm text-muted-foreground">
                    {t("settings.about.description")}
                  </p>
                  <div className="grid grid-cols-[88px_1fr] gap-x-2 gap-y-1 text-xs text-muted-foreground pt-1">
                    <span className="font-medium text-foreground/90">
                      {t("settings.about.github")}
                    </span>
                    <a
                      href={GITHUB_URL}
                      target="_blank"
                      rel="noreferrer noopener"
                      className="truncate underline-offset-4 hover:underline"
                    >
                      {GITHUB_URL}
                    </a>
                    <span className="font-medium text-foreground/90">
                      {t("settings.about.license")}
                    </span>
                    <span>Apache-2.0</span>
                    <span className="font-medium text-foreground/90">
                      {t("settings.about.platforms")}
                    </span>
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
