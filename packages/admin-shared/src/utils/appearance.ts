export const THEME_COLOR_PRESET_KEYS = [
  "default",
  "hermes_orange",
  "prussian_blue",
] as const;

export type ThemeColorPresetKey = (typeof THEME_COLOR_PRESET_KEYS)[number];

export type ThemeColorPreset = {
  key: ThemeColorPresetKey;
  color: string;
};

export const DEFAULT_THEME_COLOR_PRESET_KEY: ThemeColorPresetKey = "default";

export const THEME_COLOR_PRESETS: readonly ThemeColorPreset[] = [
  { key: "default", color: "#171717" },
  { key: "hermes_orange", color: "#EB5C20" },
  { key: "prussian_blue", color: "#0D3A69" },
];

export interface AppearanceConfig {
  theme_color_preset: ThemeColorPresetKey;
}

export const DEFAULT_APPEARANCE_CONFIG: AppearanceConfig = {
  theme_color_preset: DEFAULT_THEME_COLOR_PRESET_KEY,
};

export const normalizeThemeColorPresetKey = (
  value: unknown,
): ThemeColorPresetKey =>
  THEME_COLOR_PRESET_KEYS.includes(value as ThemeColorPresetKey)
    ? (value as ThemeColorPresetKey)
    : DEFAULT_THEME_COLOR_PRESET_KEY;

export const normalizeAppearanceConfig = (
  value?: Partial<AppearanceConfig> | null,
): AppearanceConfig => ({
  theme_color_preset: normalizeThemeColorPresetKey(value?.theme_color_preset),
});
