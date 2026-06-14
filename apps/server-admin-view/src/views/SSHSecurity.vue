<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { parseCidrTextarea } from "@admin-shared/utils/cidr";
import { usePagedSelectionList } from "@admin-shared/composables/usePagedSelectionList";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  TagsInput,
  TagsInputItem,
  TagsInputItemDelete,
  TagsInputItemText,
} from "@/components/ui/tags-input";
import { Textarea } from "@/components/ui/textarea";
import RefreshButton from "@/components/RefreshButton.vue";
import {
  ChevronDown,
  Loader2,
  Plus,
  RefreshCw,
  Save,
  Trash2,
} from "lucide-vue-next";
import { CidrAPI, SSHSecurityAPI } from "../lib/api";
import { useConfigStore } from "../store/config";
import type {
  CidrCityOption,
  CidrProvinceOption,
  SSHLoginLogEntry,
  SSHSecurityBlockRecord,
  SSHSecurityDetails,
  SSHSecuritySelection,
} from "../types";

const configStore = useConfigStore();
const { t } = useI18n();
const details = ref<SSHSecurityDetails | null>(null);
const provinces = ref<CidrProvinceOption[]>([]);
const cityOptions = ref<CidrCityOption[]>([]);
const cityOptionsLoading = ref(false);
const activeTab = ref("login-logs");
const logItems = ref<SSHLoginLogEntry[]>([]);
const logTotal = ref(0);
const logPage = ref(1);
const logLimit = ref("20");
const logSearch = ref("");
const logOutcome = ref<"all" | "success" | "failure">("all");
const isRegionDialogOpen = ref(false);
const isClearFirewallDialogOpen = ref(false);

const regionDraft = reactive({
  province: "",
  cityValue: "",
});

const form = reactive({
  enabled: false,
  windowMinutes: 10,
  failedLoginThreshold: 5,
  blockDurationValue: 1,
  blockDurationUnit: "day" as "minute" | "hour" | "day",
  allowedRegions: [] as SSHSecuritySelection[],
  customCidrsText: "",
});

let cityRequestToken = 0;
let logSearchTimer: ReturnType<typeof setTimeout> | null = null;

const { isPending: isLoading, run: runLoad } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sshSecurity.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sshSecurity.loadDescription"),
      ),
    });
  },
});
const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sshSecurity.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sshSecurity.saveDescription"),
      ),
    });
  },
});
const { isPending: isSyncingFirewall, run: runSyncFirewall } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sshSecurity.syncFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sshSecurity.syncDescription"),
      ),
    });
  },
});
const { isPending: isLoadingLogs, run: runLoadLogs } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sshSecurity.logsLoadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sshSecurity.logsLoadDescription"),
      ),
    });
  },
});
const { isPending: isDeleting, run: runDelete } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sshSecurity.unblockFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sshSecurity.unblockDescription"),
      ),
    });
  },
});

const selectionKey = (selection: {
  province: string;
  query_city?: string | null;
}) => `${selection.province}::${selection.query_city ?? ""}`;

