export const LOCALE_DISPLAY_NAMES = {
  "zh-CN": "中文简体",
  "zh-Hant": "中文正體",
  en: "English",
} as const;

export const LOCALE_OPTIONS = [
  { code: "zh-CN", label: LOCALE_DISPLAY_NAMES["zh-CN"] },
  { code: "zh-Hant", label: LOCALE_DISPLAY_NAMES["zh-Hant"] },
  { code: "en", label: LOCALE_DISPLAY_NAMES.en },
] as const;
