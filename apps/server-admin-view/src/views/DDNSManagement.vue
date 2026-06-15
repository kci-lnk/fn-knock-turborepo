<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, computed } from "vue";
import { useI18n } from "vue-i18n";
import { DDNSAPI } from "../lib/api";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Cable,
  RefreshCw,
  Trash2,
  Globe,
  Wifi,
  Clock,
  ChevronDown,
  Network,
  Route as RouteIcon,
  Eye,
  EyeOff,
  Plus,
} from "lucide-vue-next";
import CredentialTransferHint from "@/components/CredentialTransferHint.vue";
import LiveStatusBadge from "@/components/LiveStatusBadge.vue";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import { toast } from "@admin-shared/utils/toast";
import LogViewer from "@admin-shared/components/LogViewer.vue";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import OverflowTooltipText from "@admin-shared/components/common/OverflowTooltipText.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";
import {
  DEFAULT_LOG_WINDOW_SIZE,
  mergePollingLogWindow,
} from "@admin-shared/utils/log-window";
import { useDnsCredentialTransfer } from "@/composables/useDnsCredentialTransfer";
import { useTargetPolling } from "../composables/useTargetPolling";
import type {
  DDNSIpSource,
  DDNSNetworkInterfacePayload,
  DDNSUpdateScope,
  DDNSTargetDetailPayload,
  DDNSTargetSummaryPayload,
} from "../lib/api";
import { useConfigStore } from "../store/config";
import { isAnySubdomainRoutingMode } from "../lib/reverse-proxy-submode";
import { docsUrls } from "../lib/docs";
import { buildDDNSTimestampTooltipLines } from "../lib/ddns-time";

interface ProviderField {
  key: string;
  label: string;
  type: "text" | "password" | "select";
  placeholder?: string;
  required?: boolean;
  options?: { label: string; value: string }[];
  description?: string;
}

interface Provider {
  name: string;
  label: string;
  fields: ProviderField[];
  capabilities?: {
    addressMode?: "dual_stack" | "single_address";
    ipSources?: DDNSIpSource[];
  };
}

interface LogEntry {
  time: string;
  level: "info" | "error" | "warn";
  message: string;
}

interface LastIP {
  ipv4: string | null;
  ipv6: string | null;
  updated_at: string | null;
}

interface LastCheck {
  checked_at: string | null;
  outcome: "updated" | "noop" | "skipped" | "error" | null;
  message: string | null;
}

interface TargetDialogState {
  id: string | null;
  name: string;
  enabled: boolean;
  provider: string;
  config: Record<string, string>;
}

const UPDATE_SCOPE_KEY = "update_scope";
const IP_SOURCE_KEY = "ip_source";
const NETWORK_INTERFACE_KEY = "network_interface";
const INTERFACE_IPV4_INDEX_KEY = "interface_ipv4_index";
const INTERFACE_IPV6_INDEX_KEY = "interface_ipv6_index";
const STATIC_IPV4_KEY = "static_ipv4";
const STATIC_IPV6_KEY = "static_ipv6";
const SOURCE_DOMAIN_KEY = "source_domain";
const NETWORK_INTERFACE_AUTO_VALUE = "__auto__";
const DEFAULT_DDNS_UPDATE_SCOPE: DDNSUpdateScope = "dual_stack";
const DEFAULT_DDNS_IP_SOURCE: DDNSIpSource = "public";
const DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES = 10;
const MIN_DDNS_UPDATE_INTERVAL_MINUTES = 5;
const MAX_DDNS_UPDATE_INTERVAL_MINUTES = 1440;
const UPDATE_SCOPE_OPTIONS: Array<{
  labelKey: string;
  value: DDNSUpdateScope;
}> = [
  { labelKey: "admin.ddns.updateScope.dualStack", value: "dual_stack" },
  { labelKey: "admin.ddns.updateScope.ipv6Only", value: "ipv6_only" },
  { labelKey: "admin.ddns.updateScope.ipv4Only", value: "ipv4_only" },
];
const IP_SOURCE_OPTIONS: Array<{ labelKey: string; value: DDNSIpSource }> = [
  { labelKey: "admin.ddns.ipSource.public", value: "public" },
  { labelKey: "admin.ddns.ipSource.interface", value: "interface" },
  { labelKey: "admin.ddns.ipSource.static", value: "static" },
  { labelKey: "admin.ddns.ipSource.domain", value: "domain" },
];

const { t, locale } = useI18n();

const formatOptionLabel = (option: { labelKey: string }) => t(option.labelKey);

const normalizeUpdateScope = (
  value: string | null | undefined,
): DDNSUpdateScope => {
  if (
    value === "dual_stack" ||
    value === "ipv6_only" ||
    value === "ipv4_only"
  ) {
    return value;
  }
  return DEFAULT_DDNS_UPDATE_SCOPE;
};

const normalizeIpSource = (value: string | null | undefined): DDNSIpSource => {
  if (value === "interface" || value === "static" || value === "domain") {
    return value;
  }
  return DEFAULT_DDNS_IP_SOURCE;
};

const normalizeNetworkInterface = (value: string | null | undefined) => {
  return value?.trim() || "";
};

const normalizeInterfaceAddressIndex = (value: string | null | undefined) => {
  const trimmed = value?.trim() || "";
  if (!trimmed) {
    return "";
  }

  const parsed = Number(trimmed);
  if (!Number.isInteger(parsed) || parsed < 0) {
    return "";
  }

  return String(parsed);
};

const normalizeStaticIPAddress = (value: string | null | undefined) => {
  return value?.trim() || "";
};

const normalizeSourceDomain = (value: string | null | undefined) => {
  return (value || "").trim().replace(/\.+$/, "");
};

const normalizeUpdateIntervalMinutes = (value: unknown) => {
  const parsed = Number(value);
  if (
    Number.isInteger(parsed) &&
    parsed >= MIN_DDNS_UPDATE_INTERVAL_MINUTES &&
    parsed <= MAX_DDNS_UPDATE_INTERVAL_MINUTES
  ) {
    return parsed;
  }

  return DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES;
};

const toNetworkInterfaceSelectValue = (value: string | null | undefined) => {
  return normalizeNetworkInterface(value) || NETWORK_INTERFACE_AUTO_VALUE;
};

const getUpdateScopeLabel = (value: string | null | undefined) => {
  return UPDATE_SCOPE_OPTIONS.find(
    (option) => option.value === normalizeUpdateScope(value),
  )
    ? formatOptionLabel(
        UPDATE_SCOPE_OPTIONS.find(
          (option) => option.value === normalizeUpdateScope(value),
        )!,
      )
    : t("admin.ddns.updateScope.dualStack");
};

const normalizeTargetConfigValues = (
  config: Record<string, string> | null | undefined,
): Record<string, string> => ({
  ...(config || {}),
  [UPDATE_SCOPE_KEY]: normalizeUpdateScope(config?.[UPDATE_SCOPE_KEY]),
  [IP_SOURCE_KEY]: normalizeIpSource(config?.[IP_SOURCE_KEY]),
  [NETWORK_INTERFACE_KEY]: normalizeNetworkInterface(
    config?.[NETWORK_INTERFACE_KEY],
  ),
  [INTERFACE_IPV4_INDEX_KEY]: normalizeInterfaceAddressIndex(
    config?.[INTERFACE_IPV4_INDEX_KEY],
  ),
  [INTERFACE_IPV6_INDEX_KEY]: normalizeInterfaceAddressIndex(
    config?.[INTERFACE_IPV6_INDEX_KEY],
  ),
  [STATIC_IPV4_KEY]: normalizeStaticIPAddress(config?.[STATIC_IPV4_KEY]),
  [STATIC_IPV6_KEY]: normalizeStaticIPAddress(config?.[STATIC_IPV6_KEY]),
  [SOURCE_DOMAIN_KEY]: normalizeSourceDomain(config?.[SOURCE_DOMAIN_KEY]),
});

const extractCommonTargetConfig = (
  config: Record<string, string>,
): Record<string, string> => ({
  [UPDATE_SCOPE_KEY]: normalizeUpdateScope(config[UPDATE_SCOPE_KEY]),
  [IP_SOURCE_KEY]: normalizeIpSource(config[IP_SOURCE_KEY]),
  [NETWORK_INTERFACE_KEY]: normalizeNetworkInterface(
    config[NETWORK_INTERFACE_KEY],
  ),
  [INTERFACE_IPV4_INDEX_KEY]: normalizeInterfaceAddressIndex(
    config[INTERFACE_IPV4_INDEX_KEY],
  ),
  [INTERFACE_IPV6_INDEX_KEY]: normalizeInterfaceAddressIndex(
    config[INTERFACE_IPV6_INDEX_KEY],
  ),
  [STATIC_IPV4_KEY]: normalizeStaticIPAddress(config[STATIC_IPV4_KEY]),
  [STATIC_IPV6_KEY]: normalizeStaticIPAddress(config[STATIC_IPV6_KEY]),
  [SOURCE_DOMAIN_KEY]: normalizeSourceDomain(config[SOURCE_DOMAIN_KEY]),
});

const resolveNetworkInterfaceOptions = (
  items: DDNSNetworkInterfacePayload[],
  selected: string,
) => {
  const resolved = [...items];
  if (selected && !resolved.some((item) => item.name === selected)) {
    resolved.push({
      name: selected,
      label: t("admin.ddns.unavailableInterfaceLabel", { name: selected }),
      summary: t("admin.ddns.unavailableInterfaceSummary"),
      hasIpv4: false,
      hasIpv6: false,
      addresses: [],
      selectableAddresses: [],
    });
  }
  return resolved;
};

// ─── State ─────────────────────────────────────────────────────
const isInitialized = ref(false);
const configStore = useConfigStore();
const enabled = ref(true);
const selectedProvider = ref<string>("");
const providers = ref<Provider[]>([]);
const providerConfig = ref<Record<string, string>>({});
const savedProviderConfig = ref<Record<string, string>>({});
const lastIP = ref<LastIP>({ ipv4: null, ipv6: null, updated_at: null });
const lastCheck = ref<LastCheck>({
  checked_at: null,
  outcome: null,
  message: null,
});
const updateIntervalMinutes = ref(DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES);
const updateIntervalDraft = ref(String(DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES));
const logs = ref<LogEntry[]>([]);
const statusUpdateScope = ref<DDNSUpdateScope>(DEFAULT_DDNS_UPDATE_SCOPE);
const statusIpSource = ref<DDNSIpSource>(DEFAULT_DDNS_IP_SOURCE);
const statusNetworkInterface = ref("");
const networkInterfaces = ref<DDNSNetworkInterfacePayload[]>([]);
const targetSummaries = ref<DDNSTargetSummaryPayload[]>([]);
const showTargetDialog = ref(false);
const showUpdateIntervalDialog = ref(false);
const showClearPrimaryConfigDialog = ref(false);
const targetDialogMode = ref<"create" | "edit">("create");
const targetDialogState = ref<TargetDialogState>({
  id: null,
  name: "",
  enabled: true,
  provider: "",
  config: normalizeTargetConfigValues({}),
});
const testingTargetId = ref("");
const deletingTargetId = ref("");
const togglingTargetId = ref("");
const pendingPrimaryConfigCollapse = ref<(() => void) | null>(null);

const { isPending: isSaving, run: runSaveConfig } = useAsyncAction({
  rethrow: true,
  onError: (error) => {
    toast.error(t("admin.ddns.saveConfigFailed"), {
      description: extractErrorMessage(error, t("admin.ddns.saveConfigFailed")),
    });
  },
});
const { isPending: isClearingPrimaryConfig, run: runClearPrimaryConfig } =
  useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.ddns.clearPrimaryConfigFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.ddns.clearPrimaryConfigFailed"),
        ),
      });
    },
  });
const { isPending: isTesting, run: runTestUpdate } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.updateFailed"), {
      description: extractErrorMessage(error, t("admin.ddns.updateFailed")),
    });
  },
});
const { isPending: isClearingLogs, run: runClearLogs } = useAsyncAction({
  onError: () => {
    toast.error(t("admin.ddns.clearLogsFailed"));
  },
});
const { isPending: isTogglingEnabled, run: runToggleEnabled } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.toggleFailed"), {
      description: extractErrorMessage(error, t("admin.ddns.toggleFailed")),
    });
  },
});
const { isPending: isSwitchingProvider, run: runSwitchProvider } =
  useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.ddns.switchProviderFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.ddns.switchProviderFailed"),
        ),
      });
    },
  });