const customCidrsState = computed(() =>
  parseCidrTextarea(form.customCidrsText),
);
const invalidCustomCidrs = computed(() => customCidrsState.value.invalid);
const regionInputsDisabled = computed(() => isSaving.value || !form.enabled);
const selectedCityOption = computed(
  () =>
    cityOptions.value.find(
      (option) => option.value === regionDraft.cityValue,
    ) ?? null,
);
const citySelectKey = computed(() => regionDraft.province || "empty");
const citySelectPlaceholder = computed(() => {
  if (cityOptionsLoading.value) return t("admin.sshSecurity.loading");
  if (!regionDraft.province) return t("admin.sshSecurity.selectProvinceFirst");
  return cityOptions.value.some((option) => option.isProvinceWide)
    ? t("admin.sshSecurity.selectCityOrProvince")
    : t("admin.sshSecurity.selectCity");
});
const pendingRegionExists = computed(() => {
  const city = selectedCityOption.value;
  if (!regionDraft.province || !city) return false;
  return form.allowedRegions.some(
    (item) =>
      selectionKey(item) ===
      selectionKey({
        province: regionDraft.province,
        query_city: city.queryCity,
      }),
  );
});
const canAddRegion = computed(
  () =>
    form.enabled &&
    Boolean(regionDraft.province) &&
    Boolean(selectedCityOption.value) &&
    !pendingRegionExists.value &&
    !cityOptionsLoading.value,
);
const logParsedLimit = computed(() => {
  const parsed = Number.parseInt(logLimit.value, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 20;
});
const sshPortsLabel = computed(() => {
  const ports = details.value?.summary.ssh_ports ?? [22];
  return ports.length > 0
    ? ports.join(t("admin.sshSecurity.listSeparator"))
    : "22";
});

const summaryText = computed(() => {
  const summary = details.value?.summary;
  if (!summary) return t("admin.sshSecurity.notLoaded");
  const enabled = summary.enabled
    ? t("admin.sshSecurity.enabled")
    : t("admin.sshSecurity.disabled");
  return t("admin.sshSecurity.summary", {
    status: enabled,
    ports: sshPortsLabel.value,
    allowed: summary.allowed_cidr_count,
    blocks: summary.active_block_count,
  });
});

const saveBlockedReason = computed(() => {
  if (!details.value?.summary.available && form.enabled) {
    return (
      details.value?.summary.unavailable_reason ||
      t("admin.sshSecurity.unavailableToEnable")
    );
  }
  if (invalidCustomCidrs.value.length > 0) {
    return t("admin.sshSecurity.fixCustomCidrs");
  }
  return "";
});

const reasonLabel = (reason: SSHSecurityBlockRecord["reason"]) =>
  reason === "cidr_not_allowed"
    ? t("admin.sshSecurity.reasonRegionNotAllowed")
    : t("admin.sshSecurity.reasonThresholdReached");

const applyDetails = (value: SSHSecurityDetails) => {
  details.value = value;
  form.enabled = value.config.enabled;
  form.windowMinutes = value.config.window_minutes;
  form.failedLoginThreshold = value.config.failed_login_threshold;
  form.blockDurationValue = value.config.block_duration_value;
  form.blockDurationUnit = value.config.block_duration_unit;
  form.allowedRegions = value.config.allowed_regions.map((item) => ({
    ...item,
  }));
  form.customCidrsText = value.config.custom_cidrs.join("\n");
};

const clearRegionDraft = () => {
  cityRequestToken += 1;
  cityOptionsLoading.value = false;
  regionDraft.province = "";
  regionDraft.cityValue = "";
  cityOptions.value = [];
};

const prepareRegionDraft = () => {
  const preferredProvince =
    form.allowedRegions[0]?.province || provinces.value[0]?.value || "";

  clearRegionDraft();

  if (preferredProvince) {
    regionDraft.province = preferredProvince;
  }
};

const openRegionDialog = () => {
  if (regionInputsDisabled.value || provinces.value.length === 0) {
    return;
  }

  isRegionDialogOpen.value = true;
  prepareRegionDraft();
};

const handleRegionDialogOpenChange = (nextOpen: boolean) => {
  isRegionDialogOpen.value = nextOpen;

  if (!nextOpen) {
    clearRegionDraft();
  }
};

const loadCityOptions = async (province: string) => {
  if (!province) {
    cityOptions.value = [];
    regionDraft.cityValue = "";
    return;
  }

  const token = ++cityRequestToken;
  cityOptionsLoading.value = true;
  cityOptions.value = [];
  regionDraft.cityValue = "";
  try {
    const payload = await CidrAPI.getCities(province);
    if (token !== cityRequestToken) return;
    cityOptions.value = payload.options;
    const hasCurrentValue = payload.options.some(
      (option) => option.value === regionDraft.cityValue,
    );
    regionDraft.cityValue = hasCurrentValue
      ? regionDraft.cityValue
      : (payload.defaultValue ?? payload.options[0]?.value ?? "");
  } catch (error) {
    if (token !== cityRequestToken) return;
    cityOptions.value = [];
    regionDraft.cityValue = "";
    toast.error(t("admin.sshSecurity.regionsLoadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sshSecurity.regionsLoadDescription"),
      ),
    });
  } finally {
    if (token === cityRequestToken) {
      cityOptionsLoading.value = false;
    }
  }
};

watch(
  () => regionDraft.province,
  (province, previousProvince) => {
    if (province !== previousProvince) {
      void loadCityOptions(province);
    }
  },
);

watch(
  () => form.enabled,
  (enabled) => {
    if (!enabled) {
      handleRegionDialogOpenChange(false);
    }
  },
);

const addRegion = () => {
  const option = selectedCityOption.value;
  if (!option || !canAddRegion.value) return;
  form.allowedRegions.push({
    province: regionDraft.province,
    city: option.isProvinceWide ? null : option.label,
    label: option.label,
    value: option.value,
    query_city: option.queryCity,
    is_province_wide: option.isProvinceWide,
    is_municipality: option.isMunicipality,
  });
  handleRegionDialogOpenChange(false);
};

const removeRegion = (selection: SSHSecuritySelection) => {
  if (regionInputsDisabled.value) return;
  form.allowedRegions = form.allowedRegions.filter(
    (item) => selectionKey(item) !== selectionKey(selection),
  );
};

const loadDetails = async () => {
  await runLoad(async () => {
    const [provincePayload, nextDetails] = await Promise.all([
      CidrAPI.getProvinces(),
      SSHSecurityAPI.getDetails(),
    ]);
    provinces.value = provincePayload.options;
    applyDetails(nextDetails);
  });
};

