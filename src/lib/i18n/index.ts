import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { getSetting, saveSetting } from "@/services/store";
import { en } from "./locales/en";
import { zh } from "./locales/zh";

export const SUPPORTED_LANGUAGES = ["en", "zh"] as const;
export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

const resources = {
  en: { translation: en },
  zh: { translation: zh },
};

const normalizeLanguage = (value?: string | null): SupportedLanguage => {
  if (!value) return "en";
  if (value.toLowerCase().startsWith("zh")) return "zh";
  return "en";
};

if (!i18n.isInitialized) {
  void i18n.use(initReactI18next).init({
    resources,
    lng: "en",
    fallbackLng: "en",
    interpolation: {
      escapeValue: false,
    },
  });
}

i18n.on("languageChanged", (lng) => {
  const normalized = normalizeLanguage(lng);
  if (normalized !== lng) {
    void i18n.changeLanguage(normalized);
    return;
  }
  void saveSetting("language", normalized);
});

export const getCurrentLanguage = () => normalizeLanguage(i18n.language);

export const changeLanguage = async (lng: string) => {
  await i18n.changeLanguage(normalizeLanguage(lng));
};

export const initI18nFromStore = async () => {
  const saved = await getSetting<string>("language", "en");
  const normalized = normalizeLanguage(saved);
  if (i18n.language !== normalized) {
    await i18n.changeLanguage(normalized);
  }
  return normalized;
};

export default i18n;