const { run: runLoadStatus } = useAsyncAction({
  onError: (error) => {
    console.error(
      "loadStatus:",
      extractErrorMessage(error, t("admin.ddns.loadStatusFailed")),
    );
  },
});
const { run: runLoadProviders } = useAsyncAction({
  onError: (error) => {
    console.error(
      "loadProviders:",
      extractErrorMessage(error, t("admin.ddns.loadProvidersFailed")),
    );
  },
});
const { run: runLoadNetworkInterfaces } = useAsyncAction({
  onError: (error) => {
    console.error(
      "loadNetworkInterfaces:",
      extractErrorMessage(error, t("admin.ddns.loadInterfacesFailed")),
    );
  },
});
const { run: runLoadConfig } = useAsyncAction({
  onError: (error) => {
    console.error(
      "loadConfig:",
      extractErrorMessage(error, t("admin.ddns.loadConfigFailed")),
    );
  },
});
const { isPending: isLoading, run: runInitialize } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.initFailed"), {
      description: extractErrorMessage(error, t("admin.ddns.initLoadFailed")),
    });
  },
});
const { isPending: isSavingTarget, run: runSaveTarget } = useAsyncAction({
  rethrow: true,
  onError: (error) => {
    toast.error(t("admin.ddns.saveTargetFailed"), {
      description: extractErrorMessage(error, t("admin.ddns.saveTargetFailed")),
    });
  },
});
const { isPending: isSavingUpdateInterval, run: runSaveUpdateInterval } =
  useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.ddns.saveIntervalFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.ddns.saveIntervalFailed"),
        ),
      });
    },
  });
const { run: runDeleteTarget } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.deleteTargetFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.ddns.deleteTargetFailed"),
      ),
    });
  },
});
const { run: runToggleTarget } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.toggleTargetFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.ddns.toggleTargetFailed"),
      ),
    });
  },
});
const { run: runTestTarget } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.testTargetFailed"), {
      description: extractErrorMessage(error, t("admin.ddns.testTargetFailed")),
    });
  },
});

const fieldVisibility = ref<Record<string, boolean>>({});
const targetFieldVisibility = ref<Record<string, boolean>>({});
const fieldEditReady = ref<Record<string, boolean>>({});

const toggleFieldVisibility = (key: string) => {
  fieldVisibility.value[key] = !fieldVisibility.value[key];
};

const getTargetFieldStateKey = (key: string) =>
  `${targetDialogState.value.provider}:${targetDialogState.value.id || "new"}:${key}`;

const toggleTargetFieldVisibility = (key: string) => {
  const stateKey = getTargetFieldStateKey(key);
  targetFieldVisibility.value[stateKey] =
    !targetFieldVisibility.value[stateKey];
};

const isTargetFieldVisible = (key: string) => {
  return targetFieldVisibility.value[getTargetFieldStateKey(key)] === true;
};

const getFieldStateKey = (key: string) => `${selectedProvider.value}:${key}`;

const getFieldDomId = (index: number) => `ddns-field-${index}`;

const getFieldInputName = (index: number) => `ddns-input-${index}`;

const enableFieldEditing = (key: string) => {
  fieldEditReady.value[getFieldStateKey(key)] = true;
};

const isFieldEditReady = (key: string) => {
  return fieldEditReady.value[getFieldStateKey(key)] === true;
};

const getFieldAutocomplete = (field: ProviderField) => {
  const normalizedKey = field.key.toLowerCase();
  if (
    field.type === "password" ||
    /access|account|auth|credential|email|key|login|secret|token|user/.test(
      normalizedKey,
    )
  ) {
    return "new-password";
  }

  return "off";
};

const currentProviderDef = computed(() => {
  return providers.value.find((p) => p.name === selectedProvider.value) || null;
});

const findProviderDef = (providerName: string) =>
  providers.value.find((provider) => provider.name === providerName) || null;

const isSingleAddressProvider = (providerName: string) =>
  findProviderDef(providerName)?.capabilities?.addressMode === "single_address";

const isUpdateScopeOptionDisabled = (
  providerName: string,
  option: DDNSUpdateScope,
) => isSingleAddressProvider(providerName) && option === "dual_stack";

const isIpSourceOptionDisabled = (
  providerName: string,
  option: DDNSIpSource,
) => {
  const supportedSources =
    findProviderDef(providerName)?.capabilities?.ipSources;
  return Array.isArray(supportedSources) && !supportedSources.includes(option);
};

const isLikelyIPv4Address = (value: string) => {
  const parts = value.trim().split(".");
  return (
    parts.length === 4 &&
    parts.every((part) => {
      if (!/^\d{1,3}$/.test(part)) return false;
      const parsed = Number(part);
      return Number.isInteger(parsed) && parsed >= 0 && parsed <= 255;
    })
  );
};

const isLikelyIPv6Address = (value: string) => {
  const address = value.trim();
  return (
    address.includes(":") &&
    address.length <= 45 &&
    /^[0-9a-f:.]+$/i.test(address)
  );
};

const isLikelySourceDomain = (value: string) => {
  const domain = normalizeSourceDomain(value);
  if (!domain || domain.length > 253) return false;
  if (/^https?:\/\//i.test(value) || /[\s/:*]/.test(domain)) {
    return false;
  }
  return domain.split(".").every((label) => {
    return (
      label.length > 0 &&
      label.length <= 63 &&
      !label.startsWith("-") &&
      !label.endsWith("-") &&
      /^[a-z0-9-]+$/i.test(label)
    );
  });
};

const extraTargets = computed(() =>
  targetSummaries.value.filter((target) => !target.isPrimary),
);

const hasExtraTargets = computed(() => extraTargets.value.length > 0);

const targetDialogTitle = computed(() =>
  targetDialogMode.value === "create"
    ? t("admin.ddns.targetCreateTitle")
    : t("admin.ddns.targetEditTitle"),
);

const targetDialogDescription = computed(() =>
  targetDialogMode.value === "create"
    ? t("admin.ddns.targetCreateDescription")
    : t("admin.ddns.targetEditDescription"),
);

const targetDialogProviderDef = computed(() =>
  findProviderDef(targetDialogState.value.provider),
);

const targetDialogResolvedNetworkInterfaces = computed(() =>
  resolveNetworkInterfaceOptions(
    networkInterfaces.value,
    normalizeNetworkInterface(
      targetDialogState.value.config[NETWORK_INTERFACE_KEY],
    ),
  ),
);

const targetDialogNetworkInterfaceLabel = computed(() => {
  const selected = normalizeNetworkInterface(
    targetDialogState.value.config[NETWORK_INTERFACE_KEY],
  );
  if (!selected) {
    return t("admin.ddns.autoSelect");
  }
  return (
    targetDialogResolvedNetworkInterfaces.value.find(
      (item) => item.name === selected,
    )?.label || selected
  );
});