const saveConfig = async () => {
  if (saveBlockedReason.value) {
    toast.error(t("admin.sshSecurity.cannotSave"), {
      description: saveBlockedReason.value,
    });
    return;
  }
  await runSave(
    () =>
      SSHSecurityAPI.updateConfig({
        enabled: form.enabled,
        window_minutes: form.windowMinutes,
        failed_login_threshold: form.failedLoginThreshold,
        block_duration_value: form.blockDurationValue,
        block_duration_unit: form.blockDurationUnit,
        allowed_regions: form.allowedRegions.map((item) => ({
          province: item.province,
          query_city: item.query_city,
        })),
        custom_cidrs: customCidrsState.value.cidrs,
      }),
    {
      onSuccess: async (nextDetails) => {
        applyDetails(nextDetails);
        toast.success(t("admin.sshSecurity.saved"));
        await configStore.loadConfig();
      },
    },
  );
};

const syncFirewall = async () => {
  if (!details.value?.summary.available) {
    toast.error(t("admin.sshSecurity.cannotSync"), {
      description:
        details.value?.summary.unavailable_reason ||
        t("admin.sshSecurity.unavailableToSync"),
    });
    return;
  }

  await runSyncFirewall(SSHSecurityAPI.syncFirewall, {
    onSuccess: async (result) => {
      toast.success(t("admin.sshSecurity.firewallSynced"), {
        description: t("admin.sshSecurity.firewallSyncedDescription", {
          allowed: result.allowed_cidrs,
          synced: result.synced,
          ports:
            result.ports.join(t("admin.sshSecurity.listSeparator")) || "22",
        }),
      });
      await Promise.all([loadDetails(), loadBlocks()]);
    },
  });
};

const openClearFirewallDialog = () => {
  if (!details.value?.summary.available || isSyncingFirewall.value) return;
  isClearFirewallDialogOpen.value = true;
};

const clearFirewall = async () => {
  if (!details.value?.summary.available) {
    toast.error(t("admin.sshSecurity.cannotClear"), {
      description:
        details.value?.summary.unavailable_reason ||
        t("admin.sshSecurity.unavailableToClear"),
    });
    return;
  }

  await runSyncFirewall(SSHSecurityAPI.clearFirewall, {
    onSuccess: async (result) => {
      isClearFirewallDialogOpen.value = false;
      toast.success(t("admin.sshSecurity.firewallCleared"), {
        description: t("admin.sshSecurity.firewallClearedDescription", {
          count: result.cleared_blocks,
        }),
      });
      await Promise.all([loadDetails(), loadBlocks()]);
    },
  });
};

const loadLogs = async () => {
  await runLoadLogs(
    () =>
      SSHSecurityAPI.getLoginLogs({
        page: logPage.value,
        limit: logLimit.value,
        search: logSearch.value,
        outcome: logOutcome.value,
      }),
    {
      onSuccess: (payload) => {
        logItems.value = payload.items;
        logTotal.value = payload.total;
      },
    },
  );
};

const handleLogSearch = () => {
  logPage.value = 1;
  void loadLogs();
};

const handleLogPageChange = (page: number) => {
  logPage.value = page;
  void loadLogs();
};

const handleLogLimitChange = (value: unknown) => {
  logLimit.value = String(value ?? "20");
  logPage.value = 1;
  void loadLogs();
};

watch(logSearch, () => {
  if (logSearchTimer) clearTimeout(logSearchTimer);
  logSearchTimer = setTimeout(handleLogSearch, 500);
});

watch(logOutcome, () => {
  handleLogSearch();
});

const {
  items: blockRecords,
  total: blockTotal,
  loading: isLoadingBlocks,
  searchQuery: blockSearch,
  currentPage: blockPage,
  limit: blockLimit,
  parsedLimit: blockParsedLimit,
  selectedKeys: selectedBlockIps,
  isAllSelected: isAllBlocksSelected,
  fetchList: loadBlocks,
  handleSearch: handleBlockSearch,
  handlePageChange: handleBlockPageChange,
  handleLimitChange: handleBlockLimitChange,
  toggleSelect: toggleBlockSelect,
  clearSelection: clearBlockSelection,
} = usePagedSelectionList<SSHSecurityBlockRecord, string>({
  fetchPage: async ({ page, limit, query }) => {
    const payload = await SSHSecurityAPI.getBlocks(page, limit, query);
    return { items: payload.items, total: payload.total };
  },
  getKey: (record) => record.ip,
  onError: (error) => {
    toast.error(t("admin.sshSecurity.blocksLoadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sshSecurity.blocksLoadDescription"),
      ),
    });
  },
});

const deleteBlocks = async (ips: string[]) => {
  if (ips.length === 0) return;
  await runDelete(() => SSHSecurityAPI.deleteBlocks(ips), {
    onSuccess: async () => {
      toast.success(t("admin.sshSecurity.unblocked"));
      clearBlockSelection();
      await Promise.all([loadBlocks(), loadDetails()]);
    },
  });
};

