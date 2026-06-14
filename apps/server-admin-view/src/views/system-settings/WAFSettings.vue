<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Download,
  Eye,
  Loader2,
  RefreshCw,
  TriangleAlert,
  Trash2,
  Upload,
} from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { toast } from "@admin-shared/utils/toast";
import { downloadBlob } from "@admin-shared/utils/downloadBlob";
import { WAFAPI } from "../../lib/api";
import type {
  WAFDetails,
  WAFRuleFile,
  WAFRuleFileContent,
  WAFRuleSource,
} from "../../types";
import { useConfigStore } from "../../store/config";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";

const { locale, t } = useI18n();
const levelOptions = computed(() => [
  {
    value: "1",
    label: t("admin.wafSettings.levels.daily"),
    description: t("admin.wafSettings.levels.dailyDescription"),
  },
  {
    value: "2",
    label: t("admin.wafSettings.levels.enhanced"),
    description: t("admin.wafSettings.levels.enhancedDescription"),
  },
  {
    value: "3",
    label: t("admin.wafSettings.levels.strict"),
    description: t("admin.wafSettings.levels.strictDescription"),
  },
  {
    value: "4",
    label: t("admin.wafSettings.levels.maximum"),
    description: t("admin.wafSettings.levels.maximumDescription"),
  },
] as const);
const SYSTEM_INITIALIZATION_RULE_FILENAME = "REQUEST-901-INITIALIZATION.conf";

const configStore = useConfigStore();
const details = ref<WAFDetails | null>(null);
const uploadInputRef = ref<HTMLInputElement | null>(null);
const selectedSystemRules = ref<string[]>([]);
const selectedCustomRules = ref<string[]>([]);
const activeRuleActionsKey = ref("");
const loadingRuleKey = ref("");
const downloadingRuleKey = ref("");
const isRulePreviewOpen = ref(false);
const activeRulePreview = ref<WAFRuleFileContent | null>(null);
const form = reactive({
  enabled: false,
  system_rules_auto_update_enabled: true,
  common_location_exempt_enabled: false,
  paranoia_level: 1,
  executing_paranoia_level: 1,
});

const { isPending: isLoading, run: runLoadDetails } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.wafSettings.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.wafSettings.loadDescription"),
      ),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.wafSettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.wafSettings.saveDescription"),
      ),
    });
  },
});
const { isPending: isUpdatingSystemRules, run: runUpdateSystemRules } =
  useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.wafSettings.updateFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wafSettings.systemUpdateDescription"),
        ),
      });
    },
  });
const { isPending: isUploading, run: runUploadRules } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.wafSettings.uploadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.wafSettings.uploadDescription"),
      ),
    });
  },
});
const { isPending: isChangingRules, run: runRuleChange } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.wafSettings.updateFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.wafSettings.ruleUpdateDescription"),
      ),
    });
  },
});

const isBusy = computed(
  () =>
    isSaving.value ||
    isUpdatingSystemRules.value ||
    isUploading.value ||
    isChangingRules.value,
);
const systemRules = computed(() =>
  (details.value?.system.rules || []).filter(
    (rule) => rule.filename !== SYSTEM_INITIALIZATION_RULE_FILENAME,
  ),
);
const customRules = computed(() => details.value?.custom.rules || []);
const sourceRules = (source: WAFRuleSource) =>
  source === "system" ? systemRules.value : customRules.value;
const allRulesEnabled = (source: WAFRuleSource) => {
  const rules = sourceRules(source);
  return rules.length > 0 && rules.every((rule) => rule.enabled);
};
const toggleAllRulesLabel = (source: WAFRuleSource) =>
  allRulesEnabled(source)
    ? t("admin.wafSettings.disableAll")
    : t("admin.wafSettings.enableAll");
const syncedLabel = computed(() => {
  const syncedAt = details.value?.system.synced?.synced_at;
  return syncedAt ? formatDate(syncedAt) : t("admin.wafSettings.notSynced");
});
const manifestLabel = computed(() => {
  const manifest = details.value?.system.manifest;
  if (!manifest) return t("admin.wafSettings.notFetched");
  return manifest.packagingTime
    ? formatDate(manifest.packagingTime)
    : t("admin.wafSettings.fetched");
});

