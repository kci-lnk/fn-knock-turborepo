import {
  DEFAULT_THEME_COLOR_PRESET_KEY,
  normalizeAppearanceConfig,
  normalizeThemeColorPresetKey,
  type AppearanceConfig,
  type ThemeColorPresetKey,
} from "@admin-shared/utils/appearance";

export const applyThemeColorPreset = (value: unknown) => {
  if (typeof document === "undefined") return;

  const preset = normalizeThemeColorPresetKey(value);
  const root = document.documentElement;

  if (preset === DEFAULT_THEME_COLOR_PRESET_KEY) {
    delete root.dataset.themeColor;
    return;
  }

  root.dataset.themeColor = preset;
};

export const applyAppearanceConfig = (
  value?: Partial<AppearanceConfig> | null,
) => {
  const appearance = normalizeAppearanceConfig(value);
  applyThemeColorPreset(appearance.theme_color_preset);
  return appearance;
};

export type { AppearanceConfig, ThemeColorPresetKey };