const deleteOneBlock = async (ip: string) => {
  await runDelete(() => SSHSecurityAPI.deleteBlock(ip), {
    onSuccess: async () => {
      toast.success(t("admin.sshSecurity.unblocked"));
      selectedBlockIps.value.delete(ip);
      selectedBlockIps.value = new Set(selectedBlockIps.value);
      await Promise.all([loadBlocks(), loadDetails()]);
    },
  });
};

onMounted(async () => {
  await loadDetails();
  await Promise.all([loadLogs(), loadBlocks()]);
});
</script>

<template>
  <div class="space-y-4">
    <ConfigCollapsibleCard
      :title="t('admin.sshSecurity.title')"
      :configured="details?.summary.configured === true"
      :ready="details !== null && !isLoading"
      :edit-label="t('admin.sshSecurity.editConfig')"
      summary-class="text-xs text-muted-foreground"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col-reverse items-stretch gap-2 rounded-b-lg sm:flex-row sm:items-center sm:justify-end"
    >
      <template #summary>
        {{ summaryText }}
      </template>

      <template #collapsed-actions>
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button
              variant="outline"
              class="w-24 gap-2"
              :disabled="
                isSaving ||
                isSyncingFirewall ||
                !details ||
                !details.summary.available
              "
            >
              <Loader2 v-if="isSyncingFirewall" class="h-4 w-4 animate-spin" />
              <span>{{ t("admin.sshSecurity.actions") }}</span>
              <ChevronDown class="h-4 w-4 text-muted-foreground" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-56">
            <DropdownMenuItem
              :disabled="
                isSaving ||
                isSyncingFirewall ||
                !details ||
                !details.summary.available
              "
              @select="syncFirewall"
            >
              <RefreshCw class="h-4 w-4" />
              {{ t("admin.sshSecurity.syncFirewall") }}
            </DropdownMenuItem>
            <DropdownMenuItem
              class="text-destructive focus:text-destructive"
              :disabled="
                isSaving ||
                isSyncingFirewall ||
                !details ||
                !details.summary.available
              "
              @select="openClearFirewallDialog"
            >
              <Trash2 class="h-4 w-4" />
              {{ t("admin.sshSecurity.clearSshFirewall") }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </template>

      <template #default>
        <div class="divide-y divide-border">
          <div v-if="details && !details.summary.available" class="p-4 sm:p-6">
            <div
              class="rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-900"
            >
              {{ details.summary.unavailable_reason }}
            </div>
          </div>

          <div
            class="grid gap-3 p-4 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
          >
            <div class="space-y-1">
              <Label class="text-sm font-medium">
                {{ t("admin.sshSecurity.enableSshSecurity") }}
              </Label>
              <p
                class="hidden pr-4 text-xs leading-5 text-muted-foreground sm:block"
              >
                {{ t("admin.sshSecurity.enableDescription") }}
              </p>
            </div>
            <div class="flex items-start justify-between gap-4">
              <p class="text-sm leading-6 text-muted-foreground sm:hidden">
                {{ t("admin.sshSecurity.enableDescription") }}
              </p>
              <Switch
                v-model="form.enabled"
                class="mt-0.5 shrink-0"
                :disabled="
                  isSaving || (details !== null && !details.summary.available)
                "
              />
            </div>
          </div>

          <div
            class="grid gap-3 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
          >
            <div class="space-y-1">
              <Label for="ssh-window-minutes" class="text-sm font-medium">
                {{ t("admin.sshSecurity.windowTime") }}
              </Label>
              <p
                class="hidden pr-4 text-xs leading-5 text-muted-foreground sm:block"
              >
                {{ t("admin.sshSecurity.windowDescription") }}
              </p>
            </div>
            <div class="w-full max-w-xs space-y-2">
              <Input
                id="ssh-window-minutes"
                v-model.number="form.windowMinutes"
                type="number"
                min="1"
                max="1440"
                :disabled="isSaving"
              />
              <p class="text-[11px] text-muted-foreground">
                {{ t("admin.sshSecurity.unitMinutes") }}
              </p>
            </div>
          </div>

          <div
            class="grid gap-3 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
          >
            <div class="space-y-1">
              <Label for="ssh-failure-threshold" class="text-sm font-medium">
                {{ t("admin.sshSecurity.failureThreshold") }}
              </Label>
              <p
                class="hidden pr-4 text-xs leading-5 text-muted-foreground sm:block"
              >
                {{ t("admin.sshSecurity.failureThresholdDescription") }}
              </p>
            </div>
            <div class="w-full max-w-xs">
              <Input
                id="ssh-failure-threshold"
                v-model.number="form.failedLoginThreshold"
                type="number"
                min="1"
                max="1000"
                :disabled="isSaving"
              />
            </div>
          </div>

          <div
            class="grid gap-3 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
          >
            <div class="space-y-1">
              <Label for="ssh-block-duration" class="text-sm font-medium">
                {{ t("admin.sshSecurity.blockDuration") }}
              </Label>
              <p
                class="hidden pr-4 text-xs leading-5 text-muted-foreground sm:block"
              >
                {{ t("admin.sshSecurity.blockDurationDescription") }}
              </p>
            </div>
            <div
              class="grid w-full max-w-md grid-cols-[minmax(0,1fr)_140px] gap-2"
            >
              <Input
                id="ssh-block-duration"
                v-model.number="form.blockDurationValue"
                type="number"
                min="1"
                max="365"
                :disabled="isSaving"
              />
              <Select v-model="form.blockDurationUnit">
                <SelectTrigger :disabled="isSaving">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="minute">
                    {{ t("admin.sshSecurity.minute") }}
                  </SelectItem>
                  <SelectItem value="hour">
                    {{ t("admin.sshSecurity.hour") }}
                  </SelectItem>
                  <SelectItem value="day">
                    {{ t("admin.sshSecurity.day") }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <div
            class="grid gap-3 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
          >
            <div class="space-y-1">
              <Label class="text-sm font-medium">
                {{ t("admin.sshSecurity.allowedRegions") }}
              </Label>
              <p
                class="hidden pr-4 text-xs leading-5 text-muted-foreground sm:block"
              >
                {{ t("admin.sshSecurity.allowedRegionsDescription") }}
              </p>
            </div>
            <div class="w-full max-w-2xl space-y-3">
              <div class="flex flex-wrap items-center justify-between gap-3">
                <p class="text-sm leading-6 text-muted-foreground sm:hidden">
                  {{ t("admin.sshSecurity.allowedRegionsDescription") }}
                </p>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  :disabled="regionInputsDisabled || provinces.length === 0"
                  @click="openRegionDialog"
                >
                  <Plus class="h-4 w-4" />
                  {{ t("admin.sshSecurity.addRegion") }}
                </Button>
              </div>

              <div class="rounded-xl bg-muted/20 px-4 py-4">
                <TagsInput
                  :model-value="
                    form.allowedRegions.map((item) => selectionKey(item))
                  "
                  class="min-h-0 items-start gap-2 border-none bg-transparent px-0 py-0 shadow-none"
                >
                  <template v-if="form.allowedRegions.length > 0">
                    <TagsInputItem
                      v-for="selection in form.allowedRegions"
                      :key="selectionKey(selection)"
                      :value="selectionKey(selection)"
                      class="h-auto rounded-full border border-border/70 bg-background pr-1"
                    >
                      <TagsInputItemText class="px-3 py-1.5">
                        {{ selection.label }}
                      </TagsInputItemText>
                      <TagsInputItemDelete
                        v-if="form.enabled"
                        class="mr-1 rounded-full hover:bg-muted"
                        :disabled="regionInputsDisabled"
                        @click.prevent="removeRegion(selection)"
                      />
                    </TagsInputItem>
                  </template>
                  <span v-else class="px-1 py-1 text-sm text-muted-foreground">
                    {{ t("admin.sshSecurity.noRegions") }}
                  </span>
                </TagsInput>
              </div>
            </div>
          </div>

          <div
            class="grid gap-3 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
          >
            <div class="space-y-1">
              <Label for="ssh-custom-cidrs" class="text-sm font-medium">
                {{ t("admin.sshSecurity.customCidrs") }}
              </Label>
              <p
                class="hidden pr-4 text-xs leading-5 text-muted-foreground sm:block"
              >
                {{ t("admin.sshSecurity.customCidrsDescription") }}
              </p>
            </div>
            <div class="w-full max-w-2xl space-y-2">
              <Textarea
                id="ssh-custom-cidrs"
                v-model="form.customCidrsText"
                class="min-h-32 font-mono text-sm"
                placeholder="1.2.3.0/24"
                :disabled="isSaving"
              />
              <p
                class="text-sm"
                :class="
                  invalidCustomCidrs.length > 0
                    ? 'text-destructive'
                    : 'text-muted-foreground'
                "
              >
                {{
                  invalidCustomCidrs.length > 0
                    ? t("admin.sshSecurity.customCidrsInvalid", {
                        items: invalidCustomCidrs.join(
                          t("admin.sshSecurity.listSeparator"),
                        ),
                      })
                    : t("admin.sshSecurity.customCidrsRecognized", {
                        count: customCidrsState.cidrs.length,
                      })
                }}
              </p>
            </div>
          </div>
        </div>
      </template>

      <template #actions="{ collapse }">
        <Button variant="outline" @click="collapse">
          {{ t("admin.sshSecurity.collapse") }}
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button
              variant="outline"
              class="gap-2"
              :disabled="
                isSaving ||
                isSyncingFirewall ||
                !details ||
                !details.summary.available
              "
            >
              <Loader2 v-if="isSyncingFirewall" class="h-4 w-4 animate-spin" />
              <span>{{ t("admin.sshSecurity.actions") }}</span>
              <ChevronDown class="h-4 w-4 text-muted-foreground" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-56">
            <DropdownMenuItem
              :disabled="
                isSaving ||
                isSyncingFirewall ||
                !details ||
                !details.summary.available
              "
              @select="syncFirewall"
            >
              <RefreshCw class="h-4 w-4" />
              {{ t("admin.sshSecurity.syncFirewall") }}
            </DropdownMenuItem>
            <DropdownMenuItem
              class="text-destructive focus:text-destructive"
              :disabled="
                isSaving ||
                isSyncingFirewall ||
                !details ||
                !details.summary.available
              "
              @select="openClearFirewallDialog"
            >
              <Trash2 class="h-4 w-4" />
              {{ t("admin.sshSecurity.clearSshFirewall") }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <Button
          :disabled="
            isSaving || isSyncingFirewall || Boolean(saveBlockedReason)
          "
          @click="saveConfig"
        >
          <Save class="h-4 w-4" />
          {{
            isSaving
              ? t("admin.sshSecurity.saving")
              : t("admin.sshSecurity.saveConfig")
          }}
        </Button>
      </template>
    </ConfigCollapsibleCard>

    <Dialog
      :open="isRegionDialogOpen"
      @update:open="handleRegionDialogOpenChange"
    >
      <DialogContent
        class="overflow-hidden border-border/70 bg-background p-0 shadow-xl sm:max-w-[560px]"
      >
        <div class="px-6 pt-6 pb-2">
          <DialogHeader class="space-y-2 text-left">
            <DialogTitle class="text-xl font-semibold tracking-tight">
              {{ t("admin.sshSecurity.addRegion") }}
            </DialogTitle>
            <DialogDescription class="text-sm leading-6 text-muted-foreground">
              {{ t("admin.sshSecurity.addRegionDescription") }}
            </DialogDescription>
          </DialogHeader>
        </div>

        <div class="space-y-4 border-t border-border/60 px-6 py-5">
          <div class="grid gap-4 sm:grid-cols-2">
            <div class="space-y-2">
              <Label class="text-sm font-medium">
                {{ t("admin.sshSecurity.province") }}
              </Label>
              <Select v-model="regionDraft.province">
                <SelectTrigger
                  class="h-11 w-full rounded-lg border-border/70 bg-background px-3 shadow-none"
                  :disabled="regionInputsDisabled || provinces.length === 0"
                >
                  <SelectValue
                    :placeholder="t('admin.sshSecurity.selectProvince')"
                  />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="province in provinces"
                    :key="province.value"
                    :value="province.value"
                  >
                    {{ province.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div class="space-y-2">
              <Label class="text-sm font-medium">
                {{ t("admin.sshSecurity.scope") }}
              </Label>
              <Select :key="citySelectKey" v-model="regionDraft.cityValue">
                <SelectTrigger
                  class="h-11 w-full rounded-lg border-border/70 bg-background px-3 shadow-none"
                  :disabled="
                    regionInputsDisabled ||
                    !regionDraft.province ||
                    cityOptionsLoading ||
                    cityOptions.length === 0
                  "
                >
                  <Loader2
                    v-if="cityOptionsLoading"
                    class="h-4 w-4 animate-spin text-muted-foreground"
                  />
                  <SelectValue :placeholder="citySelectPlaceholder" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="city in cityOptions"
                    :key="city.value"
                    :value="city.value"
                  >
                    {{ city.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </div>

        <DialogFooter
          class="border-t border-border/60 px-6 py-4 sm:justify-end"
        >
          <Button
            variant="outline"
            @click="handleRegionDialogOpenChange(false)"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button :disabled="!canAddRegion || isSaving" @click="addRegion">
            {{ t("admin.sshSecurity.add") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Tabs v-model="activeTab" class="space-y-4">
      <TabsList>
        <TabsTrigger value="login-logs">
          {{ t("admin.sshSecurity.loginLogs") }}
        </TabsTrigger>
        <TabsTrigger value="blocks">
          {{ t("admin.sshSecurity.blockList") }}
        </TabsTrigger>
      </TabsList>

      <TabsContent value="login-logs" class="space-y-3">
        <div class="flex flex-wrap items-center gap-2">
          <SearchInput
            v-model="logSearch"
            :placeholder="t('admin.sshSecurity.searchLogsPlaceholder')"
            class="w-full max-w-xs"
            @search="handleLogSearch"
          />
          <Select v-model="logOutcome">
            <SelectTrigger class="w-[140px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">
                {{ t("admin.sshSecurity.allResults") }}
              </SelectItem>
              <SelectItem value="success">
                {{ t("admin.sshSecurity.success") }}
              </SelectItem>
              <SelectItem value="failure">
                {{ t("admin.sshSecurity.failure") }}
              </SelectItem>
            </SelectContent>
          </Select>
          <div class="flex-1"></div>
          <RefreshButton
            :loading="isLoadingLogs"
            :disabled="isLoadingLogs"
            @click="loadLogs"
          />
        </div>

        <Card class="border-border/60 shadow-none">
          <CardContent class="p-0">
            <div class="overflow-auto">
              <Table class="min-w-[760px]">
                <TableHeader>
                  <TableRow>
                    <TableHead class="h-11 w-[168px] px-4">
                      {{ t("admin.sshSecurity.time") }}
                    </TableHead>
                    <TableHead class="h-11 w-[92px] px-4">
                      {{ t("admin.sshSecurity.result") }}
                    </TableHead>
                    <TableHead class="h-11 min-w-[160px] px-4">
                      {{ t("admin.sshSecurity.user") }}
                    </TableHead>
                    <TableHead class="h-11 min-w-[220px] px-4">
                      {{ t("admin.sshSecurity.ipLocation") }}
                    </TableHead>
                    <TableHead class="h-11 min-w-[180px] px-4">
                      {{ t("admin.sshSecurity.method") }}
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow v-if="isLoadingLogs">
                    <TableCell colspan="5" class="px-4 py-10 text-center">
                      <Loader2
                        class="mx-auto h-6 w-6 animate-spin text-muted-foreground"
                      />
                    </TableCell>
                  </TableRow>
                  <TableRow v-else-if="logItems.length === 0">
                    <TableCell
                      colspan="5"
                      class="px-4 py-10 text-center text-muted-foreground"
                    >
                      {{ t("admin.sshSecurity.noLoginLogs") }}
                    </TableCell>
                  </TableRow>
                  <TableRow v-for="entry in logItems" v-else :key="entry.id">
                    <TableCell class="px-4 py-3 align-top whitespace-nowrap">
                      <HumanFriendlyTime :value="entry.happened_at" />
                    </TableCell>
                    <TableCell class="px-4 py-3 align-top">
                      <div class="flex flex-wrap items-center gap-1.5">
                        <Badge
                          :variant="
                            entry.outcome === 'success'
                              ? 'default'
                              : 'secondary'
                          "
                        >
                          {{
                            entry.outcome === "success"
                              ? t("admin.sshSecurity.success")
                              : t("admin.sshSecurity.failure")
                          }}
                        </Badge>
                      </div>
                    </TableCell>
                    <TableCell
                      class="min-w-[160px] px-4 py-3 align-top whitespace-normal"
                    >
                      <div class="font-medium">{{ entry.username }}</div>
                      <div
                        v-if="entry.invalid_user"
                        class="text-xs text-muted-foreground"
                      >
                        {{ t("admin.sshSecurity.invalidUser") }}
                      </div>
                    </TableCell>
                    <TableCell
                      class="min-w-[220px] px-4 py-3 align-top whitespace-normal"
                    >
                      <div class="font-mono text-sm">{{ entry.ip }}</div>
                      <div
                        v-if="entry.ipLocation"
                        class="mt-0.5 text-xs text-muted-foreground"
                      >
                        {{ entry.ipLocation }}
                      </div>
                    </TableCell>
                    <TableCell
                      class="min-w-[180px] px-4 py-3 align-top whitespace-normal"
                    >
                      <span class="break-words">
                        {{ entry.auth_method || "-" }}
                      </span>
                      <span
                        v-if="entry.port"
                        class="text-muted-foreground whitespace-nowrap"
                      >
                        / {{ entry.port }}
                      </span>
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </div>
            <PagedTableFooter
              :total="logTotal"
              :page="logPage"
              :limit="logLimit"
              :items-per-page="logParsedLimit"
              @update:page="handleLogPageChange"
              @update:limit="handleLogLimitChange"
            />
          </CardContent>
        </Card>
      </TabsContent>

      <TabsContent value="blocks" class="space-y-3">
        <div class="flex flex-wrap items-center gap-2">
          <SearchInput
            v-model="blockSearch"
            :placeholder="t('admin.sshSecurity.searchBlocksPlaceholder')"
            class="w-full max-w-xs"
            @search="handleBlockSearch"
          />
          <div class="flex-1"></div>
          <RefreshButton
            :loading="isLoadingBlocks"
            :disabled="isLoadingBlocks"
            @click="loadBlocks"
          />
          <ConfirmDangerPopover
            :title="
              t('admin.sshSecurity.confirmUnblockSelectedTitle', {
                count: selectedBlockIps.size,
              })
            "
            :description="t('admin.sshSecurity.unblockDescriptionText')"
            :loading="isDeleting"
            :disabled="selectedBlockIps.size === 0 || isDeleting"
            :on-confirm="() => deleteBlocks(Array.from(selectedBlockIps))"
          >
            <template #trigger>
              <Button
                variant="destructive"
                :disabled="selectedBlockIps.size === 0 || isDeleting"
              >
                <Trash2 class="h-4 w-4" />
                {{
                  t("admin.sshSecurity.deleteSelected", {
                    count: selectedBlockIps.size,
                  })
                }}
              </Button>
            </template>
          </ConfirmDangerPopover>
        </div>

        <Card class="border-border/60 shadow-none">
          <CardContent class="p-0">
            <div class="overflow-auto">
              <Table class="min-w-[860px]">
                <TableHeader>
                  <TableRow>
                    <TableHead class="h-11 w-[48px] px-3">
                      <Checkbox v-model="isAllBlocksSelected" />
                    </TableHead>
                    <TableHead class="h-11 min-w-[220px] px-4">
                      {{ t("admin.sshSecurity.ipLocation") }}
                    </TableHead>
                    <TableHead class="h-11 w-[168px] px-4">
                      {{ t("admin.sshSecurity.blockedAt") }}
                    </TableHead>
                    <TableHead class="h-11 w-[168px] px-4">
                      {{ t("admin.sshSecurity.expiresAt") }}
                    </TableHead>
                    <TableHead class="h-11 w-[120px] px-4">
                      {{ t("admin.sshSecurity.reason") }}
                    </TableHead>
                    <TableHead class="h-11 w-[120px] px-4">
                      {{ t("admin.sshSecurity.count") }}
                    </TableHead>
                    <TableHead class="h-11 w-[88px] px-4 text-right">
                      {{ t("admin.sshSecurity.actions") }}
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow v-if="isLoadingBlocks">
                    <TableCell colspan="7" class="px-4 py-10 text-center">
                      <Loader2
                        class="mx-auto h-6 w-6 animate-spin text-muted-foreground"
                      />
                    </TableCell>
                  </TableRow>
                  <TableRow v-else-if="blockRecords.length === 0">
                    <TableCell
                      colspan="7"
                      class="px-4 py-10 text-center text-muted-foreground"
                    >
                      {{ t("admin.sshSecurity.noBlockRecords") }}
                    </TableCell>
                  </TableRow>
                  <TableRow
                    v-for="record in blockRecords"
                    v-else
                    :key="record.ip"
                  >
                    <TableCell class="px-3 py-3 align-top">
                      <Checkbox
                        :model-value="selectedBlockIps.has(record.ip)"
                        @update:model-value="toggleBlockSelect(record.ip)"
                      />
                    </TableCell>
                    <TableCell
                      class="min-w-[220px] px-4 py-3 align-top whitespace-normal"
                    >
                      <div class="font-mono text-sm">{{ record.ip }}</div>
                      <div
                        v-if="record.ipLocation"
                        class="mt-0.5 text-xs text-muted-foreground"
                      >
                        {{ record.ipLocation }}
                      </div>
                    </TableCell>
                    <TableCell class="px-4 py-3 align-top whitespace-nowrap">
                      <HumanFriendlyTime :value="record.blocked_at" />
                    </TableCell>
                    <TableCell class="px-4 py-3 align-top whitespace-nowrap">
                      <HumanFriendlyTime :value="record.expires_at" />
                    </TableCell>
                    <TableCell class="px-4 py-3 align-top">
                      <Badge
                        :variant="record.applied ? 'secondary' : 'outline'"
                      >
                        {{ reasonLabel(record.reason) }}
                      </Badge>
                    </TableCell>
                    <TableCell class="px-4 py-3 align-top whitespace-nowrap">
                      {{ record.failed_count }} / {{ record.threshold }}
                    </TableCell>
                    <TableCell class="px-4 py-3 text-right align-top">
                      <ConfirmDangerPopover
                        :title="t('admin.sshSecurity.confirmUnblockOneTitle')"
                        :description="t('admin.sshSecurity.unblockDescriptionText')"
                        :loading="isDeleting"
                        :disabled="isDeleting"
                        :on-confirm="() => deleteOneBlock(record.ip)"
                      >
                        <template #trigger>
                          <Button
                            variant="ghost"
                            size="icon"
                            class="text-destructive"
                            :disabled="isDeleting"
                          >
                            <Trash2 class="h-4 w-4" />
                          </Button>
                        </template>
                      </ConfirmDangerPopover>
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </div>
            <PagedTableFooter
              :total="blockTotal"
              :page="blockPage"
              :limit="blockLimit"
              :items-per-page="blockParsedLimit"
              @update:page="handleBlockPageChange"
              @update:limit="handleBlockLimitChange"
            />
          </CardContent>
        </Card>
      </TabsContent>
    </Tabs>

    <Dialog v-model:open="isClearFirewallDialogOpen">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {{ t("admin.sshSecurity.clearFirewallTitle") }}
          </DialogTitle>
          <DialogDescription>
            {{ t("admin.sshSecurity.clearFirewallDescription") }}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            variant="outline"
            :disabled="isSyncingFirewall"
            @click="isClearFirewallDialogOpen = false"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            variant="destructive"
            :disabled="isSyncingFirewall"
            @click="clearFirewall"
          >
            <Loader2 v-if="isSyncingFirewall" class="h-4 w-4 animate-spin" />
            {{ t("admin.sshSecurity.clear") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
