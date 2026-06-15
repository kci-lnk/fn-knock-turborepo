export const LOCALE_DISPLAY_NAMES = {
  "zh-CN": "中文简体",
  "zh-Hant": "中文正體",
  en: "English",
  "ko-KR": "한국어",
  "ja-JP": "日本語",
} as const;

export const LOCALE_OPTIONS = [
  { code: "zh-CN", label: LOCALE_DISPLAY_NAMES["zh-CN"] },
  { code: "zh-Hant", label: LOCALE_DISPLAY_NAMES["zh-Hant"] },
  { code: "en", label: LOCALE_DISPLAY_NAMES.en },
  { code: "ko-KR", label: LOCALE_DISPLAY_NAMES["ko-KR"] },
  { code: "ja-JP", label: LOCALE_DISPLAY_NAMES["ja-JP"] },
] as const;
