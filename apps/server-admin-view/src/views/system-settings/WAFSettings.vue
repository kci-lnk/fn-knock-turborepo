<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
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

const LEVEL_OPTIONS = [
  {
    value: "1",
    label: "日常防护",
    description: "推荐",
  },
  {
    value: "2",
    label: "加强防护",
    description: "更敏感",
  },
  {
    value: "3",
    label: "严格防护",
    description: "误拦截可能增加",
  },
  {
    value: "4",
    label: "最高防护",
    description: "仅建议排查时使用",
  },
] as const;
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
    toast.error("加载失败", {
      description: extractErrorMessage(error, "无法获取 WAF 设置"),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "WAF 设置保存失败"),
    });
  },
});
const { isPending: isUpdatingSystemRules, run: runUpdateSystemRules } =
  useAsyncAction({
    onError: (error) => {
      toast.error("更新失败", {
        description: extractErrorMessage(error, "系统规则更新失败"),
      });
    },
  });
const { isPending: isUploading, run: runUploadRules } = useAsyncAction({
  onError: (error) => {
    toast.error("上传失败", {
      description: extractErrorMessage(error, "自定义规则上传失败"),
    });
  },
});
const { isPending: isChangingRules, run: runRuleChange } = useAsyncAction({
  onError: (error) => {
    toast.error("更新失败", {
      description: extractErrorMessage(error, "规则状态更新失败"),
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
  allRulesEnabled(source) ? "关闭全部" : "开启全部";
const syncedLabel = computed(() => {
  const syncedAt = details.value?.system.synced?.synced_at;
  return syncedAt ? formatDate(syncedAt) : "尚未同步";
});
const manifestLabel = computed(() => {
  const manifest = details.value?.system.manifest;
  if (!manifest) return "未获取";
  return manifest.packagingTime ? formatDate(manifest.packagingTime) : "已获取";
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
  return date.toLocaleString("zh-CN", {
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
  source === "system" ? "系统规则" : "自定义规则";

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
  return saveSettings("WAF 防护强度已更新");
};

const saveSettings = async (successMessage = "WAF 设置已更新") => {
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
        toast.success(enabled ? "WAF 已开启" : "WAF 已关闭", {
          description: enabled
            ? "已更新系统规则并加载到 Go 网关。"
            : "Go 网关会立即跳过 WAF 检查。",
        });
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
        toast.success(enabled ? "常用地豁免已开启" : "常用地豁免已关闭");
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
        toast.success(enabled ? "自动更新已开启" : "自动更新已关闭", {
          description: enabled
            ? "后端会自动维护 WAF 系统规则更新。"
            : "后端不会再自动检查 WAF 系统规则更新。",
        });
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
        toast.success("系统规则已更新", {
          description: nextDetails.config.enabled
            ? "已自动加载到 Go 网关。"
            : "WAF 开启后会按这些规则生效。",
        });
        return;
      }
      toast.success("规则已是最新", {
        description: "已检查清单，没有发现新的系统规则。",
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
        toast.success(enabled ? "规则已开启" : "规则已关闭", {
          description: data.config.enabled
            ? "已自动加载到 Go 网关。"
            : "WAF 开启后会按当前规则生效。",
        });
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
    toast.error("读取失败", {
      description: extractErrorMessage(error, "WAF 规则读取失败"),
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
    toast.error("下载失败", {
      description: extractErrorMessage(error, "WAF 规则下载失败"),
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
    reader.onerror = () => reject(reader.error || new Error("读取文件失败"));
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
        toast.success("自定义规则已上传", {
          description: data.config.enabled
            ? "已自动加载到 Go 网关。"
            : "WAF 开启后会按当前规则生效。",
        });
      },
    },
  );
};

const deleteCustomRule = async (filename: string) => {
  await runRuleChange(() => WAFAPI.deleteCustomRule(filename), {
    onSuccess: (data) => {
      applyFromDetails(data);
      toast.success("自定义规则已删除", {
        description: data.config.enabled
          ? "已自动加载到 Go 网关。"
          : "WAF 开启后会按当前规则生效。",
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
          <CardTitle class="text-md">Web 防护</CardTitle>
          <CardDescription>
            使用系统规则和上传规则保护网关请求；开关、强度和规则操作会即时同步到
            Go 网关。
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
            <AlertTitle>规则误报提示</AlertTitle>
            <AlertDescription class="text-sm leading-6 text-amber-900">
              默认已关闭高频误报的系统规则文件；手动开启更多规则后，部分正常请求仍可能被误判并拦截。如果发现误报，请及时反馈。QQ群：1081609274
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
              启用WAF
            </Label>
            <div class="text-sm text-muted-foreground">
              默认关闭。开启时会先更新系统规则，并跳过高频误报规则文件，再按当前强度加载到
              Go 网关；关闭后网关会立即跳过 WAF 检查。
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
              规则自动更新
            </Label>
            <div class="text-sm text-muted-foreground">
              开启后会自动维护 WAF 系统规则更新，失败后后续会继续尝试。
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
              常用地豁免
            </Label>
            <div class="text-sm text-muted-foreground">
              来自最近登录常用地区的请求会跳过 WAF 检查。
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
              <Label class="text-base">防护强度</Label>
              <div class="text-sm text-muted-foreground">
                日常使用建议保持 1 级。等级越高越严格，也越可能误拦截。
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
                    v-for="level in LEVEL_OPTIONS"
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
                  <Label class="text-base">系统规则</Label>
                  <Badge
                    v-if="details?.system.update_available"
                    variant="secondary"
                  >
                    有更新
                  </Badge>
                </div>
                <div class="text-sm text-muted-foreground">
                  清单 {{ manifestLabel }} · 本地 {{ syncedLabel }}
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
                  更新规则
                </Button>
              </div>
            </div>

            <div
              v-if="systemRules.length === 0"
              class="text-sm text-muted-foreground"
            >
              尚未同步系统规则
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
                  <span>已选择 {{ selectedCount("system") }} 个</span>
                </label>
                <div class="flex flex-wrap gap-2">
                  <Button
                    v-if="selectedCount('system') > 0"
                    variant="outline"
                    size="sm"
                    :disabled="isBusy"
                    @click="updateSelectedRules('system', true)"
                  >
                    开启所选
                  </Button>
                  <Button
                    v-if="selectedCount('system') > 0"
                    variant="outline"
                    size="sm"
                    :disabled="isBusy"
                    @click="updateSelectedRules('system', false)"
                  >
                    关闭所选
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
                              title="查看规则"
                              aria-label="查看规则"
                              @click.stop="openRulePreview(rule)"
                            >
                              <Loader2
                                v-if="loadingRuleKey === ruleKey(rule)"
                                class="h-4 w-4 animate-spin"
                              />
                              <Eye v-else class="h-4 w-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>查看规则</TooltipContent>
                        </Tooltip>
                        <Tooltip>
                          <TooltipTrigger as-child>
                            <Button
                              variant="ghost"
                              size="icon"
                              class="h-8 w-8 text-muted-foreground hover:text-foreground"
                              :disabled="downloadingRuleKey === ruleKey(rule)"
                              title="下载规则"
                              aria-label="下载规则"
                              @click.stop="downloadRuleFile(rule)"
                            >
                              <Loader2
                                v-if="downloadingRuleKey === ruleKey(rule)"
                                class="h-4 w-4 animate-spin"
                              />
                              <Download v-else class="h-4 w-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>下载规则</TooltipContent>
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
                <Label class="text-base">自定义规则</Label>
                <div class="text-sm text-muted-foreground">
                  上传 `.conf` 文件后可单独开启或关闭。
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
                  上传规则
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
              暂无自定义规则
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
                  <span>已选择 {{ selectedCount("custom") }} 个</span>
                </label>
                <div class="flex flex-wrap gap-2">
                  <Button
                    v-if="selectedCount('custom') > 0"
                    variant="outline"
                    size="sm"
                    :disabled="isBusy"
                    @click="updateSelectedRules('custom', true)"
                  >
                    开启所选
                  </Button>
                  <Button
                    v-if="selectedCount('custom') > 0"
                    variant="outline"
                    size="sm"
                    :disabled="isBusy"
                    @click="updateSelectedRules('custom', false)"
                  >
                    关闭所选
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
                              title="查看规则"
                              aria-label="查看规则"
                              @click.stop="openRulePreview(rule)"
                            >
                              <Loader2
                                v-if="loadingRuleKey === ruleKey(rule)"
                                class="h-4 w-4 animate-spin"
                              />
                              <Eye v-else class="h-4 w-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>查看规则</TooltipContent>
                        </Tooltip>
                        <Tooltip>
                          <TooltipTrigger as-child>
                            <Button
                              variant="ghost"
                              size="icon"
                              class="h-8 w-8 text-muted-foreground hover:text-foreground"
                              :disabled="downloadingRuleKey === ruleKey(rule)"
                              title="下载规则"
                              aria-label="下载规则"
                              @click.stop="downloadRuleFile(rule)"
                            >
                              <Loader2
                                v-if="downloadingRuleKey === ruleKey(rule)"
                                class="h-4 w-4 animate-spin"
                              />
                              <Download v-else class="h-4 w-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>下载规则</TooltipContent>
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
                      :title="`删除 ${rule.filename}？`"
                      description="删除后不会再加载这个自定义规则。"
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
      :title="activeRulePreview?.filename || '规则内容'"
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
