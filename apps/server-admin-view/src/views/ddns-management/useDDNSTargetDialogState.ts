import { computed, type Ref } from "vue";
import type { DDNSNetworkInterfacePayload } from "@/lib/api";
import {
  IP_SOURCE_KEY,
  NETWORK_INTERFACE_KEY,
  UPDATE_SCOPE_KEY,
  buildNetworkInterfaceAddressOptions,
  findProviderDef,
  normalizeIpSource,
  normalizeNetworkInterface,
  normalizeUpdateScope,
  resolveNetworkInterfaceOptions,
  type Provider,
  type TargetDialogState,
} from "./model";

type TargetDialogMode = "create" | "edit";
type Translate = (key: string, params?: Record<string, unknown>) => string;
type AddressOptionLabelFormatter = (
  item: { address: string; family: "ipv4" | "ipv6" },
  index: number,
) => string;

export const useDDNSTargetDialogState = ({
  formatAddressOptionLabel,
  mode,
  networkInterfaces,
  providers,
  state,
  translate,
}: {
  formatAddressOptionLabel: AddressOptionLabelFormatter;
  mode: Ref<TargetDialogMode>;
  networkInterfaces: Ref<DDNSNetworkInterfacePayload[]>;
  providers: Ref<Provider[]>;
  state: Ref<TargetDialogState>;
  translate: Translate;
}) => {
  const targetDialogTitle = computed(() =>
    mode.value === "create"
      ? translate("admin.ddns.targetCreateTitle")
      : translate("admin.ddns.targetEditTitle"),
  );

  const targetDialogDescription = computed(() =>
    mode.value === "create"
      ? translate("admin.ddns.targetCreateDescription")
      : translate("admin.ddns.targetEditDescription"),
  );

  const targetDialogProviderDef = computed(() =>
    findProviderDef(providers.value, state.value.provider),
  );

  const targetDialogResolvedNetworkInterfaces = computed(() => {
    const selected = normalizeNetworkInterface(
      state.value.config[NETWORK_INTERFACE_KEY],
    );
    return resolveNetworkInterfaceOptions(networkInterfaces.value, selected, {
      label: translate("admin.ddns.unavailableInterfaceLabel", { name: selected }),
      summary: translate("admin.ddns.unavailableInterfaceSummary"),
    });
  });

  const targetDialogNetworkInterfaceLabel = computed(() => {
    const selected = normalizeNetworkInterface(
      state.value.config[NETWORK_INTERFACE_KEY],
    );
    if (!selected) {
      return translate("admin.ddns.autoSelect");
    }
    return (
      targetDialogResolvedNetworkInterfaces.value.find(
        (item) => item.name === selected,
      )?.label || selected
    );
  });

  const targetDialogNetworkInterfaceOption = computed(() => {
    const selected = normalizeNetworkInterface(
      state.value.config[NETWORK_INTERFACE_KEY],
    );
    if (!selected) {
      return null;
    }
    return (
      targetDialogResolvedNetworkInterfaces.value.find(
        (item) => item.name === selected,
      ) || null
    );
  });

  const targetDialogShouldShowInterfaceBlock = computed(
    () =>
      !!state.value.provider &&
      normalizeIpSource(state.value.config[IP_SOURCE_KEY]) === "interface",
  );

  const targetDialogShouldShowStaticBlock = computed(
    () =>
      !!state.value.provider &&
      normalizeIpSource(state.value.config[IP_SOURCE_KEY]) === "static",
  );

  const targetDialogShouldShowDomainBlock = computed(
    () =>
      !!state.value.provider &&
      normalizeIpSource(state.value.config[IP_SOURCE_KEY]) === "domain",
  );

  const targetDialogUpdateScope = computed(() =>
    normalizeUpdateScope(state.value.config[UPDATE_SCOPE_KEY]),
  );

  const targetDialogIPv4Options = computed(() =>
    buildNetworkInterfaceAddressOptions(
      targetDialogNetworkInterfaceOption.value,
      "ipv4",
      formatAddressOptionLabel,
    ),
  );

  const targetDialogIPv6Options = computed(() =>
    buildNetworkInterfaceAddressOptions(
      targetDialogNetworkInterfaceOption.value,
      "ipv6",
      formatAddressOptionLabel,
    ),
  );

  return {
    targetDialogDescription,
    targetDialogIPv4Options,
    targetDialogIPv6Options,
    targetDialogNetworkInterfaceLabel,
    targetDialogProviderDef,
    targetDialogResolvedNetworkInterfaces,
    targetDialogShouldShowDomainBlock,
    targetDialogShouldShowInterfaceBlock,
    targetDialogShouldShowStaticBlock,
    targetDialogTitle,
    targetDialogUpdateScope,
  };
};
