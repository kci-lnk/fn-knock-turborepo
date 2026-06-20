<script setup lang="ts">
import { computed, onMounted, reactive, ref, toRef } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { parseCidrTextarea } from "@admin-shared/utils/cidr";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import { Button } from "@/components/ui/button";
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  TagsInput,
  TagsInputItem,
  TagsInputItemDelete,
  TagsInputItemText,
} from "@/components/ui/tags-input";
import { Textarea } from "@/components/ui/textarea";
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
  CidrProvinceOption,
  SSHSecurityDetails,
  SSHSecuritySelection,
} from "../types";
import SSHBlockListPanel from "./ssh-security/SSHBlockListPanel.vue";
import SSHLoginLogsPanel from "./ssh-security/SSHLoginLogsPanel.vue";
import { useSSHAllowedRegions } from "./ssh-security/useSSHAllowedRegions";

type SSHBlockListPanelInstance = {
  loadBlocks: () => Promise<void>;
};

const configStore = useConfigStore();
const { t } = useI18n();
const details = ref<SSHSecurityDetails | null>(null);
const provinces = ref<CidrProvinceOption[]>([]);
const activeTab = ref("login-logs");
const isClearFirewallDialogOpen = ref(false);
const blockListPanel = ref<SSHBlockListPanelInstance | null>(null);

const form = reactive({
  enabled: false,
  windowMinutes: 10,
  failedLoginThreshold: 5,
  blockDurationValue: 1,
  blockDurationUnit: "day" as "minute" | "hour" | "day",
  allowedRegions: [] as SSHSecuritySelection[],
  customCidrsText: "",
});

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
const customCidrsState = computed(() =>
  parseCidrTextarea(form.customCidrsText),
);
const invalidCustomCidrs = computed(() => customCidrsState.value.invalid);
const regionInputsDisabled = computed(() => isSaving.value || !form.enabled);
const {
  addRegion,
  canAddRegion,
  cityOptions,
  cityOptionsLoading,
  citySelectKey,
  citySelectPlaceholder,
  handleRegionDialogOpenChange,
  isRegionDialogOpen,
  openRegionDialog,
  regionDraft,
  removeRegion,
  selectionKey,
} = useSSHAllowedRegions({
  allowedRegions: toRef(form, "allowedRegions"),
  isEnabled: toRef(form, "enabled"),
  loadCities: (province) => CidrAPI.getCities(province),
  provinces,
  regionInputsDisabled,
  translate: (key, params) => (params ? t(key, params) : t(key)),
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

const reloadBlockList = () =>
  blockListPanel.value?.loadBlocks() ?? Promise.resolve();

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
      await Promise.all([loadDetails(), reloadBlockList()]);
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
      await Promise.all([loadDetails(), reloadBlockList()]);
    },
  });
};

onMounted(async () => {
  await loadDetails();
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

      <TabsContent value="login-logs">
        <SSHLoginLogsPanel />
      </TabsContent>

      <TabsContent value="blocks">
        <SSHBlockListPanel ref="blockListPanel" :reload-details="loadDetails" />
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
