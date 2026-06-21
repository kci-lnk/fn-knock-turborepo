import {
  DEFAULT_THEME_COLOR_PRESET_KEY,
  normalizeAppearanceConfig,
  normalizeThemeColorPresetKey,
  type AppearanceConfig,
  type ThemeColorPresetKey,
} from "@admin-shared/utils/appearance";
import { readonly, ref } from "vue";

const activeThemeColorPreset = ref<ThemeColorPresetKey>(
  DEFAULT_THEME_COLOR_PRESET_KEY,
);

export const useAppearanceState = () => ({
  activeThemeColorPreset: readonly(activeThemeColorPreset),
});

export const applyThemeColorPreset = (value: unknown) => {
  const preset = normalizeThemeColorPresetKey(value);
  activeThemeColorPreset.value = preset;

  if (typeof document === "undefined") return preset;

  const root = document.documentElement;

  if (preset === DEFAULT_THEME_COLOR_PRESET_KEY) {
    delete root.dataset.themeColor;
    return preset;
  }

  root.dataset.themeColor = preset;
  return preset;
};

export const applyAppearanceConfig = (
  value?: Partial<AppearanceConfig> | null,
) => {
  const appearance = normalizeAppearanceConfig(value);
  applyThemeColorPreset(appearance.theme_color_preset);
  return appearance;
};

export type { AppearanceConfig, ThemeColorPresetKey };
