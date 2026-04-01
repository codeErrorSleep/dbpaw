import { describe, expect, test, mock } from "bun:test";

// Mock i18next
const mockI18nUse = mock();
const mockI18nInit = mock();
const mockI18nChangeLanguage = mock();
const mockI18nOn = mock();

const mockI18n = {
  use: mockI18nUse.mockReturnThis(),
  init: mockI18nInit,
  on: mockI18nOn,
  changeLanguage: mockI18nChangeLanguage,
  isInitialized: false,
  language: "en",
};

mock.module("i18next", () => ({
  default: mockI18n,
}));

// Mock react-i18next
mock.module("react-i18next", () => ({
  initReactI18next: {},
}));

// Mock store functions
const mockGetSetting = mock();
const mockSaveSetting = mock();

mock.module("@/services/store", () => ({
  getSetting: mockGetSetting,
  saveSetting: mockSaveSetting,
}));

// Mock locales
mock.module("./locales/en", () => ({
  en: { greeting: "Hello" },
}));
mock.module("./locales/zh", () => ({
  zh: { greeting: "你好" },
}));
mock.module("./locales/ja", () => ({
  ja: { greeting: "こんにちは" },
}));

const loadI18n = async () =>
  import(`./index?test=${Date.now()}-${Math.random()}`);

describe("i18n", () => {
  test("SUPPORTED_LANGUAGES should contain en, zh, ja", async () => {
    const i18n = await loadI18n();
    expect(i18n.SUPPORTED_LANGUAGES).toEqual(["en", "zh", "ja"]);
  });

  test("getCurrentLanguage should return normalized language", async () => {
    const i18n = await loadI18n();

    // Test with zh
    mockI18n.language = "zh-CN";
    expect(i18n.getCurrentLanguage()).toBe("zh");

    // Test with ja
    mockI18n.language = "ja-JP";
    expect(i18n.getCurrentLanguage()).toBe("ja");

    // Test with en
    mockI18n.language = "en-US";
    expect(i18n.getCurrentLanguage()).toBe("en");

    // Reset
    mockI18n.language = "en";
  });

  test("changeLanguage should call i18n.changeLanguage with normalized value", async () => {
    const i18n = await loadI18n();
    mockI18nChangeLanguage.mockResolvedValue(undefined);

    await i18n.changeLanguage("zh-CN");
    expect(mockI18nChangeLanguage).toHaveBeenCalledWith("zh");

    mockI18nChangeLanguage.mockClear();

    await i18n.changeLanguage("ja-JP");
    expect(mockI18nChangeLanguage).toHaveBeenCalledWith("ja");
  });

  test("initI18nFromStore should initialize with saved language", async () => {
    const i18n = await loadI18n();
    mockGetSetting.mockResolvedValue("zh");
    mockI18nChangeLanguage.mockResolvedValue(undefined);

    const result = await i18n.initI18nFromStore();

    expect(mockGetSetting).toHaveBeenCalledWith("language", "en");
    expect(result).toBe("zh");
  });

  test("initI18nFromStore should use default when no saved language", async () => {
    const i18n = await loadI18n();
    mockGetSetting.mockResolvedValue("en");
    mockI18n.language = "en";
    mockI18nChangeLanguage.mockResolvedValue(undefined);

    const result = await i18n.initI18nFromStore();

    expect(result).toBe("en");
  });
});
