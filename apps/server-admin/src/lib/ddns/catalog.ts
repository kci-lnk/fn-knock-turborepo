import type {
  DDNSProviderDefinition,
  DDNSProviderField,
} from "./types";
import { ddnsTranslate } from "./providers/helpers";

const DDNS_PROVIDER_I18N_KEYS: Record<string, string> = {
  baiducloud: "baidu",
  huaweicloud: "huawei",
};

const getDDNSProviderI18nKey = (providerName: string): string =>
  DDNS_PROVIDER_I18N_KEYS[providerName] ?? providerName;

const translateDDNSCatalogText = (
  key: string,
  fallback: string | undefined,
  params?: Record<string, string | number | boolean | null | undefined>,
): string | undefined => {
  if (fallback === undefined) return undefined;
  const fullKey = `server.ddns.${key}`;
  const translated = ddnsTranslate(key, params);
  return translated === fullKey ? fallback : translated;
};

const localizeProviderField = (
  providerKey: string,
  field: DDNSProviderField,
): DDNSProviderField => {
  const fieldParams =
    field.key === "ttl"
      ? {
          seconds: Number(field.placeholder) || 600,
        }
      : undefined;
  const localizeFieldPart = (
    part: "label" | "placeholder" | "description",
    fallback: string | undefined,
  ) => {
    const providerValue = translateDDNSCatalogText(
      `providers.${providerKey}.fields.${field.key}.${part}`,
      fallback,
      fieldParams,
    );
    if (providerValue !== fallback) return providerValue;
    return translateDDNSCatalogText(
      `providers.common.fields.${field.key}.${part}`,
      fallback,
      fieldParams,
    );
  };

  return {
    ...field,
    label: localizeFieldPart("label", field.label) ?? field.label,
    ...(field.placeholder !== undefined
      ? { placeholder: localizeFieldPart("placeholder", field.placeholder) }
      : {}),
    ...(field.description !== undefined
      ? { description: localizeFieldPart("description", field.description) }
      : {}),
    ...(field.options
      ? {
          options: field.options.map((option) => ({
            ...option,
            label:
              translateDDNSCatalogText(
                `providers.${providerKey}.fields.${field.key}.options.${option.value}`,
                option.label,
              ) ?? option.label,
          })),
        }
      : {}),
  };
};

export const localizeProviderDefinition = (
  provider: DDNSProviderDefinition,
): DDNSProviderDefinition => {
  const providerKey = getDDNSProviderI18nKey(provider.name);
  return {
    ...provider,
    label:
      translateDDNSCatalogText(
        `providers.${providerKey}.label`,
        provider.label,
      ) ?? provider.label,
    fields: provider.fields.map((field) =>
      localizeProviderField(providerKey, field),
    ),
  };
};