const targetDialogNetworkInterfaceOption = computed(() => {
  const selected = normalizeNetworkInterface(
    targetDialogState.value.config[NETWORK_INTERFACE_KEY],
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
    !!targetDialogState.value.provider &&
    normalizeIpSource(targetDialogState.value.config[IP_SOURCE_KEY]) ===
      "interface",
);

const targetDialogShouldShowStaticBlock = computed(
  () =>
    !!targetDialogState.value.provider &&
    normalizeIpSource(targetDialogState.value.config[IP_SOURCE_KEY]) ===
      "static",
);

const targetDialogShouldShowDomainBlock = computed(
  () =>
    !!targetDialogState.value.provider &&
    normalizeIpSource(targetDialogState.value.config[IP_SOURCE_KEY]) ===
      "domain",
);

const targetDialogUpdateScope = computed(() =>
  normalizeUpdateScope(targetDialogState.value.config[UPDATE_SCOPE_KEY]),
);

const shouldShowTargetIPv4Status = (target: DDNSTargetSummaryPayload) =>
  normalizeUpdateScope(target.updateScope) !== "ipv6_only";

const shouldShowTargetIPv6Status = (target: DDNSTargetSummaryPayload) =>
  normalizeUpdateScope(target.updateScope) !== "ipv4_only";

const getTargetDisplayName = (target: DDNSTargetSummaryPayload) =>
  target.name || target.domainSummary || target.providerLabel;

const getTargetLastCheckTooltipLines = (target: DDNSTargetSummaryPayload) =>
  buildDDNSTimestampTooltipLines({
    updatedAt: target.lastIP.updated_at,
    checkedAt: target.lastCheck.checked_at,
    locale: String(locale.value),
    labels: {
      lastSuccessfulUpdate: t("admin.ddns.lastSuccessfulUpdate"),
      lastCheck: t("admin.ddns.lastCheck"),
      never: t("admin.ddns.never"),
    },
  });

const targetDialogIPv4Options = computed(() => {
  return (targetDialogNetworkInterfaceOption.value?.selectableAddresses || [])
    .filter((item) => item.family === "ipv4")
    .map((item, index) => ({
      value: String(index),
      label: t("admin.ddns.addressOptionLabel", {
        index: index + 1,
        family: "IPv4",
        address: item.address,
      }),
    }));
});

const targetDialogIPv6Options = computed(() => {
  return (targetDialogNetworkInterfaceOption.value?.selectableAddresses || [])
    .filter((item) => item.family === "ipv6")
    .map((item, index) => ({
      value: String(index),
      label: t("admin.ddns.addressOptionLabel", {
        index: index + 1,
        family: "IPv6",
        address: item.address,
      }),
    }));
});

const {
  applySuggestion: applyTransferredCredentials,
  isLoadingSource: isTransferSourceLoading,
  sourceScopeLabel: transferSourceScopeLabel,
  suggestion: credentialTransferSuggestion,
} = useDnsCredentialTransfer({
  target: "ddns",
  providerId: selectedProvider,
  targetCredentials: providerConfig,
});

const credentialTransferDescription = computed(() => {
  const suggestion = credentialTransferSuggestion.value;
  if (!suggestion) return "";

  return t("admin.ddns.credentialTransferDescription", {
    scope: transferSourceScopeLabel.value,
    bridge: suggestion.bridgeLabel,
    count: suggestion.fillableFields.length,
  });
});

const hasProviderConfig = computed(() => {
  const def = currentProviderDef.value;
  if (!def) return false;
  return def.fields.some(
    (field) => providerConfig.value[field.key]?.toString().trim() !== "",
  );
});

const hasSavedProviderConfig = computed(() => {
  const def = currentProviderDef.value;
  if (!def) return false;
  return def.fields.some(
    (field) => savedProviderConfig.value[field.key]?.toString().trim() !== "",
  );
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
  return IP_SOURCE_OPTIONS.find((option) => option.value === ipSource)
    ? formatOptionLabel(
        IP_SOURCE_OPTIONS.find((option) => option.value === ipSource)!,
      )
    : t("admin.ddns.ipSource.public");
});

const selectedNetworkInterface = computed(() => {
  return normalizeNetworkInterface(
    providerConfig.value[NETWORK_INTERFACE_KEY] || statusNetworkInterface.value,
  );
});

const configuredNetworkInterface = computed(() => {
  return normalizeNetworkInterface(providerConfig.value[NETWORK_INTERFACE_KEY]);
});

const resolvedNetworkInterfaces = computed(() => {
  const items = [...networkInterfaces.value];
  const selected = selectedNetworkInterface.value;
  if (selected && !items.some((item) => item.name === selected)) {
    items.push({
      name: selected,
      label: t("admin.ddns.unavailableInterfaceLabel", { name: selected }),
      summary: t("admin.ddns.unavailableInterfaceSummary"),
      hasIpv4: false,
      hasIpv6: false,
      addresses: [],
      selectableAddresses: [],
    });
  }
  return items;
});

const currentNetworkInterfaceLabel = computed(() => {
  const selected = selectedNetworkInterface.value;
  if (!selected) {
    return t("admin.ddns.autoSelect");
  }
  return (
    resolvedNetworkInterfaces.value.find((item) => item.name === selected)
      ?.label || selected
  );
});

const configuredNetworkInterfaceLabel = computed(() => {
  const selected = configuredNetworkInterface.value;
  if (!selected) {
    return t("admin.ddns.autoSelect");
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
  return (selectedNetworkInterfaceOption.value?.selectableAddresses || [])
    .filter((item) => item.family === "ipv4")
    .map((item, index) => ({
      value: String(index),
      label: t("admin.ddns.addressOptionLabel", {
        index: index + 1,
        family: "IPv4",
        address: item.address,
      }),
    }));
});

const interfaceIPv6Options = computed(() => {
  return (selectedNetworkInterfaceOption.value?.selectableAddresses || [])
    .filter((item) => item.family === "ipv6")
    .map((item, index) => ({
      value: String(index),
      label: t("admin.ddns.addressOptionLabel", {
        index: index + 1,
        family: "IPv6",
        address: item.address,
      }),
    }));
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
const hasConfiguredInterfaceIPv4Selection = computed(() =>
  normalizeInterfaceAddressIndex(
    providerConfig.value[INTERFACE_IPV4_INDEX_KEY],
  ),
);
const hasConfiguredInterfaceIPv6Selection = computed(() =>
  normalizeInterfaceAddressIndex(
    providerConfig.value[INTERFACE_IPV6_INDEX_KEY],
  ),
);

const showIPv4Status = computed(
  () => effectiveUpdateScope.value !== "ipv6_only",
);
const showIPv6Status = computed(
  () => effectiveUpdateScope.value !== "ipv4_only",
);
const isEnabledSwitchDisabled = computed(
  () => isTogglingEnabled.value || isLoading.value,
);
const isProviderSelectDisabled = computed(
  () => isSwitchingProvider.value || isLoading.value,
);
const isSubdomainMode = computed(() =>
  isAnySubdomainRoutingMode(configStore.config),
);
const updateIntervalLabel = computed(() =>
  t("admin.ddns.updateIntervalLabel", {
    minutes: updateIntervalMinutes.value,
  }),
);

const getFieldDescription = (field: ProviderField) => {
  const description = field.description?.trim() || "";

  if (isSubdomainMode.value && field.key === "domain") {
    const wildcardHint = t("admin.ddns.wildcardHint");
    return description ? `${description} ${wildcardHint}` : wildcardHint;
  }

  return description;
};

async function loadStatus() {
  await runLoadStatus(async () => {
    const status = await DDNSAPI.getStatus();
    enabled.value = status.enabled;
    selectedProvider.value = status.provider || "";
    lastIP.value = status.lastIP;
    lastCheck.value = status.lastCheck;
    updateIntervalMinutes.value = normalizeUpdateIntervalMinutes(
      status.updateIntervalMinutes,
    );
    statusUpdateScope.value = normalizeUpdateScope(status.updateScope);
    statusIpSource.value = normalizeIpSource(status.ipSource);
    statusNetworkInterface.value = normalizeNetworkInterface(
      status.networkInterface,
    );
    targetSummaries.value = status.targets || [];
  });
}

async function loadProviders() {
  await runLoadProviders(async () => {
    const data = await DDNSAPI.getProviders();
    providers.value = data.map((p) => ({
      ...p,
      fields: p.fields.map((f) => ({
        ...f,
        type: f.type as "text" | "password" | "select",
      })),
    }));
  });
}

async function loadNetworkInterfaces() {
  await runLoadNetworkInterfaces(async () => {
    networkInterfaces.value = await DDNSAPI.getNetworkInterfaces();
  });
}

async function loadConfig() {
  if (!selectedProvider.value) {
    providerConfig.value = {};
    savedProviderConfig.value = {};
    return;
  }
  await runLoadConfig(async () => {
    const config = await DDNSAPI.getConfig(selectedProvider.value);
    const def = currentProviderDef.value;
    const merged: Record<string, string> = {
      [UPDATE_SCOPE_KEY]: normalizeUpdateScope(config[UPDATE_SCOPE_KEY]),
      [IP_SOURCE_KEY]: normalizeIpSource(config[IP_SOURCE_KEY]),
      [NETWORK_INTERFACE_KEY]: normalizeNetworkInterface(
        config[NETWORK_INTERFACE_KEY],
      ),
      [INTERFACE_IPV4_INDEX_KEY]: normalizeInterfaceAddressIndex(
        config[INTERFACE_IPV4_INDEX_KEY],
      ),
      [INTERFACE_IPV6_INDEX_KEY]: normalizeInterfaceAddressIndex(
        config[INTERFACE_IPV6_INDEX_KEY],
      ),
      [STATIC_IPV4_KEY]: normalizeStaticIPAddress(config[STATIC_IPV4_KEY]),
      [STATIC_IPV6_KEY]: normalizeStaticIPAddress(config[STATIC_IPV6_KEY]),
      [SOURCE_DOMAIN_KEY]: normalizeSourceDomain(config[SOURCE_DOMAIN_KEY]),
    };

    fieldEditReady.value = {};

    if (def) {
      for (const f of def.fields) {
        const val = config[f.key] ?? "";
        merged[f.key] = val;
        if (f.type === "password" && !(f.key in fieldVisibility.value)) {
          fieldVisibility.value[f.key] = true;
        }
      }
    }
    providerConfig.value = merged;
    savedProviderConfig.value = { ...merged };
  });
}

const ddnsPolling = useTargetPolling({
  target: "ddns",
  intervalMs: 2000,
  onData: (payload) => {
    logs.value = mergePollingLogWindow(logs.value, payload.logs as LogEntry[], {
      reset: payload.reset,
      max: DEFAULT_LOG_WINDOW_SIZE,
    });

    const status = payload.status;
    lastIP.value = status.lastIP;
    lastCheck.value = status.lastCheck;
    updateIntervalMinutes.value = normalizeUpdateIntervalMinutes(
      status.updateIntervalMinutes,
    );
    statusUpdateScope.value = normalizeUpdateScope(status.updateScope);
    statusIpSource.value = normalizeIpSource(status.ipSource);
    statusNetworkInterface.value = normalizeNetworkInterface(
      status.networkInterface,
    );
    targetSummaries.value = status.targets || [];
    selectedProvider.value = status.provider || "";
    if (enabledInitialized && status.enabled !== enabled.value) {
      enabledInitialized = false;
      enabled.value = status.enabled;
      enabledInitialized = true;
    }
  },
  onError: (error) => {
    console.error(
      "ddns poll:",
      extractErrorMessage(error, t("admin.ddns.pollStatusFailed")),
    );
  },
});

let enabledInitialized = false;
watch(enabled, async (val) => {
  if (!enabledInitialized) return;
  await runToggleEnabled(() => DDNSAPI.toggle(val), {
    onSuccess: () => {
      toast.success(val ? t("admin.ddns.enabled") : t("admin.ddns.disabled"));
    },
    onError: () => {
      enabledInitialized = false;
      enabled.value = !val;
      enabledInitialized = true;
    },
  });
});

function openUpdateIntervalDialog() {
  updateIntervalDraft.value = String(updateIntervalMinutes.value);
  showUpdateIntervalDialog.value = true;
}

function parseUpdateIntervalDraft() {
  const trimmed = String(updateIntervalDraft.value ?? "").trim();
  if (!/^\d+$/.test(trimmed)) {
    return null;
  }

  const parsed = Number(trimmed);
  if (
    !Number.isInteger(parsed) ||
    parsed < MIN_DDNS_UPDATE_INTERVAL_MINUTES ||
    parsed > MAX_DDNS_UPDATE_INTERVAL_MINUTES
  ) {
    return null;
  }

  return parsed;
}

async function saveUpdateInterval() {
  const next = parseUpdateIntervalDraft();
  if (next === null) {
    toast.error(t("admin.ddns.intervalInvalid"), {
      description: t("admin.ddns.intervalInvalidDescription", {
        min: MIN_DDNS_UPDATE_INTERVAL_MINUTES,
        max: MAX_DDNS_UPDATE_INTERVAL_MINUTES,
      }),
    });
    return;
  }

  await runSaveUpdateInterval(
    () => DDNSAPI.saveSettings({ updateIntervalMinutes: next }),
    {
      onSuccess: (settings) => {
        updateIntervalMinutes.value = normalizeUpdateIntervalMinutes(
          settings.updateIntervalMinutes,
        );
        updateIntervalDraft.value = String(updateIntervalMinutes.value);
        showUpdateIntervalDialog.value = false;
        toast.success(t("admin.ddns.intervalSaved"));
      },
    },
  );
}

async function onProviderChange(val: string) {
  if (!val || val === selectedProvider.value) return;
  await runSwitchProvider(async () => {
    await DDNSAPI.setProvider(val);
    selectedProvider.value = val;
    await loadConfig();
  });
}

function updateConfiguredNetworkInterface(value: string) {
  providerConfig.value[NETWORK_INTERFACE_KEY] = value;
  providerConfig.value[INTERFACE_IPV4_INDEX_KEY] = "";
  providerConfig.value[INTERFACE_IPV6_INDEX_KEY] = "";
}

function updateConfiguredIpSource(value: string) {
  providerConfig.value[IP_SOURCE_KEY] = normalizeIpSource(value);
}

function validateCommonConfig() {
  if (
    isSingleAddressProvider(selectedProvider.value) &&
    effectiveUpdateScope.value === "dual_stack"
  ) {
    toast.error(t("admin.ddns.singleAddressProviderRequiresSingleStack"));
    return false;
  }

  if (effectiveIpSource.value === "static") {
    const ipv4 = normalizeStaticIPAddress(
      providerConfig.value[STATIC_IPV4_KEY],
    );
    const ipv6 = normalizeStaticIPAddress(
      providerConfig.value[STATIC_IPV6_KEY],
    );
    const needsIPv4 = effectiveUpdateScope.value !== "ipv6_only";
    const needsIPv6 = effectiveUpdateScope.value !== "ipv4_only";

    if (needsIPv4 && ipv4 && !isLikelyIPv4Address(ipv4)) {
      toast.error(t("admin.ddns.invalidStaticIpv4"));
      return false;
    }
    if (needsIPv6 && ipv6 && !isLikelyIPv6Address(ipv6)) {
      toast.error(t("admin.ddns.invalidStaticIpv6"));
      return false;
    }
    if (effectiveUpdateScope.value === "ipv4_only" && !ipv4) {
      toast.error(t("admin.ddns.enterStaticIpv4"));
      return false;
    }
    if (effectiveUpdateScope.value === "ipv6_only" && !ipv6) {
      toast.error(t("admin.ddns.enterStaticIpv6"));
      return false;
    }
    if (effectiveUpdateScope.value === "dual_stack" && !ipv4 && !ipv6) {
      toast.error(t("admin.ddns.enterStaticIp"));
      return false;
    }
    return true;
  }

  if (effectiveIpSource.value === "domain") {
    const domain = normalizeSourceDomain(
      providerConfig.value[SOURCE_DOMAIN_KEY],
    );
    if (!domain) {
      toast.error(t("admin.ddns.enterSourceDomain"));
      return false;
    }
    if (!isLikelySourceDomain(domain)) {
      toast.error(t("admin.ddns.invalidSourceDomain"));
      return false;
    }
    return true;
  }

  if (effectiveIpSource.value !== "interface") {
    return true;
  }

  if (!configuredNetworkInterface.value) {
    toast.error(t("admin.ddns.chooseInterface"), {
      description: t("admin.ddns.chooseInterfaceDescription"),
    });
    return false;
  }

  if (
    effectiveUpdateScope.value === "ipv4_only" &&
    interfaceIPv4Options.value.length === 0
  ) {
    toast.error(t("admin.ddns.noIpv4Available"), {
      description: t("admin.ddns.addressFilteredDescription"),
    });
    return false;
  }

  if (
    effectiveUpdateScope.value === "ipv6_only" &&
    interfaceIPv6Options.value.length === 0
  ) {
    toast.error(t("admin.ddns.noIpv6Available"), {
      description: t("admin.ddns.addressFilteredDescription"),
    });
    return false;
  }

  if (
    effectiveUpdateScope.value === "dual_stack" &&
    interfaceIPv4Options.value.length === 0 &&
    interfaceIPv6Options.value.length === 0
  ) {
    toast.error(t("admin.ddns.noAddressAvailable"), {
      description: t("admin.ddns.addressFilteredDescription"),
    });
    return false;
  }

  if (
    showInterfaceIPv4Select.value &&
    interfaceIPv4Options.value.length > 0 &&
    !hasConfiguredInterfaceIPv4Selection.value
  ) {
    toast.error(t("admin.ddns.chooseIpv4"), {
      description: t("admin.ddns.chooseIpv4Description"),
    });
    return false;
  }

  if (
    showInterfaceIPv4Select.value &&
    hasConfiguredInterfaceIPv4Selection.value &&
    !interfaceIPv4Options.value.some(
      (option) => option.value === hasConfiguredInterfaceIPv4Selection.value,
    )
  ) {
    toast.error(t("admin.ddns.ipv4Unavailable"), {
      description: t("admin.ddns.ipv4UnavailableDescription"),
    });
    return false;
  }

  if (
    showInterfaceIPv6Select.value &&
    interfaceIPv6Options.value.length > 0 &&
    !hasConfiguredInterfaceIPv6Selection.value
  ) {
    toast.error(t("admin.ddns.chooseIpv6"), {
      description: t("admin.ddns.chooseIpv6Description"),
    });
    return false;
  }

  if (
    showInterfaceIPv6Select.value &&
    hasConfiguredInterfaceIPv6Selection.value &&
    !interfaceIPv6Options.value.some(
      (option) => option.value === hasConfiguredInterfaceIPv6Selection.value,
    )
  ) {
    toast.error(t("admin.ddns.ipv6Unavailable"), {
      description: t("admin.ddns.ipv6UnavailableDescription"),
    });
    return false;
  }

  return true;
}

function resetTargetDialogState(next?: Partial<TargetDialogState>) {
  targetFieldVisibility.value = {};
  targetDialogState.value = {
    id: next?.id ?? null,
    name: next?.name ?? "",
    enabled: next?.enabled ?? true,
    provider: next?.provider ?? selectedProvider.value,
    config: normalizeTargetConfigValues(
      next?.config ?? extractCommonTargetConfig(providerConfig.value),
    ),
  };
}

function openCreateTargetDialog() {
  targetDialogMode.value = "create";
  resetTargetDialogState({
    provider: selectedProvider.value,
    enabled: true,
  });
  showTargetDialog.value = true;
}

function applyTargetDetailToDialog(detail: DDNSTargetDetailPayload) {
  targetDialogMode.value = "edit";
  resetTargetDialogState({
    id: detail.id,
    name: detail.rawName || "",
    enabled: detail.enabled,
    provider: detail.provider || "",
    config: detail.config,
  });
  showTargetDialog.value = true;
}

async function openEditTargetDialog(targetId: string) {
  try {
    const detail = await DDNSAPI.getTarget(targetId);
    applyTargetDetailToDialog(detail);
  } catch (error) {
    toast.error(t("admin.ddns.loadTargetFailed"), {
      description: extractErrorMessage(error, t("admin.ddns.loadTargetFailed")),
    });
  }
}

function updateTargetDialogNetworkInterface(value: string) {
  targetDialogState.value.config = {
    ...targetDialogState.value.config,
    [NETWORK_INTERFACE_KEY]: value,
    [INTERFACE_IPV4_INDEX_KEY]: "",
    [INTERFACE_IPV6_INDEX_KEY]: "",
  };
}

function handleTargetDialogProviderChange(value: string) {
  targetFieldVisibility.value = {};
  targetDialogState.value.provider = value;
  targetDialogState.value.config = normalizeTargetConfigValues(
    extractCommonTargetConfig(targetDialogState.value.config),
  );
}

function validateTargetDialogConfig() {
  const provider = targetDialogState.value.provider.trim();
  if (!provider) {
    toast.error(t("admin.ddns.selectProvider"));
    return false;
  }

  const providerDef = targetDialogProviderDef.value;
  if (providerDef) {
    const missingField = providerDef.fields.find((field) => {
      if (field.required === false) {
        return false;
      }
      return !targetDialogState.value.config[field.key]?.toString().trim();
    });
    if (missingField) {
      toast.error(
        t("admin.ddns.fillField", {
          label: missingField.label,
        }),
      );
      return false;
    }
  }

  const config = targetDialogState.value.config;
  if (
    isSingleAddressProvider(provider) &&
    targetDialogUpdateScope.value === "dual_stack"
  ) {
    toast.error(t("admin.ddns.singleAddressProviderRequiresSingleStack"));
    return false;
  }

  const ipSource = normalizeIpSource(config[IP_SOURCE_KEY]);
  if (ipSource === "static") {
    const ipv4 = normalizeStaticIPAddress(config[STATIC_IPV4_KEY]);
    const ipv6 = normalizeStaticIPAddress(config[STATIC_IPV6_KEY]);
    const needsIPv4 = targetDialogUpdateScope.value !== "ipv6_only";
    const needsIPv6 = targetDialogUpdateScope.value !== "ipv4_only";

    if (needsIPv4 && ipv4 && !isLikelyIPv4Address(ipv4)) {
      toast.error(t("admin.ddns.invalidStaticIpv4"));
      return false;
    }
    if (needsIPv6 && ipv6 && !isLikelyIPv6Address(ipv6)) {
      toast.error(t("admin.ddns.invalidStaticIpv6"));
      return false;
    }
    if (targetDialogUpdateScope.value === "ipv4_only" && !ipv4) {
      toast.error(t("admin.ddns.enterStaticIpv4"));
      return false;
    }
    if (targetDialogUpdateScope.value === "ipv6_only" && !ipv6) {
      toast.error(t("admin.ddns.enterStaticIpv6"));
      return false;
    }
    if (targetDialogUpdateScope.value === "dual_stack" && !ipv4 && !ipv6) {
      toast.error(t("admin.ddns.enterStaticIp"));
      return false;
    }
    return true;
  }

  if (ipSource === "domain") {
    const domain = normalizeSourceDomain(config[SOURCE_DOMAIN_KEY]);
    if (!domain) {
      toast.error(t("admin.ddns.enterSourceDomain"));
      return false;
    }
    if (!isLikelySourceDomain(domain)) {
      toast.error(t("admin.ddns.invalidSourceDomain"));
      return false;
    }
    return true;
  }

  if (ipSource !== "interface") {
    return true;
  }

  const networkInterface = normalizeNetworkInterface(
    config[NETWORK_INTERFACE_KEY],
  );
  if (!networkInterface) {
    toast.error(t("admin.ddns.chooseInterface"), {
      description: t("admin.ddns.chooseInterfaceDescription"),
    });
    return false;
  }

  if (
    targetDialogUpdateScope.value === "ipv4_only" &&
    targetDialogIPv4Options.value.length === 0
  ) {
    toast.error(t("admin.ddns.noIpv4Available"));
    return false;
  }

  if (
    targetDialogUpdateScope.value === "ipv6_only" &&
    targetDialogIPv6Options.value.length === 0
  ) {
    toast.error(t("admin.ddns.noIpv6Available"));
    return false;
  }

  if (
    targetDialogUpdateScope.value === "dual_stack" &&
    targetDialogIPv4Options.value.length === 0 &&
    targetDialogIPv6Options.value.length === 0
  ) {
    toast.error(t("admin.ddns.noAddressAvailable"));
    return false;
  }

  if (
    targetDialogUpdateScope.value !== "ipv6_only" &&
    targetDialogIPv4Options.value.length > 0 &&
    !normalizeInterfaceAddressIndex(config[INTERFACE_IPV4_INDEX_KEY])
  ) {
    toast.error(t("admin.ddns.chooseIpv4"));
    return false;
  }

  if (
    targetDialogUpdateScope.value !== "ipv4_only" &&
    targetDialogIPv6Options.value.length > 0 &&
    !normalizeInterfaceAddressIndex(config[INTERFACE_IPV6_INDEX_KEY])
  ) {
    toast.error(t("admin.ddns.chooseIpv6"));
    return false;
  }

  return true;
}

async function saveTargetDialog() {
  if (!validateTargetDialogConfig()) {
    return;
  }

  const payload = {
    name: targetDialogState.value.name.trim() || undefined,
    provider: targetDialogState.value.provider,
    enabled: targetDialogState.value.enabled,
    config: { ...targetDialogState.value.config },
  };

  await runSaveTarget(
    async () => {
      if (targetDialogMode.value === "edit" && targetDialogState.value.id) {
        await DDNSAPI.updateTarget(targetDialogState.value.id, payload);
        return;
      }

      await DDNSAPI.createTarget(payload);
    },
    {
      onSuccess: async () => {
        showTargetDialog.value = false;
        toast.success(
          targetDialogMode.value === "create"
            ? t("admin.ddns.targetCreated")
            : t("admin.ddns.targetUpdated"),
        );
        await loadStatus();
        ddnsPolling.resetCursor();
        void ddnsPolling.refresh();
      },
    },
  );
}

async function onTestExtraTarget(target: DDNSTargetSummaryPayload) {
  testingTargetId.value = target.id;
  await runTestTarget(
    async () => {
      const result = await DDNSAPI.testTarget(target.id);
      if (result.success) {
        toast.success(t("admin.ddns.targetUpdateSuccess"));
      } else {
        toast.error(t("admin.ddns.testTargetFailed"), {
          description: result.message,
        });
      }
    },
    {
      onFinally: async () => {
        testingTargetId.value = "";
        await loadStatus();
      },
    },
  );
}

async function onToggleExtraTarget(
  target: DDNSTargetSummaryPayload,
  enabled: boolean,
) {
  togglingTargetId.value = target.id;
  await runToggleTarget(
    async () => {
      await DDNSAPI.setTargetEnabled(target.id, enabled);
    },
    {
      onSuccess: async () => {
        toast.success(
          enabled
            ? t("admin.ddns.targetEnabled")
            : t("admin.ddns.targetDisabled"),
        );
        await loadStatus();
      },
      onFinally: () => {
        togglingTargetId.value = "";
      },
    },
  );
}

async function onDeleteExtraTarget(target: DDNSTargetSummaryPayload) {
  deletingTargetId.value = target.id;
  await runDeleteTarget(
    async () => {
      await DDNSAPI.deleteTarget(target.id);
    },
    {
      onSuccess: async () => {
        toast.success(t("admin.ddns.targetDeleted"));
        await loadStatus();
      },
      onFinally: () => {
        deletingTargetId.value = "";
      },
    },
  );
}

async function onSaveConfigSilent() {
  if (!selectedProvider.value) return false;
  if (!validateCommonConfig()) return false;
  await runSaveConfig(() =>
    DDNSAPI.saveConfig(selectedProvider.value, providerConfig.value),
  );
  savedProviderConfig.value = { ...providerConfig.value };
  return true;
}

function openClearPrimaryConfigDialog(collapse: () => void) {
  pendingPrimaryConfigCollapse.value = collapse;
  showClearPrimaryConfigDialog.value = true;
}

async function confirmClearPrimaryConfig() {
  if (!selectedProvider.value) return;

  await runClearPrimaryConfig(
    async () => {
      await DDNSAPI.saveConfig(selectedProvider.value, {});
    },
    {
      onSuccess: async () => {
        providerConfig.value = {};
        savedProviderConfig.value = {};
        fieldEditReady.value = {};
        showClearPrimaryConfigDialog.value = false;
        pendingPrimaryConfigCollapse.value?.();
        pendingPrimaryConfigCollapse.value = null;
        await loadStatus();
        await loadConfig();
        ddnsPolling.resetCursor();
        void ddnsPolling.refresh();
        toast.success(t("admin.ddns.primaryConfigCleared"));
      },
    },
  );
}

function applyCredentialTransfer() {
  const result = applyTransferredCredentials();
  if (!result) return;

  for (const key of result.appliedKeys) {
    enableFieldEditing(key);
  }

  toast.success(
    t("admin.ddns.credentialsApplied", {
      scope: transferSourceScopeLabel.value,
      count: result.count,
    }),
  );
}

async function onTest() {
  await runTestUpdate(async () => {
    const saved = await onSaveConfigSilent();
    if (!saved) {
      return;
    }
    const result = await DDNSAPI.test();
    if (result.success) {
      toast.success(t("admin.ddns.updateSuccess"));
      await loadStatus();
      return;
    }
    toast.error(t("admin.ddns.updateFailed"), { description: result.message });
  });
}

async function onClearLogs() {
  await runClearLogs(() => DDNSAPI.clearLogs(), {
    onSuccess: () => {
      logs.value = [];
      ddnsPolling.resetCursor();
      void ddnsPolling.refresh();
      toast.success(t("admin.ddns.logsCleared"));
    },
  });
}

function formatTime(iso: string | null): string {
  return formatDateTimeSafe(iso, {
    locale: String(locale.value),
    emptyText: t("admin.ddns.never"),
  });
}

const lastCheckTooltipLines = computed(() =>
  buildDDNSTimestampTooltipLines({
    updatedAt: lastIP.value.updated_at,
    checkedAt: lastCheck.value.checked_at,
    locale: String(locale.value),
    labels: {
      lastSuccessfulUpdate: t("admin.ddns.lastSuccessfulUpdate"),
      lastCheck: t("admin.ddns.lastCheck"),
      never: t("admin.ddns.never"),
    },
  }),
);

async function copyTextToClipboard(text: string) {
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  if (typeof document === "undefined") {
    throw new Error("Clipboard API unavailable");
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.top = "0";
  textarea.style.left = "0";
  textarea.style.opacity = "0";

  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();
  textarea.setSelectionRange(0, textarea.value.length);

  const copied = document.execCommand("copy");
  document.body.removeChild(textarea);

  if (!copied) {
    throw new Error("execCommand copy failed");
  }
}

async function copyIpAddress(
  versionLabel: "IPv4" | "IPv6",
  value: string | null,
) {
  const address = value?.trim();
  if (!address) {
    toast.error(
      t("admin.ddns.copyUnavailable", {
        version: versionLabel,
      }),
    );
    return;
  }

  try {
    await copyTextToClipboard(address);
    toast.success(
      t("admin.ddns.copySuccess", {
        version: versionLabel,
      }),
      { description: address },
    );
  } catch (error) {
    console.error("copyIpAddress:", error);
    toast.error(
      t("admin.ddns.copyFailed", {
        version: versionLabel,
      }),
      {
        description: t("admin.ddns.copyFailedDescription"),
      },
    );
  }
}

const logLines = computed(() =>
  logs.value.map((e) => {
    const tag =
      e.level === "error"
        ? t("admin.ddns.logLevelError")
        : e.level === "warn"
          ? t("admin.ddns.logLevelWarn")
          : t("admin.ddns.logLevelInfo");
    return `${tag} ${formatTime(e.time)}  ${e.message}`;
  }),
);

onMounted(async () => {
  const initialized = await runInitialize(async () => {
    await Promise.all([loadProviders(), loadStatus(), loadNetworkInterfaces()]);
    enabledInitialized = true;
    await loadConfig();
    return true;
  });
  isInitialized.value = true;
  if (initialized) {
    ddnsPolling.start();
  }
});
onUnmounted(() => {
  ddnsPolling.stop();
});
</script>

<template>
  <div v-if="isInitialized && !isLoading" class="space-y-3">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-2">
        <h2 class="text-xl font-semibold">{{ t("admin.ddns.title") }}</h2>
        <DocsLinkButton :href="docsUrls.guides.ddns" />
      </div>
      <div class="flex items-center gap-3">
        <span class="text-sm text-muted-foreground">{{
          enabled ? t("admin.ddns.enabled") : t("admin.ddns.disabled")
        }}</span>
        <Switch v-model="enabled" :disabled="isEnabledSwitchDisabled" />
      </div>
    </div>

    <Card class="overflow-hidden py-5 mb-6">
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle class="text-base font-medium flex items-center gap-2">
            {{ t("admin.ddns.statusTitle") }}
            <LiveStatusBadge
              :active="enabled"
              :active-label="t('admin.ddns.activeLabel')"
              :inactive-label="t('admin.ddns.pausedLabel')"
              class="mt-px"
            />
          </CardTitle>

          <button
            v-if="enabled"
            type="button"
            class="inline-flex items-center gap-1.5 rounded-md bg-muted/50 px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
            :aria-label="
              t('admin.ddns.setIntervalAria', { label: updateIntervalLabel })
            "
            @click="openUpdateIntervalDialog"
          >
            <Clock class="h-3.5 w-3.5" />
            <span>{{ updateIntervalLabel }}</span>
          </button>
        </div>
      </CardHeader>

      <CardContent>
        <div
          class="grid gap-4 xl:grid-cols-[minmax(10rem,1fr)_auto] xl:items-center xl:gap-6"
        >
          <div
            class="flex min-w-0 flex-col gap-4 md:min-w-[min(100%,10rem)] md:flex-row md:items-center md:gap-6"
          >
            <div
              v-if="showIPv4Status"
              class="flex min-w-0 items-center gap-4 md:shrink-0"
            >
              <div class="p-2.5 rounded-xl">
                <Wifi class="h-5 w-5" />
              </div>
              <div class="space-y-1">
                <p
                  class="text-[10px] uppercase tracking-wider font-semibold text-muted-foreground"
                >
                  {{ t("admin.ddns.ipv4Address") }}
                </p>
                <button
                  type="button"
                  class="block text-left text-sm font-mono font-medium transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 rounded-sm disabled:pointer-events-none disabled:text-foreground"
                  :disabled="!lastIP.ipv4"
                  @click="copyIpAddress('IPv4', lastIP.ipv4)"
                >
                  {{ lastIP.ipv4 || "---.---.---.---" }}
                </button>
              </div>
            </div>

            <div
              v-if="showIPv6Status"
              class="flex min-w-0 flex-1 items-center gap-4"
              :class="showIPv4Status ? 'md:border-l md:pl-6' : ''"
            >
              <div class="p-2.5 rounded-xl shrink-0">
                <Globe class="h-5 w-5" />
              </div>
              <div class="min-w-0 flex-1 space-y-1 overflow-hidden">
                <p
                  class="text-[10px] uppercase tracking-wider font-semibold text-muted-foreground"
                >
                  {{ t("admin.ddns.ipv6Address") }}
                </p>
                <button
                  type="button"
                  class="block w-full min-w-0 rounded-sm text-left transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:text-foreground"
                  :disabled="!lastIP.ipv6"
                  @click="copyIpAddress('IPv6', lastIP.ipv6)"
                >
                  <OverflowTooltipText
                    as="span"
                    :text="lastIP.ipv6 || t('admin.ddns.addressNotDetected')"
                    class="text-sm font-mono font-medium"
                  />
                </button>
              </div>
            </div>
          </div>

          <div
            class="flex min-w-0 flex-wrap items-center gap-4 xl:ml-auto xl:min-w-max xl:flex-nowrap xl:border-l xl:pl-6 2xl:gap-5"
          >
            <div
              class="flex min-w-[7.5rem] flex-[1_1_7.5rem] items-center gap-4 lg:flex-none"
            >
              <div class="p-2.5 rounded-xl">
                <Clock class="h-5 w-5" />
              </div>
              <div class="space-y-1">
                <p
                  class="whitespace-nowrap text-[10px] uppercase tracking-wider font-semibold text-muted-foreground"
                >
                  {{ t("admin.ddns.lastCheck") }}
                </p>
                <p class="text-sm font-medium">
                  <HumanFriendlyTime
                    :value="lastCheck.checked_at"
                    :empty-text="t('admin.ddns.never')"
                    :tooltip-lines="lastCheckTooltipLines"
                  />
                </p>
              </div>
            </div>

            <div
              class="flex min-w-[8.5rem] flex-[1_1_8.5rem] items-center gap-4 lg:flex-none"
            >
              <div class="p-2.5 rounded-xl">
                <RouteIcon class="h-5 w-5" />
              </div>
              <div class="space-y-1">
                <p
                  class="whitespace-nowrap text-[10px] uppercase tracking-wider font-semibold text-muted-foreground"
                >
                  {{ t("admin.ddns.updateScopeLabel") }}
                </p>
                <p class="whitespace-nowrap text-sm font-medium">
                  {{ currentUpdateScopeLabel }}
                </p>
              </div>
            </div>

            <div
              class="flex min-w-[8.5rem] flex-[1_1_8.5rem] items-center gap-4 lg:flex-none"
            >
              <div class="p-2.5 rounded-xl">
                <Network class="h-5 w-5" />
              </div>
              <div class="space-y-1">
                <p
                  class="whitespace-nowrap text-[10px] uppercase tracking-wider font-semibold text-muted-foreground"
                >
                  {{ t("admin.ddns.ipSourceLabel") }}
                </p>
                <p class="whitespace-nowrap text-sm font-medium">
                  {{ currentIpSourceLabel }}
                </p>
              </div>
            </div>

            <div
              class="flex min-w-[9rem] flex-[1_1_10rem] items-center gap-4 lg:flex-none"
            >
              <div class="p-2.5 rounded-xl">
                <Cable class="h-5 w-5" />
              </div>
              <div class="min-w-0 max-w-[180px] space-y-1">
                <p
                  class="whitespace-nowrap text-[10px] uppercase tracking-wider font-semibold text-muted-foreground"
                >
                  {{ t("admin.ddns.outboundInterface") }}
                </p>
                <OverflowTooltipText
                  as="p"
                  :text="currentNetworkInterfaceLabel"
                  class="text-sm font-medium"
                />
              </div>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>

    <ConfigCollapsibleCard
      :title="t('admin.ddns.mainConfigTitle')"
      :configured="hasProviderConfig"
      :ready="!isLoading"
      expanded-content-class="p-0 sm:p-0"
    >
      <template #summary>
        {{
          t("admin.ddns.currentProvider", {
            provider:
              currentProviderDef?.label || t("admin.ddns.notConfigured"),
          })
        }}
      </template>

      <template v-if="hasSavedProviderConfig" #collapsed-actions>
        <Button
          variant="outline"
          :disabled="isTesting || isSaving || !selectedProvider"
          @click="onTest"
        >
          <RefreshCw v-if="isTesting" class="w-4 h-4 mr-2 animate-spin" />
          {{
            isTesting ? t("admin.ddns.updating") : t("admin.ddns.refreshNow")
          }}
        </Button>
      </template>

      <template #default>
        <div class="divide-y divide-border">
          <div
            class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start"
          >
            <div class="space-y-1 mt-1.5">
              <Label class="text-sm font-medium">
                {{ t("admin.ddns.providerLabel") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.providerHint") }}
              </p>
            </div>
            <div class="w-full max-w-md">
              <Select
                :modelValue="selectedProvider"
                :disabled="isProviderSelectDisabled"
                @update:modelValue="
                  (val: any) => onProviderChange(String(val ?? ''))
                "
              >
                <SelectTrigger class="w-full" id="ddns-provider">
                  <SelectValue :placeholder="t('admin.ddns.selectProvider')" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="p in providers"
                    :key="p.name"
                    :value="p.name"
                  >
                    {{ p.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <div
            v-if="selectedProvider"
            class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-network-interface" class="text-sm font-medium">{{
                t("admin.ddns.outboundInterface")
              }}</Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.interfaceHint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Select
                :modelValue="
                  toNetworkInterfaceSelectValue(
                    providerConfig[NETWORK_INTERFACE_KEY],
                  )
                "
                @update:modelValue="
                  (val: any) =>
                    updateConfiguredNetworkInterface(
                      val === NETWORK_INTERFACE_AUTO_VALUE
                        ? ''
                        : String(val ?? ''),
                    )
                "
              >
                <SelectTrigger
                  class="w-full overflow-hidden"
                  id="ddns-network-interface"
                >
                  <SelectValue :placeholder="t('admin.ddns.autoSelect')">
                    <span class="block min-w-0 max-w-full truncate">
                      {{ configuredNetworkInterfaceLabel }}
                    </span>
                  </SelectValue>
                </SelectTrigger>
                <SelectContent
                  class="w-[var(--reka-select-trigger-width)] max-w-[min(32rem,calc(100vw-2rem))]"
                >
                  <SelectItem :value="NETWORK_INTERFACE_AUTO_VALUE">
                    {{ t("admin.ddns.autoSelect") }}
                  </SelectItem>
                  <SelectItem
                    v-for="networkInterface in resolvedNetworkInterfaces"
                    :key="networkInterface.name"
                    :value="networkInterface.name"
                  >
                    <div class="min-w-0 flex-1 pr-5">
                      <OverflowTooltipText
                        :text="networkInterface.label"
                        class="text-sm"
                        tooltip-align="start"
                        tooltip-side="right"
                      />
                    </div>
                  </SelectItem>
                </SelectContent>
              </Select>

              <p
                v-if="selectedNetworkInterfaceDetail"
                class="text-[11px] leading-5 text-muted-foreground break-all"
              >
                {{ selectedNetworkInterfaceDetail }}
              </p>

              <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                {{ t("admin.ddns.interfaceHint") }}
              </p>
            </div>
          </div>

          <div
            v-if="selectedProvider"
            class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-ip-source" class="text-sm font-medium">{{
                t("admin.ddns.ipSourceLabel")
              }}</Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.ipSourceHint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Select
                :modelValue="
                  providerConfig[IP_SOURCE_KEY] || DEFAULT_DDNS_IP_SOURCE
                "
                @update:modelValue="
                  (val: any) => updateConfiguredIpSource(String(val ?? ''))
                "
              >
                <SelectTrigger class="w-full" id="ddns-ip-source">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="option in IP_SOURCE_OPTIONS"
                    :key="option.value"
                    :value="option.value"
                    :disabled="
                      isIpSourceOptionDisabled(selectedProvider, option.value)
                    "
                  >
                    {{ formatOptionLabel(option) }}
                  </SelectItem>
                </SelectContent>
              </Select>

              <p class="text-[11px] text-muted-foreground">
                {{ t("admin.ddns.interfaceOnlyFiltered") }}
              </p>

              <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                {{ t("admin.ddns.ipSourceHint") }}
              </p>
            </div>
          </div>

          <div
            v-if="selectedProvider"
            class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-update-scope" class="text-sm font-medium">{{
                t("admin.ddns.updateScopeLabel")
              }}</Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.updateScopeHint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Select
                :modelValue="
                  providerConfig[UPDATE_SCOPE_KEY] || DEFAULT_DDNS_UPDATE_SCOPE
                "
                @update:modelValue="
                  (val: any) =>
                    (providerConfig[UPDATE_SCOPE_KEY] = normalizeUpdateScope(
                      String(val ?? ''),
                    ))
                "
              >
                <SelectTrigger class="w-full" id="ddns-update-scope">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="option in UPDATE_SCOPE_OPTIONS"
                    :key="option.value"
                    :value="option.value"
                    :disabled="
                      isUpdateScopeOptionDisabled(
                        selectedProvider,
                        option.value,
                      )
                    "
                  >
                    {{ formatOptionLabel(option) }}
                  </SelectItem>
                </SelectContent>
              </Select>

              <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                {{ t("admin.ddns.updateScopeHint") }}
              </p>
            </div>
          </div>

          <div
            v-if="showStaticIPv4Input"
            class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-static-ipv4" class="text-sm font-medium">
                {{ t("admin.ddns.staticIpv4Label") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.staticIpv4Hint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Input
                id="ddns-static-ipv4"
                v-model="providerConfig[STATIC_IPV4_KEY]"
                placeholder="203.0.113.10"
                inputmode="decimal"
                autocomplete="off"
              />
              <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                {{ t("admin.ddns.staticIpv4Hint") }}
              </p>
            </div>
          </div>

          <div
            v-if="showStaticIPv6Input"
            class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-static-ipv6" class="text-sm font-medium">
                {{ t("admin.ddns.staticIpv6Label") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.staticIpv6Hint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Input
                id="ddns-static-ipv6"
                v-model="providerConfig[STATIC_IPV6_KEY]"
                placeholder="2001:db8::10"
                autocomplete="off"
              />
              <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                {{ t("admin.ddns.staticIpv6Hint") }}
              </p>
            </div>
          </div>

          <div
            v-if="shouldShowSourceDomainBlock"
            class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-source-domain" class="text-sm font-medium">
                {{ t("admin.ddns.sourceDomainLabel") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.sourceDomainHint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Input
                id="ddns-source-domain"
                v-model="providerConfig[SOURCE_DOMAIN_KEY]"
                placeholder="origin.example.com"
                autocomplete="off"
              />
              <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                {{ t("admin.ddns.sourceDomainHint") }}
              </p>
            </div>
          </div>

          <div
            v-if="shouldShowInterfaceAddressBlock"
            class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label class="text-sm font-medium">
                {{ t("admin.ddns.interfaceAddressHelpTitle") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.interfaceAddressHelp") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <p
                v-if="!configuredNetworkInterface"
                class="text-sm text-muted-foreground"
              >
                {{ t("admin.ddns.chooseInterfaceFirst") }}
              </p>
              <template v-else>
                <p class="text-[11px] leading-5 text-muted-foreground">
                  {{ t("admin.ddns.addressOrderHelp") }}
                </p>
                <p class="text-[11px] leading-5 text-muted-foreground">
                  {{ t("admin.ddns.filteredAddressHelp") }}
                </p>
              </template>

              <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                {{ t("admin.ddns.interfaceAddressHelp") }}
              </p>
            </div>
          </div>

          <div
            v-if="showInterfaceIPv4Select"
            class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-interface-ipv4" class="text-sm font-medium">{{
                t("admin.ddns.selectIpv4Label")
              }}</Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.selectIpv4Hint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Select
                :modelValue="
                  normalizeInterfaceAddressIndex(
                    providerConfig[INTERFACE_IPV4_INDEX_KEY],
                  ) || undefined
                "
                :disabled="
                  !configuredNetworkInterface ||
                  interfaceIPv4Options.length === 0
                "
                @update:modelValue="
                  (val: any) =>
                    (providerConfig[INTERFACE_IPV4_INDEX_KEY] =
                      normalizeInterfaceAddressIndex(String(val ?? '')))
                "
              >
                <SelectTrigger class="w-full" id="ddns-interface-ipv4">
                  <SelectValue
                    :placeholder="t('admin.ddns.selectIpv4Placeholder')"
                  />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="option in interfaceIPv4Options"
                    :key="option.value"
                    :value="option.value"
                  >
                    {{ option.label }}
                  </SelectItem>
                  <div
                    v-if="interfaceIPv4Options.length === 0"
                    class="px-2 py-1.5 text-sm text-muted-foreground"
                  >
                    {{ t("admin.ddns.noIpv4Address") }}
                  </div>
                </SelectContent>
              </Select>

              <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                {{ t("admin.ddns.selectIpv4Hint") }}
              </p>
            </div>
          </div>

          <div
            v-if="showInterfaceIPv6Select"
            class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-interface-ipv6" class="text-sm font-medium">{{
                t("admin.ddns.selectIpv6Label")
              }}</Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.selectIpv6Hint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Select
                :modelValue="
                  normalizeInterfaceAddressIndex(
                    providerConfig[INTERFACE_IPV6_INDEX_KEY],
                  ) || undefined
                "
                :disabled="
                  !configuredNetworkInterface ||
                  interfaceIPv6Options.length === 0
                "
                @update:modelValue="
                  (val: any) =>
                    (providerConfig[INTERFACE_IPV6_INDEX_KEY] =
                      normalizeInterfaceAddressIndex(String(val ?? '')))
                "
              >
                <SelectTrigger class="w-full" id="ddns-interface-ipv6">
                  <SelectValue
                    :placeholder="t('admin.ddns.selectIpv6Placeholder')"
                  />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="option in interfaceIPv6Options"
                    :key="option.value"
                    :value="option.value"
                  >
                    {{ option.label }}
                  </SelectItem>
                  <div
                    v-if="interfaceIPv6Options.length === 0"
                    class="px-2 py-1.5 text-sm text-muted-foreground"
                  >
                    {{ t("admin.ddns.noIpv6Address") }}
                  </div>
                </SelectContent>
              </Select>

              <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                {{ t("admin.ddns.selectIpv6Hint") }}
              </p>
            </div>
          </div>

          <template v-if="currentProviderDef">
            <div
              v-if="credentialTransferSuggestion"
              class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
            >
              <div class="space-y-1 mt-1.5">
                <Label class="text-sm font-medium">
                  {{ t("admin.ddns.credentialReuse") }}
                </Label>
                <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                  {{ t("admin.ddns.credentialReuseHint") }}
                </p>
              </div>

              <div class="w-full max-w-2xl space-y-2">
                <CredentialTransferHint
                  :action-label="
                    t('admin.ddns.credentialFillAction', {
                      scope: transferSourceScopeLabel,
                    })
                  "
                  :description="credentialTransferDescription"
                  :fields="
                    credentialTransferSuggestion.fillableFields.map(
                      (field) => field.targetKey,
                    )
                  "
                  :loading="isTransferSourceLoading"
                  :source-label="`${transferSourceScopeLabel} · ${credentialTransferSuggestion.bridgeLabel}`"
                  @apply="applyCredentialTransfer"
                />

                <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                  {{ t("admin.ddns.credentialReuseHint") }}
                </p>
              </div>
            </div>

            <div
              v-for="(field, index) in currentProviderDef.fields"
              :key="field.key"
              class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
            >
              <div class="space-y-1 mt-1.5">
                <Label
                  :for="getFieldDomId(index)"
                  class="text-sm font-medium flex items-center gap-1"
                >
                  {{ field.label }}
                  <span v-if="field.required !== false" class="text-destructive"
                    >*</span
                  >
                </Label>
                <p
                  v-if="getFieldDescription(field)"
                  class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4"
                >
                  {{ getFieldDescription(field) }}
                </p>
              </div>

              <div class="w-full max-w-md space-y-2">
                <Select
                  v-if="field.type === 'select' && field.options"
                  :modelValue="
                    providerConfig[field.key] ||
                    (field.options && field.options[0]?.value) ||
                    ''
                  "
                  @update:modelValue="
                    (val: any) =>
                      (providerConfig[field.key] = String(val ?? ''))
                  "
                >
                  <SelectTrigger class="w-full" :id="getFieldDomId(index)">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem
                      v-for="opt in field.options"
                      :key="opt.value"
                      :value="opt.value"
                    >
                      {{ opt.label }}
                    </SelectItem>
                  </SelectContent>
                </Select>

                <div v-else-if="field.type === 'password'" class="relative">
                  <Input
                    :id="getFieldDomId(index)"
                    :name="getFieldInputName(index)"
                    :type="fieldVisibility[field.key] ? 'text' : 'password'"
                    :placeholder="field.placeholder"
                    :autocomplete="getFieldAutocomplete(field)"
                    :readonly="!isFieldEditReady(field.key)"
                    v-model="providerConfig[field.key]"
                    class="pr-10"
                    @focus="enableFieldEditing(field.key)"
                    @pointerdown="enableFieldEditing(field.key)"
                  />
                  <button
                    type="button"
                    class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                    @click="toggleFieldVisibility(field.key)"
                  >
                    <component
                      :is="fieldVisibility[field.key] ? EyeOff : Eye"
                      class="h-4 w-4"
                    />
                  </button>
                </div>

                <Input
                  v-else
                  :id="getFieldDomId(index)"
                  :name="getFieldInputName(index)"
                  :type="field.type"
                  :placeholder="field.placeholder"
                  :autocomplete="getFieldAutocomplete(field)"
                  :readonly="!isFieldEditReady(field.key)"
                  v-model="providerConfig[field.key]"
                  @focus="enableFieldEditing(field.key)"
                  @pointerdown="enableFieldEditing(field.key)"
                />

                <p
                  v-if="getFieldDescription(field)"
                  class="text-[11px] text-muted-foreground sm:hidden mt-1.5"
                >
                  {{ getFieldDescription(field) }}
                </p>
              </div>
            </div>
          </template>
        </div>
      </template>

      <template #actions="{ collapse }">
        <div
          class="p-4 sm:px-6 sm:py-4 bg-muted/30 border-t flex items-center justify-end gap-3 rounded-b-lg"
        >
          <Button variant="outline" @click="collapse">
            {{ t("admin.ddns.collapse") }}
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button
                variant="outline"
                class="w-24 gap-2"
                :disabled="
                  isClearingPrimaryConfig ||
                  !selectedProvider ||
                  !hasSavedProviderConfig
                "
              >
                <span>{{ t("admin.ddns.actions") }}</span>
                <ChevronDown class="h-4 w-4 text-muted-foreground" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" class="w-48">
              <DropdownMenuItem
                variant="destructive"
                :disabled="isClearingPrimaryConfig || !hasSavedProviderConfig"
                @click="openClearPrimaryConfigDialog(collapse)"
              >
                <Trash2 class="mr-2 h-4 w-4" />
                {{ t("admin.ddns.clearPrimaryConfig") }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <Button
            :disabled="isTesting || isSaving || !selectedProvider"
            @click="onTest"
            class="min-w-[100px] shadow-sm"
          >
            <RefreshCw v-if="isTesting" class="w-4 h-4 mr-2 animate-spin" />
            {{
              isTesting
                ? t("admin.ddns.updating")
                : t("admin.ddns.saveAndUpdate")
            }}
          </Button>
        </div>
      </template>
    </ConfigCollapsibleCard>

    <Card class="gap-2">
      <CardHeader>
        <div
          class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
        >
          <div class="space-y-1">
            <CardTitle class="text-base">
              {{ t("admin.ddns.extraDomainsTitle") }}
            </CardTitle>
            <p class="text-sm text-muted-foreground">
              {{ t("admin.ddns.extraDomainsDescription") }}
            </p>
          </div>
          <Button size="sm" @click="openCreateTargetDialog">
            <Plus class="mr-1.5 h-4 w-4" />
            {{ t("admin.ddns.addDomain") }}
          </Button>
        </div>
      </CardHeader>
      <CardContent class="space-y-3">
        <div
          v-if="!hasExtraTargets"
          class="rounded-xl border border-dashed px-4 py-6 text-sm text-muted-foreground"
        >
          {{ t("admin.ddns.extraDomainsEmpty") }}
        </div>

        <div v-else class="space-y-3">
          <div
            v-for="target in extraTargets"
            :key="target.id"
            class="rounded-xl border bg-card px-4 py-4"
          >
            <div
              class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between"
            >
              <div class="min-w-0 space-y-2">
                <div class="flex flex-wrap items-center gap-2">
                  <p class="text-sm font-medium">
                    {{ getTargetDisplayName(target) }}
                  </p>
                  <LiveStatusBadge
                    :active="target.enabled"
                    :active-label="t('admin.ddns.activeLabel')"
                    :inactive-label="t('admin.ddns.stoppedLabel')"
                  />
                </div>
                <p
                  class="text-sm text-muted-foreground break-all"
                  v-if="target.domainSummary"
                >
                  {{ target.domainSummary }}
                </p>
                <p class="text-xs text-muted-foreground">
                  {{ target.providerLabel }}
                </p>
                <p
                  v-if="target.lastCheck.message"
                  class="text-xs text-muted-foreground"
                >
                  {{ target.lastCheck.message }}
                </p>
              </div>

              <div class="grid gap-3 sm:grid-cols-3 lg:min-w-[360px]">
                <div
                  v-if="shouldShowTargetIPv4Status(target)"
                  class="rounded-lg px-3 py-3"
                >
                  <p
                    class="text-[10px] uppercase tracking-wider text-muted-foreground"
                  >
                    {{ t("admin.ddns.ipv4Address") }}
                  </p>
                  <button
                    type="button"
                    class="mt-1 block text-left text-sm font-mono font-medium transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 rounded-sm disabled:pointer-events-none disabled:text-foreground"
                    :disabled="!target.lastIP.ipv4"
                    @click="copyIpAddress('IPv4', target.lastIP.ipv4)"
                  >
                    {{ target.lastIP.ipv4 || "---.---.---.---" }}
                  </button>
                </div>
                <div
                  v-if="shouldShowTargetIPv6Status(target)"
                  class="rounded-lg px-3 py-3"
                >
                  <p
                    class="text-[10px] uppercase tracking-wider text-muted-foreground"
                  >
                    {{ t("admin.ddns.ipv6Address") }}
                  </p>
                  <button
                    type="button"
                    class="mt-1 block min-w-0 max-w-full rounded-sm text-left transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:text-foreground"
                    :disabled="!target.lastIP.ipv6"
                    @click="copyIpAddress('IPv6', target.lastIP.ipv6)"
                  >
                    <OverflowTooltipText
                      as="span"
                      :text="
                        target.lastIP.ipv6 || t('admin.ddns.addressNotDetected')
                      "
                      class="text-sm font-mono font-medium"
                    />
                  </button>
                </div>
                <div class="rounded-lg px-3 py-3">
                  <p
                    class="text-[10px] uppercase tracking-wider text-muted-foreground"
                  >
                    {{ t("admin.ddns.lastCheck") }}
                  </p>
                  <div class="mt-1 text-sm">
                    <HumanFriendlyTime
                      :value="target.lastCheck.checked_at"
                      :empty-text="t('admin.ddns.never')"
                      :tooltip-lines="getTargetLastCheckTooltipLines(target)"
                    />
                  </div>
                </div>
              </div>
            </div>

            <div class="mt-4 flex flex-wrap justify-end gap-2">
              <Button
                variant="outline"
                size="sm"
                :disabled="isSavingTarget"
                @click="openEditTargetDialog(target.id)"
              >
                {{ t("admin.ddns.edit") }}
              </Button>
              <Button
                variant="outline"
                size="sm"
                :disabled="testingTargetId === target.id"
                @click="onTestExtraTarget(target)"
              >
                <RefreshCw
                  v-if="testingTargetId === target.id"
                  class="mr-1.5 h-3.5 w-3.5 animate-spin"
                />
                {{
                  testingTargetId === target.id
                    ? t("admin.ddns.updating")
                    : t("admin.ddns.updateNow")
                }}
              </Button>
              <Button
                variant="outline"
                size="sm"
                :disabled="togglingTargetId === target.id"
                @click="onToggleExtraTarget(target, !target.enabled)"
              >
                {{
                  target.enabled ? t("admin.ddns.stop") : t("admin.ddns.start")
                }}
              </Button>
              <ConfirmDangerPopover
                :title="t('admin.ddns.deleteExtraTitle')"
                :description="
                  t('admin.ddns.deleteExtraDescription', {
                    name: getTargetDisplayName(target),
                  })
                "
                :loading="deletingTargetId === target.id"
                :disabled="deletingTargetId === target.id"
                :on-confirm="() => onDeleteExtraTarget(target)"
                content-class="w-72 text-left"
              >
                <template #trigger>
                  <Button
                    variant="outline"
                    size="sm"
                    :disabled="deletingTargetId === target.id"
                    class="text-destructive hover:text-destructive"
                  >
                    <Trash2 class="mr-1.5 h-3.5 w-3.5" />
                    {{
                      deletingTargetId === target.id
                        ? t("admin.ddns.deleting")
                        : t("admin.ddns.delete")
                    }}
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>

    <Card class="gap-2">
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle class="text-base">{{
            t("admin.ddns.logsTitle")
          }}</CardTitle>
          <div class="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              :disabled="isClearingLogs || logs.length === 0"
              @click="onClearLogs"
            >
              <Trash2 class="h-3.5 w-3.5 mr-1" />
              {{ t("admin.ddns.clear") }}
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <LogViewer
          :logs="logLines"
          reversed
          height-class="max-h-[400px]"
          :show-header="false"
          theme="light"
          wrap
        />
      </CardContent>
    </Card>

    <Dialog :open="showTargetDialog" @update:open="showTargetDialog = $event">
      <DialogContent class="sm:max-w-[760px] max-h-[88vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{{ targetDialogTitle }}</DialogTitle>
          <DialogDescription>{{ targetDialogDescription }}</DialogDescription>
        </DialogHeader>

        <div class="overflow-hidden rounded-lg border divide-y divide-border">
          <div
            class="p-4 sm:p-5 grid gap-3 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 sm:mt-1.5">
              <Label class="text-sm font-medium">
                {{ t("admin.ddns.targetEnabledLabel") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.targetEnabledHint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2 sm:justify-self-end">
              <div
                class="flex min-h-10 w-full items-center justify-start gap-3 sm:justify-end sm:px-3"
              >
                <Switch v-model="targetDialogState.enabled" />
                <span class="text-sm text-muted-foreground">
                  {{
                    targetDialogState.enabled
                      ? t("admin.ddns.activeLabel")
                      : t("admin.ddns.stoppedLabel")
                  }}
                </span>
              </div>
              <p class="text-[11px] text-muted-foreground sm:hidden">
                {{ t("admin.ddns.targetEnabledHint") }}
              </p>
            </div>
          </div>

          <div
            class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-target-name" class="text-sm font-medium">
                {{ t("admin.ddns.name") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.targetNameHint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Input
                id="ddns-target-name"
                v-model="targetDialogState.name"
                :placeholder="t('admin.ddns.targetNamePlaceholder')"
              />
              <p class="text-[11px] text-muted-foreground sm:hidden">
                {{ t("admin.ddns.targetNameHint") }}
              </p>
            </div>
          </div>

          <div
            class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
          >
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-target-provider" class="text-sm font-medium">
                {{ t("admin.ddns.providerLabel") }}
              </Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                {{ t("admin.ddns.targetProviderHint") }}
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Select
                :modelValue="targetDialogState.provider"
                @update:modelValue="
                  (val: any) =>
                    handleTargetDialogProviderChange(String(val ?? ''))
                "
              >
                <SelectTrigger id="ddns-target-provider">
                  <SelectValue :placeholder="t('admin.ddns.selectProvider')" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="provider in providers"
                    :key="provider.name"
                    :value="provider.name"
                  >
                    {{ provider.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
              <p class="text-[11px] text-muted-foreground sm:hidden">
                {{ t("admin.ddns.targetProviderHint") }}
              </p>
            </div>
          </div>

          <template v-if="targetDialogState.provider">
            <div
              v-if="
                targetDialogShouldShowStaticBlock &&
                targetDialogUpdateScope !== 'ipv6_only'
              "
              class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
            >
              <div class="space-y-1 mt-1.5">
                <Label
                  for="ddns-target-static-ipv4"
                  class="text-sm font-medium"
                >
                  {{ t("admin.ddns.staticIpv4Label") }}
                </Label>
                <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                  {{ t("admin.ddns.staticIpv4Hint") }}
                </p>
              </div>
              <div class="w-full max-w-md space-y-2">
                <Input
                  id="ddns-target-static-ipv4"
                  v-model="targetDialogState.config[STATIC_IPV4_KEY]"
                  placeholder="203.0.113.10"
                  inputmode="decimal"
                  autocomplete="off"
                />
                <p class="text-[11px] text-muted-foreground sm:hidden">
                  {{ t("admin.ddns.staticIpv4Hint") }}
                </p>
              </div>
            </div>

            <div
              v-if="
                targetDialogShouldShowStaticBlock &&
                targetDialogUpdateScope !== 'ipv4_only'
              "
              class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
            >
              <div class="space-y-1 mt-1.5">
                <Label
                  for="ddns-target-static-ipv6"
                  class="text-sm font-medium"
                >
                  {{ t("admin.ddns.staticIpv6Label") }}
                </Label>
                <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                  {{ t("admin.ddns.staticIpv6Hint") }}
                </p>
              </div>
              <div class="w-full max-w-md space-y-2">
                <Input
                  id="ddns-target-static-ipv6"
                  v-model="targetDialogState.config[STATIC_IPV6_KEY]"
                  placeholder="2001:db8::10"
                  autocomplete="off"
                />
                <p class="text-[11px] text-muted-foreground sm:hidden">
                  {{ t("admin.ddns.staticIpv6Hint") }}
                </p>
              </div>
            </div>

            <div
              v-if="targetDialogShouldShowDomainBlock"
              class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
            >
              <div class="space-y-1 mt-1.5">
                <Label
                  for="ddns-target-source-domain"
                  class="text-sm font-medium"
                >
                  {{ t("admin.ddns.sourceDomainLabel") }}
                </Label>
                <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                  {{ t("admin.ddns.sourceDomainHint") }}
                </p>
              </div>
              <div class="w-full max-w-md space-y-2">
                <Input
                  id="ddns-target-source-domain"
                  v-model="targetDialogState.config[SOURCE_DOMAIN_KEY]"
                  placeholder="origin.example.com"
                  autocomplete="off"
                />
                <p class="text-[11px] text-muted-foreground sm:hidden">
                  {{ t("admin.ddns.sourceDomainHint") }}
                </p>
              </div>
            </div>

            <div
              class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
            >
              <div class="space-y-1 mt-1.5">
                <Label
                  for="ddns-target-update-scope"
                  class="text-sm font-medium"
                >
                  {{ t("admin.ddns.updateScopeLabel") }}
                </Label>
                <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                  {{ t("admin.ddns.updateScopeHint") }}
                </p>
              </div>
              <div class="w-full max-w-md space-y-2">
                <Select
                  :modelValue="
                    targetDialogState.config[UPDATE_SCOPE_KEY] ||
                    DEFAULT_DDNS_UPDATE_SCOPE
                  "
                  @update:modelValue="
                    (val: any) =>
                      (targetDialogState.config[UPDATE_SCOPE_KEY] =
                        normalizeUpdateScope(String(val ?? '')))
                  "
                >
                  <SelectTrigger id="ddns-target-update-scope">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem
                      v-for="option in UPDATE_SCOPE_OPTIONS"
                      :key="option.value"
                      :value="option.value"
                      :disabled="
                        isUpdateScopeOptionDisabled(
                          targetDialogState.provider,
                          option.value,
                        )
                      "
                    >
                      {{ formatOptionLabel(option) }}
                    </SelectItem>
                  </SelectContent>
                </Select>
                <p class="text-[11px] text-muted-foreground sm:hidden">
                  {{ t("admin.ddns.updateScopeHint") }}
                </p>
              </div>
            </div>

            <div
              class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
            >
              <div class="space-y-1 mt-1.5">
                <Label for="ddns-target-ip-source" class="text-sm font-medium">
                  {{ t("admin.ddns.ipSourceLabel") }}
                </Label>
                <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                  {{ t("admin.ddns.ipSourceHint") }}
                </p>
              </div>
              <div class="w-full max-w-md space-y-2">
                <Select
                  :modelValue="
                    targetDialogState.config[IP_SOURCE_KEY] ||
                    DEFAULT_DDNS_IP_SOURCE
                  "
                  @update:modelValue="
                    (val: any) =>
                      (targetDialogState.config[IP_SOURCE_KEY] =
                        normalizeIpSource(String(val ?? '')))
                  "
                >
                  <SelectTrigger id="ddns-target-ip-source">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem
                      v-for="option in IP_SOURCE_OPTIONS"
                      :key="option.value"
                      :value="option.value"
                      :disabled="
                        isIpSourceOptionDisabled(
                          targetDialogState.provider,
                          option.value,
                        )
                      "
                    >
                      {{ formatOptionLabel(option) }}
                    </SelectItem>
                  </SelectContent>
                </Select>
                <p class="text-[11px] text-muted-foreground">
                  {{ t("admin.ddns.interfaceOnlyFiltered") }}
                </p>
              </div>
            </div>

            <div
              class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
            >
              <div class="space-y-1 mt-1.5">
                <Label
                  for="ddns-target-network-interface"
                  class="text-sm font-medium"
                >
                  {{ t("admin.ddns.outboundInterface") }}
                </Label>
                <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                  {{ t("admin.ddns.interfaceHint") }}
                </p>
              </div>
              <div class="w-full max-w-md space-y-2">
                <Select
                  :modelValue="
                    toNetworkInterfaceSelectValue(
                      targetDialogState.config[NETWORK_INTERFACE_KEY],
                    )
                  "
                  @update:modelValue="
                    (val: any) =>
                      updateTargetDialogNetworkInterface(
                        val === NETWORK_INTERFACE_AUTO_VALUE
                          ? ''
                          : String(val ?? ''),
                      )
                  "
                >
                  <SelectTrigger id="ddns-target-network-interface">
                    <SelectValue :placeholder="t('admin.ddns.autoSelect')">
                      <span class="block min-w-0 max-w-full truncate">
                        {{ targetDialogNetworkInterfaceLabel }}
                      </span>
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent
                    class="w-[var(--reka-select-trigger-width)] max-w-[min(32rem,calc(100vw-2rem))]"
                  >
                    <SelectItem :value="NETWORK_INTERFACE_AUTO_VALUE">
                      {{ t("admin.ddns.autoSelect") }}
                    </SelectItem>
                    <SelectItem
                      v-for="networkInterface in targetDialogResolvedNetworkInterfaces"
                      :key="networkInterface.name"
                      :value="networkInterface.name"
                    >
                      <div class="min-w-0 flex-1 pr-5">
                        <OverflowTooltipText
                          :text="networkInterface.label"
                          class="text-sm"
                          tooltip-align="start"
                          tooltip-side="right"
                        />
                      </div>
                    </SelectItem>
                  </SelectContent>
                </Select>
                <p class="text-[11px] text-muted-foreground sm:hidden">
                  {{ t("admin.ddns.interfaceHint") }}
                </p>
              </div>
            </div>

            <div
              v-if="targetDialogShouldShowInterfaceBlock"
              class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
            >
              <div class="space-y-1 mt-1.5">
                <Label class="text-sm font-medium">
                  {{ t("admin.ddns.interfaceAddressHelpTitle") }}
                </Label>
                <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                  {{ t("admin.ddns.interfaceAddressHelp") }}
                </p>
              </div>
              <div
                class="w-full max-w-md space-y-2 text-[11px] leading-5 text-muted-foreground"
              >
                <p>
                  {{ t("admin.ddns.addressOrderHelp") }}
                </p>
                <p>
                  {{ t("admin.ddns.filteredAddressHelp") }}
                </p>
              </div>
            </div>

            <div
              v-if="
                targetDialogUpdateScope !== 'ipv6_only' &&
                targetDialogShouldShowInterfaceBlock
              "
              class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
            >
              <div class="space-y-1 mt-1.5">
                <Label for="ddns-target-ipv4" class="text-sm font-medium">
                  {{ t("admin.ddns.selectIpv4Label") }}
                </Label>
                <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                  {{ t("admin.ddns.selectIpv4Hint") }}
                </p>
              </div>
              <div class="w-full max-w-md space-y-2">
                <Select
                  :modelValue="
                    normalizeInterfaceAddressIndex(
                      targetDialogState.config[INTERFACE_IPV4_INDEX_KEY],
                    ) || undefined
                  "
                  :disabled="targetDialogIPv4Options.length === 0"
                  @update:modelValue="
                    (val: any) =>
                      (targetDialogState.config[INTERFACE_IPV4_INDEX_KEY] =
                        normalizeInterfaceAddressIndex(String(val ?? '')))
                  "
                >
                  <SelectTrigger id="ddns-target-ipv4">
                    <SelectValue
                      :placeholder="t('admin.ddns.selectIpv4Placeholder')"
                    />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem
                      v-for="option in targetDialogIPv4Options"
                      :key="option.value"
                      :value="option.value"
                    >
                      {{ option.label }}
                    </SelectItem>
                  </SelectContent>
                </Select>
                <p class="text-[11px] text-muted-foreground sm:hidden">
                  {{ t("admin.ddns.selectIpv4Hint") }}
                </p>
              </div>
            </div>

            <div
              v-if="
                targetDialogUpdateScope !== 'ipv4_only' &&
                targetDialogShouldShowInterfaceBlock
              "
              class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
            >
              <div class="space-y-1 mt-1.5">
                <Label for="ddns-target-ipv6" class="text-sm font-medium">
                  {{ t("admin.ddns.selectIpv6Label") }}
                </Label>
                <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                  {{ t("admin.ddns.selectIpv6Hint") }}
                </p>
              </div>
              <div class="w-full max-w-md space-y-2">
                <Select
                  :modelValue="
                    normalizeInterfaceAddressIndex(
                      targetDialogState.config[INTERFACE_IPV6_INDEX_KEY],
                    ) || undefined
                  "
                  :disabled="targetDialogIPv6Options.length === 0"
                  @update:modelValue="
                    (val: any) =>
                      (targetDialogState.config[INTERFACE_IPV6_INDEX_KEY] =
                        normalizeInterfaceAddressIndex(String(val ?? '')))
                  "
                >
                  <SelectTrigger id="ddns-target-ipv6">
                    <SelectValue
                      :placeholder="t('admin.ddns.selectIpv6Placeholder')"
                    />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem
                      v-for="option in targetDialogIPv6Options"
                      :key="option.value"
                      :value="option.value"
                    >
                      {{ option.label }}
                    </SelectItem>
                  </SelectContent>
                </Select>
                <p class="text-[11px] text-muted-foreground sm:hidden">
                  {{ t("admin.ddns.selectIpv6Hint") }}
                </p>
              </div>
            </div>

            <template v-if="targetDialogProviderDef">
              <div
                v-for="field in targetDialogProviderDef.fields"
                :key="`target-${field.key}`"
                class="p-4 sm:p-5 grid gap-2 sm:grid-cols-[180px_1fr] md:grid-cols-[220px_1fr] items-start transition-colors hover:bg-muted/10"
              >
                <div class="space-y-1 mt-1.5">
                  <Label
                    :for="`ddns-target-field-${field.key}`"
                    class="text-sm font-medium flex items-center gap-1"
                  >
                    {{ field.label }}
                    <span
                      v-if="field.required !== false"
                      class="text-destructive"
                    >
                      *
                    </span>
                  </Label>
                  <p
                    v-if="getFieldDescription(field)"
                    class="text-xs text-muted-foreground hidden sm:block pr-4"
                  >
                    {{ getFieldDescription(field) }}
                  </p>
                </div>

                <div class="w-full max-w-md space-y-2">
                  <Select
                    v-if="field.type === 'select' && field.options"
                    :modelValue="
                      targetDialogState.config[field.key] ||
                      field.options[0]?.value ||
                      ''
                    "
                    @update:modelValue="
                      (val: any) =>
                        (targetDialogState.config[field.key] = String(
                          val ?? '',
                        ))
                    "
                  >
                    <SelectTrigger :id="`ddns-target-field-${field.key}`">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem
                        v-for="option in field.options"
                        :key="option.value"
                        :value="option.value"
                      >
                        {{ option.label }}
                      </SelectItem>
                    </SelectContent>
                  </Select>

                  <div v-else-if="field.type === 'password'" class="relative">
                    <Input
                      :id="`ddns-target-field-${field.key}`"
                      v-model="targetDialogState.config[field.key]"
                      :type="
                        isTargetFieldVisible(field.key) ? 'text' : 'password'
                      "
                      :placeholder="field.placeholder"
                      :autocomplete="getFieldAutocomplete(field)"
                      class="pr-10"
                    />
                    <button
                      type="button"
                      class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                      @click="toggleTargetFieldVisibility(field.key)"
                    >
                      <component
                        :is="isTargetFieldVisible(field.key) ? EyeOff : Eye"
                        class="h-4 w-4"
                      />
                    </button>
                  </div>

                  <Input
                    v-else
                    :id="`ddns-target-field-${field.key}`"
                    v-model="targetDialogState.config[field.key]"
                    :type="field.type"
                    :placeholder="field.placeholder"
                    :autocomplete="getFieldAutocomplete(field)"
                  />

                  <p
                    v-if="getFieldDescription(field)"
                    class="text-[11px] text-muted-foreground sm:hidden"
                  >
                    {{ getFieldDescription(field) }}
                  </p>
                </div>
              </div>
            </template>
          </template>
        </div>

        <DialogFooter class="gap-2">
          <Button
            variant="outline"
            :disabled="isSavingTarget"
            @click="showTargetDialog = false"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button :disabled="isSavingTarget" @click="saveTargetDialog">
            <RefreshCw
              v-if="isSavingTarget"
              class="mr-1.5 h-4 w-4 animate-spin"
            />
            {{ isSavingTarget ? t("admin.ddns.saving") : t("common.save") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog
      :open="showUpdateIntervalDialog"
      @update:open="showUpdateIntervalDialog = $event"
    >
      <DialogContent class="sm:max-w-[420px]">
        <DialogHeader>
          <DialogTitle>{{ t("admin.ddns.intervalDialogTitle") }}</DialogTitle>
          <DialogDescription>
            {{ t("admin.ddns.intervalDialogDescription") }}
          </DialogDescription>
        </DialogHeader>

        <div class="grid gap-2 py-2">
          <Label for="ddns-update-interval">
            {{ t("admin.ddns.intervalMinutes") }}
          </Label>
          <div class="relative">
            <Input
              id="ddns-update-interval"
              v-model="updateIntervalDraft"
              type="number"
              inputmode="numeric"
              :min="MIN_DDNS_UPDATE_INTERVAL_MINUTES"
              :max="MAX_DDNS_UPDATE_INTERVAL_MINUTES"
              step="1"
              class="pr-14"
              @keydown.enter.prevent="saveUpdateInterval"
            />
            <span
              class="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-sm text-muted-foreground"
            >
              {{ t("admin.ddns.minutes") }}
            </span>
          </div>
          <p class="text-xs text-muted-foreground">
            {{
              t("admin.ddns.intervalHelp", {
                min: MIN_DDNS_UPDATE_INTERVAL_MINUTES,
                max: MAX_DDNS_UPDATE_INTERVAL_MINUTES,
              })
            }}
          </p>
        </div>

        <DialogFooter class="gap-2">
          <Button
            variant="outline"
            :disabled="isSavingUpdateInterval"
            @click="showUpdateIntervalDialog = false"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            :disabled="isSavingUpdateInterval"
            @click="saveUpdateInterval"
          >
            <RefreshCw
              v-if="isSavingUpdateInterval"
              class="mr-1.5 h-4 w-4 animate-spin"
            />
            {{
              isSavingUpdateInterval ? t("admin.ddns.saving") : t("common.save")
            }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog
      :open="showClearPrimaryConfigDialog"
      @update:open="showClearPrimaryConfigDialog = $event"
    >
      <DialogContent class="sm:max-w-[420px]">
        <DialogHeader>
          <DialogTitle>{{ t("admin.ddns.clearPrimaryTitle") }}</DialogTitle>
          <DialogDescription>
            {{ t("admin.ddns.clearPrimaryDescription") }}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            variant="outline"
            :disabled="isClearingPrimaryConfig"
            @click="showClearPrimaryConfigDialog = false"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            variant="destructive"
            :disabled="isClearingPrimaryConfig"
            @click="confirmClearPrimaryConfig"
          >
            <RefreshCw
              v-if="isClearingPrimaryConfig"
              class="mr-2 h-4 w-4 animate-spin"
            />
            {{
              isClearingPrimaryConfig
                ? t("admin.ddns.clearing")
                : t("admin.ddns.confirmClear")
            }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>

  <div v-else class="flex h-full items-center justify-center min-h-[400px]">
    <div
      class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary"
    ></div>
  </div>
</template>
