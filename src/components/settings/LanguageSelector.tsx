import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  SUPPORTED_LANGUAGES,
  changeLanguage,
  getCurrentLanguage,
} from "@/lib/i18n";
import { useTranslation } from "react-i18next";

export function LanguageSelector() {
  const { t } = useTranslation();

  return (
    <div className="grid grid-cols-2 gap-4 items-center">
      <div className="space-y-1">
        <Label className="text-base">{t("settings.language.title")}</Label>
        <p className="text-xs text-muted-foreground">
          {t("settings.language.description")}
        </p>
      </div>
      <Select value={getCurrentLanguage()} onValueChange={(value) => void changeLanguage(value)}>
        <SelectTrigger>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {SUPPORTED_LANGUAGES.map((lang) => (
            <SelectItem key={lang} value={lang}>
              {lang === "en" ? t("settings.language.en") : t("settings.language.zh")}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
