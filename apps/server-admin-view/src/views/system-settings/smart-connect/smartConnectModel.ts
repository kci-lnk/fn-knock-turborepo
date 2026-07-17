import type {
  SmartConnectConfig,
  SmartConnectDetails,
  SmartConnectLocalIpOption,
} from "@/types";

export const cloneSmartConnectDetails = (
  value: SmartConnectDetails,
): SmartConnectDetails => ({
  config: { ...value.config },
  availability: { ...value.availability },
  dnsmasq: {
    ...value.dnsmasq,
    install_state: { ...value.dnsmasq.install_state },
    runtime: {
      ...value.dnsmasq.runtime,
      synced_domains: [...value.dnsmasq.runtime.synced_domains],
    },
  },
  domains: [...value.domains],
  local_ip_options: value.local_ip_options.map((item) => ({ ...item })),
});

export const resolveSelectedIpv4 = (
  configuredValue: string,
  localIpOptions: SmartConnectLocalIpOption[],
): string => configuredValue.trim() || localIpOptions[0]?.value || "";

export const normalizeSmartConnectConfig = (
  value: Partial<SmartConnectConfig>,
): SmartConnectConfig => ({
  enabled: value.enabled === true,
  selected_ipv4: String(value.selected_ipv4 ?? "").trim(),
});

export const getComparableSmartConnectConfig = (
  value: Partial<SmartConnectConfig>,
  persistedSelectedIpv4 = "",
): SmartConnectConfig => {
  const normalized = normalizeSmartConnectConfig(value);
  return normalized.enabled
    ? normalized
    : { ...normalized, selected_ipv4: persistedSelectedIpv4.trim() };
};

export const hasUnsavedSmartConnectDraft = (
  details: SmartConnectDetails | null,
  form: SmartConnectConfig,
): boolean =>
  Boolean(
    details &&
    JSON.stringify(normalizeSmartConnectConfig(details.config)) !==
      JSON.stringify(
        getComparableSmartConnectConfig(form, details.config.selected_ipv4),
      ),
  );
