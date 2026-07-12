import { applyAppearanceConfig } from "@admin-shared/composables/useAppearanceState";
import { setFnKnockLocale } from "@fn-knock/i18n/vue/auth";

type AuthLocaleConfig = {
  default_locale?: string | null;
};

type AuthSystemConfig = {
  appearance?: Parameters<typeof applyAppearanceConfig>[0];
  locale?: AuthLocaleConfig | null;
};

export const useAuthSystemConfig = (i18n: unknown) => {
  const applyAuthLocale = async (value: string | null | undefined) => {
    await setFnKnockLocale(i18n, value);
  };

  const applyAuthSystemConfig = async (
    config: AuthSystemConfig | null | undefined,
  ) => {
    await applyAuthLocale(config?.locale?.default_locale);
    applyAppearanceConfig(config?.appearance);
  };

  return {
    applyAuthLocale,
    applyAuthSystemConfig,
  };
};