const clampLevel = (value: unknown, fallback = 1) => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(4, Math.max(1, parsed));
};

const formatDate = (value?: string | null) => {
  if (!value) return "-";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(locale.value, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
};

const formatSize = (value: number) => {
  if (value >= 1024 * 1024) return `${(value / 1024 / 1024).toFixed(1)} MB`;
  if (value >= 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${value} B`;
};

const formatSystemRuleName = (filename: string) =>
  filename.replace(/\.conf$/i, "");

const sourceLabel = (source: WAFRuleSource) =>
  source === "system"
    ? t("admin.wafSettings.systemRules")
    : t("admin.wafSettings.customRules");

const ruleKey = (rule: Pick<WAFRuleFile, "source" | "filename">) =>
  `${rule.source}:${rule.filename}`;

const activateRuleActions = (rule: Pick<WAFRuleFile, "source" | "filename">) => {
  activeRuleActionsKey.value = ruleKey(rule);
};

const ruleActionsClass = (rule: Pick<WAFRuleFile, "source" | "filename">) =>
  activeRuleActionsKey.value === ruleKey(rule)
    ? "visible opacity-100"
    : "invisible opacity-0 group-hover:visible group-hover:opacity-100 group-focus-within:visible group-focus-within:opacity-100";

const applyFromDetails = (data: WAFDetails) => {
  details.value = data;
  form.enabled = data.config.enabled === true;
  form.system_rules_auto_update_enabled =
    data.config.system_rules_auto_update_enabled !== false;
  form.common_location_exempt_enabled =
    data.config.common_location_exempt_enabled === true;
  const level = clampLevel(data.config.paranoia_level, 1);
  form.paranoia_level = level;
  form.executing_paranoia_level = level;
  selectedSystemRules.value = [];
  selectedCustomRules.value = [];
};

const fetchDetails = async () => {
  await runLoadDetails(async () => {
    applyFromDetails(await WAFAPI.getDetails());
  });
};

const handleParanoiaLevelChange = (value: unknown) => {
  const level = clampLevel(value, 1);
  form.paranoia_level = level;
  form.executing_paranoia_level = level;
  return saveSettings(t("admin.wafSettings.protectionUpdated"));
};

const saveSettings = async (
  successMessage = t("admin.wafSettings.settingsUpdated"),
) => {
  await runSaveSettings(
    () =>
      WAFAPI.updateConfig({
        enabled: form.enabled,
        system_rules_auto_update_enabled: form.system_rules_auto_update_enabled,
        common_location_exempt_enabled: form.common_location_exempt_enabled,
        paranoia_level: form.paranoia_level,
        executing_paranoia_level: form.executing_paranoia_level,
      }),
    {
      onSuccess: async (data) => {
        applyFromDetails(data);
        toast.success(successMessage);
        await configStore.loadConfig();
      },
      onError: () => {
        if (details.value) applyFromDetails(details.value);
      },
    },
  );
};

const refreshAndSyncSystemRules = async () => {
  const refreshed = await WAFAPI.refreshManifest();
  if (!refreshed.system.update_available && refreshed.system.rules.length > 0) {
    return { details: refreshed, updated: false };
  }
  return { details: await WAFAPI.syncSystemRules(), updated: true };
};

const handleEnabledChange = async (enabled: boolean) => {
  if (form.enabled === enabled || isBusy.value) return;
  const previousEnabled = form.enabled;
  form.enabled = enabled;
  await runSaveSettings(
    async () => {
      if (enabled) {
        await refreshAndSyncSystemRules();
      }
      return WAFAPI.updateConfig({
        enabled,
        system_rules_auto_update_enabled: form.system_rules_auto_update_enabled,
        common_location_exempt_enabled: form.common_location_exempt_enabled,
        paranoia_level: form.paranoia_level,
        executing_paranoia_level: form.executing_paranoia_level,
      });
    },
    {
      onSuccess: async (data) => {
        applyFromDetails(data);
        toast.success(
          enabled
            ? t("admin.wafSettings.enabledTitle")
            : t("admin.wafSettings.disabledTitle"),
          {
            description: enabled
              ? t("admin.wafSettings.enabledDescription")
              : t("admin.wafSettings.disabledDescription"),
          },
        );
        await configStore.loadConfig();
      },
      onError: () => {
        form.enabled = previousEnabled;
        if (details.value) applyFromDetails(details.value);
      },
    },
  );
};

const handleCommonLocationExemptChange = async (enabled: boolean) => {
  if (form.common_location_exempt_enabled === enabled || isBusy.value) return;
  const previousEnabled = form.common_location_exempt_enabled;
  form.common_location_exempt_enabled = enabled;
  await runSaveSettings(
    () =>
      WAFAPI.updateConfig({
        common_location_exempt_enabled: enabled,
      }),
    {
      onSuccess: (data) => {
        applyFromDetails(data);
        toast.success(
          enabled
            ? t("admin.wafSettings.commonLocationEnabled")
            : t("admin.wafSettings.commonLocationDisabled"),
        );
      },
      onError: () => {
        form.common_location_exempt_enabled = previousEnabled;
        if (details.value) applyFromDetails(details.value);
      },
    },
  );
};

const handleAutoUpdateChange = async (enabled: boolean) => {
  if (form.system_rules_auto_update_enabled === enabled || isBusy.value) return;
  const previousEnabled = form.system_rules_auto_update_enabled;
  form.system_rules_auto_update_enabled = enabled;
  await runSaveSettings(
    () =>
      WAFAPI.updateConfig({
        system_rules_auto_update_enabled: enabled,
      }),
    {
      onSuccess: (data) => {
        applyFromDetails(data);
        toast.success(
          enabled
            ? t("admin.wafSettings.autoUpdateEnabled")
            : t("admin.wafSettings.autoUpdateDisabled"),
          {
            description: enabled
              ? t("admin.wafSettings.autoUpdateEnabledDescription")
              : t("admin.wafSettings.autoUpdateDisabledDescription"),
          },
        );
      },
      onError: () => {
        form.system_rules_auto_update_enabled = previousEnabled;
        if (details.value) applyFromDetails(details.value);
      },
    },
  );
};

const updateSystemRules = async () => {
  await runUpdateSystemRules(refreshAndSyncSystemRules, {
    onSuccess: ({ details: nextDetails, updated }) => {
      applyFromDetails(nextDetails);
      if (updated) {
        toast.success(t("admin.wafSettings.systemRulesUpdated"), {
          description: nextDetails.config.enabled
            ? t("admin.wafSettings.loadedToGateway")
            : t("admin.wafSettings.rulesApplyWhenEnabled"),
        });
        return;
      }
      toast.success(t("admin.wafSettings.rulesLatest"), {
        description: t("admin.wafSettings.noNewSystemRules"),
      });
    },
  });
};

const selectionRef = (source: WAFRuleSource) =>
  source === "system" ? selectedSystemRules : selectedCustomRules;

const selectedCount = (source: WAFRuleSource) =>
  selectionRef(source).value.length;

const setRuleSelected = (
  source: WAFRuleSource,
  filename: string,
  checked: boolean,
) => {
  const target = selectionRef(source);
  target.value = checked
    ? [...new Set([...target.value, filename])]
    : target.value.filter((item) => item !== filename);
};

const setAllSelected = (source: WAFRuleSource, checked: boolean) => {
  selectionRef(source).value = checked
    ? (source === "system" ? systemRules.value : customRules.value).map(
        (rule) => rule.filename,
      )
    : [];
};

const updateRulesEnabled = async (
  source: WAFRuleSource,
  filenames: string[] | undefined,
  enabled: boolean,
) => {
  await runRuleChange(
    () =>
      WAFAPI.setRulesEnabled({
        source,
        filenames,
        enabled,
      }),
    {
      onSuccess: (data) => {
        applyFromDetails(data);
        toast.success(
          enabled
            ? t("admin.wafSettings.ruleEnabled")
            : t("admin.wafSettings.ruleDisabled"),
          {
            description: data.config.enabled
              ? t("admin.wafSettings.loadedToGateway")
              : t("admin.wafSettings.currentRulesApplyWhenEnabled"),
          },
        );
      },
    },
  );
};

const toggleRule = (rule: WAFRuleFile, enabled: boolean) =>
  updateRulesEnabled(rule.source, [rule.filename], enabled);

const updateSelectedRules = (source: WAFRuleSource, enabled: boolean) => {
  const filenames = selectionRef(source).value;
  if (filenames.length === 0) return;
  return updateRulesEnabled(source, filenames, enabled);
};

const toggleAllRules = (source: WAFRuleSource) =>
  updateRulesEnabled(source, undefined, !allRulesEnabled(source));

const openRulePreview = async (rule: WAFRuleFile) => {
  const key = ruleKey(rule);
  activateRuleActions(rule);
  loadingRuleKey.value = key;
  try {
    activeRulePreview.value = await WAFAPI.getRuleFile(
      rule.source,
      rule.filename,
    );
    isRulePreviewOpen.value = true;
  } catch (error) {
    toast.error(t("admin.wafSettings.readFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.wafSettings.ruleReadDescription"),
      ),
    });
  } finally {
    if (loadingRuleKey.value === key) loadingRuleKey.value = "";
  }
};

const downloadRuleFile = async (rule: WAFRuleFile) => {
  const key = ruleKey(rule);
  activateRuleActions(rule);
  downloadingRuleKey.value = key;
  try {
    const data = await WAFAPI.getRuleFile(rule.source, rule.filename);
    downloadBlob(
      new Blob([data.content], { type: "text/plain;charset=utf-8" }),
      data.filename || rule.filename,
    );
  } catch (error) {
    toast.error(t("admin.wafSettings.downloadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.wafSettings.ruleDownloadDescription"),
      ),
    });
  } finally {
    if (downloadingRuleKey.value === key) downloadingRuleKey.value = "";
  }
};

const readFileAsBase64 = (file: File): Promise<string> =>
  new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const value = String(reader.result || "");
      resolve(value.includes(",") ? value.split(",")[1] || "" : value);
    };
    reader.onerror = () =>
      reject(reader.error || new Error(t("admin.wafSettings.fileReadFailed")));
    reader.readAsDataURL(file);
  });

const triggerUpload = () => uploadInputRef.value?.click();

const handleUploadChange = async (event: Event) => {
  const input = event.target as HTMLInputElement;
  const files = Array.from(input.files || []);
  input.value = "";
  if (files.length === 0) return;
  await runUploadRules(
    async () =>
      WAFAPI.uploadCustomRules({
        files: await Promise.all(
          files.map(async (file) => ({
            filename: file.name,
            content_base64: await readFileAsBase64(file),
          })),
        ),
      }),
    {
      onSuccess: (data) => {
        applyFromDetails(data);
        toast.success(t("admin.wafSettings.customRulesUploaded"), {
          description: data.config.enabled
            ? t("admin.wafSettings.loadedToGateway")
            : t("admin.wafSettings.currentRulesApplyWhenEnabled"),
        });
      },
    },
  );
};

const deleteCustomRule = async (filename: string) => {
  await runRuleChange(() => WAFAPI.deleteCustomRule(filename), {
    onSuccess: (data) => {
      applyFromDetails(data);
      toast.success(t("admin.wafSettings.customRuleDeleted"), {
        description: data.config.enabled
          ? t("admin.wafSettings.loadedToGateway")
          : t("admin.wafSettings.currentRulesApplyWhenEnabled"),
      });
    },
  });
};

onMounted(fetchDetails);
</script>

<template>
  <TooltipProvider>
    <Card>
      <CardHeader>
        <div class="space-y-1.5">
          <CardTitle class="text-md">
            {{ t("admin.wafSettings.title") }}
          </CardTitle>
          <CardDescription>
            {{ t("admin.wafSettings.description") }}
          </CardDescription>
        </div>
      </CardHeader>

      <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
        <div class="space-y-4 p-6">
          <Skeleton class="h-6 w-1/3" />
          <Skeleton class="h-4 w-2/3" />
          <Skeleton class="h-24 w-full" />
        </div>
      </CardContent>

      <CardContent v-else class="border-t p-0 divide-y">
        <section v-if="form.enabled" class="p-6">
          <Alert
            class="items-start rounded-xl border-amber-200 bg-amber-50/70 text-amber-950 [&>svg]:text-amber-600"
          >
            <TriangleAlert class="mt-0.5 h-4 w-4" />
            <AlertTitle>
              {{ t("admin.wafSettings.falsePositiveTitle") }}
            </AlertTitle>
            <AlertDescription class="text-sm leading-6 text-amber-900">
              {{ t("admin.wafSettings.falsePositiveDescription") }}
            </AlertDescription>
          </Alert>
        </section>

        <section
          class="flex flex-col gap-4 bg-muted/10 p-6 sm:flex-row sm:items-center sm:justify-between"
        >
          <div class="space-y-1 pr-6">
            <Label
              class="cursor-pointer text-base font-medium"
              @click="handleEnabledChange(!form.enabled)"
            >
              {{ t("admin.wafSettings.enableWaf") }}
            </Label>
            <div class="text-sm text-muted-foreground">
              {{ t("admin.wafSettings.enableWafDescription") }}
            </div>
          </div>
          <Switch
            :model-value="form.enabled"
            :disabled="isBusy"
            @update:model-value="(value) => handleEnabledChange(value === true)"
          />
        </section>

        <section
          v-if="form.enabled"
          class="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between"
        >
          <div class="space-y-1 pr-6">
            <Label
              class="cursor-pointer text-base font-medium"
              @click="
                handleAutoUpdateChange(!form.system_rules_auto_update_enabled)
              "
            >
              {{ t("admin.wafSettings.autoUpdate") }}
            </Label>
            <div class="text-sm text-muted-foreground">
              {{ t("admin.wafSettings.autoUpdateDescription") }}
            </div>
          </div>
          <Switch
            :model-value="form.system_rules_auto_update_enabled"
            :disabled="isBusy"
            @update:model-value="
              (value) => handleAutoUpdateChange(value === true)
            "
          />
        </section>

        <section
          v-if="form.enabled"
          class="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between"
        >
          <div class="space-y-1 pr-6">
            <Label
              class="cursor-pointer text-base font-medium"
              @click="
                handleCommonLocationExemptChange(
                  !form.common_location_exempt_enabled,
                )
              "
            >
              {{ t("admin.wafSettings.commonLocationExempt") }}
            </Label>
            <div class="text-sm text-muted-foreground">
              {{ t("admin.wafSettings.commonLocationExemptDescription") }}
            </div>
          </div>
          <Switch
            :model-value="form.common_location_exempt_enabled"
            :disabled="isBusy"
            @update:model-value="
              (value) => handleCommonLocationExemptChange(value === true)
            "
          />
        </section>

        <template v-if="form.enabled">
          <section
            class="grid gap-6 p-6 lg:grid-cols-[minmax(0,1fr)_minmax(360px,520px)]"
          >
            <div class="space-y-1 pr-6">
              <Label class="text-base">
                {{ t("admin.wafSettings.protectionLevel") }}
              </Label>
              <div class="text-sm text-muted-foreground">
                {{ t("admin.wafSettings.protectionLevelDescription") }}
              </div>
            </div>
            <div class="grid justify-items-end gap-5">
              <Select
                :model-value="String(form.paranoia_level)"
                :disabled="isBusy"
                @update:model-value="handleParanoiaLevelChange"
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="level in levelOptions"
                    :key="level.value"
                    :value="level.value"
                  >
                    {{ level.label }} · {{ level.description }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </section>

          <section class="space-y-5 p-6">
            <div
              class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
            >
              <div class="space-y-1">
                <div class="flex items-center gap-2">
                  <Label class="text-base">
                    {{ t("admin.wafSettings.systemRules") }}
                  </Label>
                  <Badge
                    v-if="details?.system.update_available"
                    variant="secondary"
                  >
                    {{ t("admin.wafSettings.updateAvailable") }}
                  </Badge>
                </div>
                <div class="text-sm text-muted-foreground">
                  {{
                    t("admin.wafSettings.manifestLocal", {
                      manifest: manifestLabel,
                      synced: syncedLabel,
                    })
                  }}
                </div>
                <div
                  v-if="details?.system.manifest_last_error"
                  class="text-sm text-destructive"
                >
                  {{ details.system.manifest_last_error }}
                </div>
              </div>
              <div class="flex flex-wrap gap-2">
                <Button size="sm" :disabled="isBusy" @click="updateSystemRules">
                  <RefreshCw
                    class="mr-2 h-4 w-4"
                    :class="isUpdatingSystemRules ? 'animate-spin' : ''"
                  />
                  {{ t("admin.wafSettings.updateRules") }}
                </Button>
              </div>
            </div>

            <div
              v-if="systemRules.length === 0"
              class="text-sm text-muted-foreground"
            >
              {{ t("admin.wafSettings.notSyncedSystemRules") }}
            </div>
            <div v-else class="overflow-hidden rounded-md border">
              <div
                class="flex flex-col gap-3 border-b bg-muted/10 px-4 py-3 sm:flex-row sm:items-center sm:justify-between"
              >
                <label class="flex items-center gap-3 text-sm">
                  <Checkbox
                    :model-value="
                      selectedSystemRules.length === systemRules.length &&
                      systemRules.length > 0
                    "
                    :disabled="isBusy"
                    @update:model-value="
                      (value) => setAllSelected('system', value === true)
                    "
                  />
                  <span>
                    {{
                      t("admin.wafSettings.selectedCount", {
                        count: selectedCount("system"),
                      })
                    }}
                  </span>
                </label>
                <div class="flex flex-wrap gap-2">
                  <Button
                    v-if="selectedCount('system') > 0"
                    variant="outline"
                    size="sm"
                    :disabled="isBusy"
                    @click="updateSelectedRules('system', true)"
                  >
                    {{ t("admin.wafSettings.enableSelected") }}
                  </Button>
                  <Button
                    v-if="selectedCount('system') > 0"
                    variant="outline"
                    size="sm"
                    :disabled="isBusy"
                    @click="updateSelectedRules('system', false)"
                  >
                    {{ t("admin.wafSettings.disableSelected") }}
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    :disabled="isBusy"
                    @click="toggleAllRules('system')"
                  >
                    {{ toggleAllRulesLabel("system") }}
                  </Button>
                </div>
              </div>

              <div class="divide-y">
                <div
                  v-for="rule in systemRules"
                  :key="rule.filename"
                  class="group flex flex-col gap-3 px-4 py-3 sm:flex-row sm:items-center"
                  @pointerdown.passive="activateRuleActions(rule)"
                  @touchstart.passive="activateRuleActions(rule)"
                >
                  <Checkbox
                    :model-value="selectedSystemRules.includes(rule.filename)"
                    :disabled="isBusy"
                    @update:model-value="
                      (value) =>
                        setRuleSelected('system', rule.filename, value === true)
                    "
                  />
                  <div class="min-w-0 flex-1 space-y-1">
                    <div class="flex min-w-0 items-center gap-2">
                      <div class="min-w-0 truncate font-mono text-sm">
                        {{ formatSystemRuleName(rule.filename) }}
                      </div>
                      <div
                        class="flex h-8 shrink-0 items-center gap-1 transition-opacity duration-150"
                        :class="ruleActionsClass(rule)"
                      >
                        <Tooltip>
                          <TooltipTrigger as-child>
                            <Button
                              variant="ghost"
                              size="icon"
                              class="h-8 w-8 text-muted-foreground hover:text-foreground"
                              :disabled="loadingRuleKey === ruleKey(rule)"
                              :title="t('admin.wafSettings.viewRule')"
                              :aria-label="t('admin.wafSettings.viewRule')"
                              @click.stop="openRulePreview(rule)"
                            >
                              <Loader2
                                v-if="loadingRuleKey === ruleKey(rule)"
                                class="h-4 w-4 animate-spin"
                              />
                              <Eye v-else class="h-4 w-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>
                            {{ t("admin.wafSettings.viewRule") }}
                          </TooltipContent>
                        </Tooltip>
                        <Tooltip>
                          <TooltipTrigger as-child>
                            <Button
                              variant="ghost"
                              size="icon"
                              class="h-8 w-8 text-muted-foreground hover:text-foreground"
                              :disabled="downloadingRuleKey === ruleKey(rule)"
                              :title="t('admin.wafSettings.downloadRule')"
                              :aria-label="t('admin.wafSettings.downloadRule')"
                              @click.stop="downloadRuleFile(rule)"
                            >
                              <Loader2
                                v-if="downloadingRuleKey === ruleKey(rule)"
                                class="h-4 w-4 animate-spin"
                              />
                              <Download v-else class="h-4 w-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>
                            {{ t("admin.wafSettings.downloadRule") }}
                          </TooltipContent>
                        </Tooltip>
                      </div>
                    </div>
                    <div class="text-sm text-muted-foreground">
                      {{ rule.description }}
                    </div>
                  </div>
                  <div
                    class="flex items-center justify-between gap-4 sm:justify-end"
                  >
                    <span class="text-xs text-muted-foreground">
                      {{ formatSize(rule.size_bytes) }}
                    </span>
                    <Switch
                      :model-value="rule.enabled"
                      :disabled="isBusy"
                      @update:model-value="
                        (value) => toggleRule(rule, value === true)
                      "
                    />
                  </div>
                </div>
              </div>
            </div>
          </section>

          <section class="space-y-5 p-6">
            <div
              class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
            >
              <div class="space-y-1">
                <Label class="text-base">
                  {{ t("admin.wafSettings.customRules") }}
                </Label>
                <div class="text-sm text-muted-foreground">
                  {{ t("admin.wafSettings.customRulesDescription") }}
                </div>
              </div>
              <div>
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="isBusy"
                  @click="triggerUpload"
                >
                  <Upload class="mr-2 h-4 w-4" />
                  {{ t("admin.wafSettings.uploadRules") }}
                </Button>
                <input
                  ref="uploadInputRef"
                  type="file"
                  class="hidden"
                  accept=".conf"
                  multiple
                  @change="handleUploadChange"
                />
              </div>
            </div>

            <div
              v-if="customRules.length === 0"
              class="text-sm text-muted-foreground"
            >
              {{ t("admin.wafSettings.noCustomRules") }}
            </div>
            <div v-else class="overflow-hidden rounded-md border">
              <div
                class="flex flex-col gap-3 border-b bg-muted/10 px-4 py-3 sm:flex-row sm:items-center sm:justify-between"
              >
                <label class="flex items-center gap-3 text-sm">
                  <Checkbox
                    :model-value="
                      selectedCustomRules.length === customRules.length &&
                      customRules.length > 0
                    "
                    :disabled="isBusy"
                    @update:model-value="
                      (value) => setAllSelected('custom', value === true)
                    "
                  />
                  <span>
                    {{
                      t("admin.wafSettings.selectedCount", {
                        count: selectedCount("custom"),
                      })
                    }}
                  </span>
                </label>
                <div class="flex flex-wrap gap-2">
                  <Button
                    v-if="selectedCount('custom') > 0"
                    variant="outline"
                    size="sm"
                    :disabled="isBusy"
                    @click="updateSelectedRules('custom', true)"
                  >
                    {{ t("admin.wafSettings.enableSelected") }}
                  </Button>
                  <Button
                    v-if="selectedCount('custom') > 0"
                    variant="outline"
                    size="sm"
                    :disabled="isBusy"
                    @click="updateSelectedRules('custom', false)"
                  >
                    {{ t("admin.wafSettings.disableSelected") }}
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    :disabled="isBusy"
                    @click="toggleAllRules('custom')"
                  >
                    {{ toggleAllRulesLabel("custom") }}
                  </Button>
                </div>
              </div>

              <div class="divide-y">
                <div
                  v-for="rule in customRules"
                  :key="rule.filename"
                  class="group flex flex-col gap-3 px-4 py-3 sm:flex-row sm:items-center"
                  @pointerdown.passive="activateRuleActions(rule)"
                  @touchstart.passive="activateRuleActions(rule)"
                >
                  <Checkbox
                    :model-value="selectedCustomRules.includes(rule.filename)"
                    :disabled="isBusy"
                    @update:model-value="
                      (value) =>
                        setRuleSelected('custom', rule.filename, value === true)
                    "
                  />
                  <div class="min-w-0 flex-1 space-y-1">
                    <div class="flex min-w-0 items-center gap-2">
                      <div class="min-w-0 truncate font-mono text-sm">
                        {{ rule.filename }}
                      </div>
                      <div
                        class="flex h-8 shrink-0 items-center gap-1 transition-opacity duration-150"
                        :class="ruleActionsClass(rule)"
                      >
                        <Tooltip>
                          <TooltipTrigger as-child>
                            <Button
                              variant="ghost"
                              size="icon"
                              class="h-8 w-8 text-muted-foreground hover:text-foreground"
                              :disabled="loadingRuleKey === ruleKey(rule)"
                              :title="t('admin.wafSettings.viewRule')"
                              :aria-label="t('admin.wafSettings.viewRule')"
                              @click.stop="openRulePreview(rule)"
                            >
                              <Loader2
                                v-if="loadingRuleKey === ruleKey(rule)"
                                class="h-4 w-4 animate-spin"
                              />
                              <Eye v-else class="h-4 w-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>
                            {{ t("admin.wafSettings.viewRule") }}
                          </TooltipContent>
                        </Tooltip>
                        <Tooltip>
                          <TooltipTrigger as-child>
                            <Button
                              variant="ghost"
                              size="icon"
                              class="h-8 w-8 text-muted-foreground hover:text-foreground"
                              :disabled="downloadingRuleKey === ruleKey(rule)"
                              :title="t('admin.wafSettings.downloadRule')"
                              :aria-label="t('admin.wafSettings.downloadRule')"
                              @click.stop="downloadRuleFile(rule)"
                            >
                              <Loader2
                                v-if="downloadingRuleKey === ruleKey(rule)"
                                class="h-4 w-4 animate-spin"
                              />
                              <Download v-else class="h-4 w-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>
                            {{ t("admin.wafSettings.downloadRule") }}
                          </TooltipContent>
                        </Tooltip>
                      </div>
                    </div>
                    <div class="text-sm text-muted-foreground">
                      {{ formatSize(rule.size_bytes) }} ·
                      {{ formatDate(rule.updated_at) }}
                    </div>
                  </div>
                  <div
                    class="flex items-center justify-between gap-3 sm:justify-end"
                  >
                    <Switch
                      :model-value="rule.enabled"
                      :disabled="isBusy"
                      @update:model-value="
                        (value) => toggleRule(rule, value === true)
                      "
                    />
                    <ConfirmDangerPopover
                      :title="
                        t('admin.wafSettings.deleteConfirmTitle', {
                          filename: rule.filename,
                        })
                      "
                      :description="
                        t('admin.wafSettings.deleteConfirmDescription')
                      "
                      :loading="isChangingRules"
                      :disabled="isBusy"
                      :on-confirm="() => deleteCustomRule(rule.filename)"
                    >
                      <template #trigger>
                        <Button
                          variant="outline"
                          size="icon"
                          class="h-8 w-8 border-destructive/30 text-destructive hover:bg-destructive/10 hover:text-destructive"
                          :disabled="isBusy"
                        >
                          <Trash2 class="h-4 w-4" />
                        </Button>
                      </template>
                    </ConfirmDangerPopover>
                  </div>
                </div>
              </div>
            </div>
          </section>
        </template>
      </CardContent>
    </Card>
    <DetailDialog
      v-model:open="isRulePreviewOpen"
      :title="activeRulePreview?.filename || t('admin.wafSettings.ruleContent')"
      :description="
        activeRulePreview
          ? `${sourceLabel(activeRulePreview.source)} · ${formatSize(activeRulePreview.size_bytes)} · ${formatDate(activeRulePreview.updated_at)}`
          : ''
      "
      max-width-class="sm:max-w-[840px]"
      close-variant="default"
    >
      <div
        v-if="activeRulePreview"
        class="overflow-hidden rounded-md border bg-muted/20"
      >
        <pre
          class="max-h-[60vh] overflow-auto whitespace-pre-wrap break-words p-3 font-mono text-xs leading-5 text-foreground"
      >{{ activeRulePreview.content }}</pre>
      </div>
    </DetailDialog>
  </TooltipProvider>
</template>
