import { computed, type Ref } from "vue";
import type { DDNSNetworkInterfacePayload } from "@/lib/api";
import {
  INTERFACE_IPV4_INDEX_KEY,
  INTERFACE_IPV6_INDEX_KEY,
  IP_SOURCE_KEY,
  IP_SOURCE_OPTIONS,
  NETWORK_INTERFACE_KEY,
  UPDATE_SCOPE_KEY,
  UPDATE_SCOPE_OPTIONS,
  buildNetworkInterfaceAddressOptions,
  isIpSourceOptionDisabled,
  isUpdateScopeOptionDisabled,
  normalizeIpSource,
  normalizeNetworkInterface,
  normalizeUpdateScope,
  resolveNetworkInterfaceOptions,
  shouldShowIPv4ForScope,
  shouldShowIPv6ForScope,
  type DDNSIpSource,
  type DDNSUpdateScope,
  type Provider,
} from "./model";

type TranslateParams = Record<string, string | number>;

export const useDDNSAddressSourceState = ({
  networkInterfaces,
  providerConfig,
  providers,
  selectedProvider,
  statusIpSource,
  statusNetworkInterface,
  statusUpdateScope,
  translate,
}: {
  networkInterfaces: Ref<DDNSNetworkInterfacePayload[]>;
  providerConfig: Ref<Record<string, string>>;
  providers: Ref<Provider[]>;
  selectedProvider: Ref<string>;
  statusIpSource: Ref<DDNSIpSource>;
  statusNetworkInterface: Ref<string>;
  statusUpdateScope: Ref<DDNSUpdateScope>;
  translate: (key: string, params?: TranslateParams) => string;
}) => {
  const formatOptionLabel = (option: { labelKey: string }) =>
    translate(option.labelKey);

  const getUpdateScopeLabel = (value: string | null | undefined) => {
    const updateScope = normalizeUpdateScope(value);
    const option = UPDATE_SCOPE_OPTIONS.find(
      (item) => item.value === updateScope,
    );
    return option
      ? formatOptionLabel(option)
      : translate("admin.ddns.updateScope.dualStack");
  };

  const isProviderUpdateScopeOptionDisabled = (
    providerName: string,
    option: DDNSUpdateScope,
  ) => isUpdateScopeOptionDisabled(providers.value, providerName, option);

  const isProviderIpSourceOptionDisabled = (
    providerName: string,
    option: DDNSIpSource,
  ) => isIpSourceOptionDisabled(providers.value, providerName, option);

  const formatAddressOptionLabel = (
    item: { address: string; family: "ipv4" | "ipv6" },
    index: number,
  ) =>
    translate("admin.ddns.addressOptionLabel", {
      index: index + 1,
      family: item.family === "ipv4" ? "IPv4" : "IPv6",
      address: item.address,
    });

  const currentUpdateScopeLabel = computed(() => {
    return getUpdateScopeLabel(
      providerConfig.value[UPDATE_SCOPE_KEY] || statusUpdateScope.value,
    );
  });

  const currentIpSourceLabel = computed(() => {
    const ipSource = normalizeIpSource(
      providerConfig.value[IP_SOURCE_KEY] || statusIpSource.value,
    );
    const option = IP_SOURCE_OPTIONS.find((item) => item.value === ipSource);
    return option
      ? formatOptionLabel(option)
      : translate("admin.ddns.ipSource.public");
  });

  const selectedNetworkInterface = computed(() => {
    return normalizeNetworkInterface(
      providerConfig.value[NETWORK_INTERFACE_KEY] ||
        statusNetworkInterface.value,
    );
  });

  const configuredNetworkInterface = computed(() => {
    return normalizeNetworkInterface(providerConfig.value[NETWORK_INTERFACE_KEY]);
  });

  const resolvedNetworkInterfaces = computed(() => {
    const selected = selectedNetworkInterface.value;
    return resolveNetworkInterfaceOptions(networkInterfaces.value, selected, {
      label: translate("admin.ddns.unavailableInterfaceLabel", {
        name: selected,
      }),
      summary: translate("admin.ddns.unavailableInterfaceSummary"),
    });
  });

  const currentNetworkInterfaceLabel = computed(() => {
    const selected = selectedNetworkInterface.value;
    if (!selected) {
      return translate("admin.ddns.autoSelect");
    }
    return (
      resolvedNetworkInterfaces.value.find((item) => item.name === selected)
        ?.label || selected
    );
  });

  const configuredNetworkInterfaceLabel = computed(() => {
    const selected = configuredNetworkInterface.value;
    if (!selected) {
      return translate("admin.ddns.autoSelect");
    }
    return (
      resolvedNetworkInterfaces.value.find((item) => item.name === selected)
        ?.label || selected
    );
  });

  const selectedNetworkInterfaceDetail = computed(() => {
    return configuredNetworkInterface.value
      ? configuredNetworkInterfaceLabel.value
      : "";
  });

  const effectiveUpdateScope = computed<DDNSUpdateScope>(() => {
    return normalizeUpdateScope(
      providerConfig.value[UPDATE_SCOPE_KEY] || statusUpdateScope.value,
    );
  });

  const effectiveIpSource = computed<DDNSIpSource>(() => {
    return normalizeIpSource(
      providerConfig.value[IP_SOURCE_KEY] || statusIpSource.value,
    );
  });

  const selectedNetworkInterfaceOption = computed(() => {
    const selected = configuredNetworkInterface.value;
    if (!selected) {
      return null;
    }

    return (
      resolvedNetworkInterfaces.value.find((item) => item.name === selected) ||
      null
    );
  });

  const interfaceIPv4Options = computed(() => {
    return buildNetworkInterfaceAddressOptions(
      selectedNetworkInterfaceOption.value,
      "ipv4",
      formatAddressOptionLabel,
    );
  });

  const interfaceIPv6Options = computed(() => {
    return buildNetworkInterfaceAddressOptions(
      selectedNetworkInterfaceOption.value,
      "ipv6",
      formatAddressOptionLabel,
    );
  });

  const shouldShowInterfaceAddressBlock = computed(
    () => !!selectedProvider.value && effectiveIpSource.value === "interface",
  );
  const shouldShowStaticAddressBlock = computed(
    () => !!selectedProvider.value && effectiveIpSource.value === "static",
  );
  const shouldShowSourceDomainBlock = computed(
    () => !!selectedProvider.value && effectiveIpSource.value === "domain",
  );
  const showStaticIPv4Input = computed(
    () =>
      shouldShowStaticAddressBlock.value &&
      effectiveUpdateScope.value !== "ipv6_only",
  );
  const showStaticIPv6Input = computed(
    () =>
      shouldShowStaticAddressBlock.value &&
      effectiveUpdateScope.value !== "ipv4_only",
  );
  const showInterfaceIPv4Select = computed(
    () =>
      shouldShowInterfaceAddressBlock.value &&
      effectiveUpdateScope.value !== "ipv6_only",
  );
  const showInterfaceIPv6Select = computed(
    () =>
      shouldShowInterfaceAddressBlock.value &&
      effectiveUpdateScope.value !== "ipv4_only",
  );

  const showIPv4Status = computed(() =>
    shouldShowIPv4ForScope(effectiveUpdateScope.value),
  );
  const showIPv6Status = computed(() =>
    shouldShowIPv6ForScope(effectiveUpdateScope.value),
  );

  const updateConfiguredNetworkInterface = (value: string) => {
    providerConfig.value[NETWORK_INTERFACE_KEY] = value;
    providerConfig.value[INTERFACE_IPV4_INDEX_KEY] = "";
    providerConfig.value[INTERFACE_IPV6_INDEX_KEY] = "";
  };

  const updateConfiguredIpSource = (value: string) => {
    providerConfig.value[IP_SOURCE_KEY] = normalizeIpSource(value);
  };

  return {
    configuredNetworkInterface,
    configuredNetworkInterfaceLabel,
    currentIpSourceLabel,
    currentNetworkInterfaceLabel,
    currentUpdateScopeLabel,
    effectiveIpSource,
    effectiveUpdateScope,
    formatAddressOptionLabel,
    formatOptionLabel,
    interfaceIPv4Options,
    interfaceIPv6Options,
    isProviderIpSourceOptionDisabled,
    isProviderUpdateScopeOptionDisabled,
    resolvedNetworkInterfaces,
    selectedNetworkInterfaceDetail,
    shouldShowInterfaceAddressBlock,
    shouldShowSourceDomainBlock,
    shouldShowStaticAddressBlock,
    showIPv4Status,
    showIPv6Status,
    showInterfaceIPv4Select,
    showInterfaceIPv6Select,
    showStaticIPv4Input,
    showStaticIPv6Input,
    updateConfiguredIpSource,
    updateConfiguredNetworkInterface,
  };
};
