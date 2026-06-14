<template>
  <Card>
    <CardHeader>
      <CardTitle>{{ t("admin.runModeSettings.title") }}</CardTitle>
      <CardDescription>{{ t("admin.runModeSettings.description") }}</CardDescription>
    </CardHeader>
    <CardContent class="grid gap-6">
      <Alert
        class="items-start rounded-xl border-zinc-200 bg-zinc-50/70 text-zinc-900"
      >
        <Info class="mt-0.5 h-4 w-4" />
        <AlertTitle>{{ accessAlertTitle }}</AlertTitle>
        <AlertDescription>
          <div class="space-y-2 text-sm leading-6">
            <p>{{ accessAlertDescription }}</p>
          </div>
        </AlertDescription>
      </Alert>

      <Alert
        v-if="showHostFirewallUnavailableAlert"
        class="items-start rounded-xl border-zinc-200 bg-zinc-50/70 text-zinc-900"
      >
        <Info class="mt-0.5 h-4 w-4" />
        <AlertTitle>{{ t("admin.runModeSettings.hostFirewallUnavailableTitle") }}</AlertTitle>
        <AlertDescription>
          <div class="space-y-2 text-sm leading-6">
            <p>{{ hostFirewallUnavailableDescription }}</p>
          </div>
        </AlertDescription>
      </Alert>

      <div
        v-if="canUseDirectMode"
        class="group flex items-start space-x-4 rounded-lg border p-4 cursor-pointer transition-all hover:border-primary/50"
        :class="
          mode === 0
            ? 'border-zinc-900 bg-zinc-50 ring-1 ring-zinc-900/10 shadow-sm'
            : 'border-zinc-200 hover:border-zinc-400'
        "
        @click="mode = 0"
      >
        <div
          class="mt-1 flex h-5 w-5 items-center justify-center rounded-full border shrink-0 transition-colors"
          :class="
            mode === 0
              ? 'border-zinc-900'
              : 'border-zinc-400 group-hover:border-zinc-700'
          "
        >
          <div
            v-show="mode === 0"
            class="h-2.5 w-2.5 rounded-full bg-zinc-900"
          />
        </div>
        <div class="flex-1 space-y-2">
          <div class="flex items-center gap-2">
            <p class="text-base font-semibold leading-none">
              {{ t("admin.runModeSettings.directModeTitle") }}
            </p>
            <span
              class="inline-flex items-center rounded-md border border-zinc-300 bg-white px-2 py-0.5 text-xs font-medium text-zinc-700"
            >
              {{ t("admin.runModeSettings.directModeBadge") }}
            </span>
          </div>
          <p class="text-sm text-muted-foreground">
            {{ t("admin.runModeSettings.directModeDescription") }}
          </p>
          <DocsLinkButton :href="docsUrls.runModes.direct" @click.stop />
        </div>
      </div>

      <div
        class="group flex items-start space-x-4 rounded-lg border p-4 cursor-pointer transition-all hover:border-primary/50"
        :class="
          mode === 1
            ? 'border-zinc-900 bg-zinc-50 ring-1 ring-zinc-900/10 shadow-sm'
            : 'border-zinc-200 hover:border-zinc-400'
        "
        @click="mode = 1"
      >
        <div
          class="mt-1 flex h-5 w-5 items-center justify-center rounded-full border shrink-0 transition-colors"
          :class="
            mode === 1
              ? 'border-zinc-900'
              : 'border-zinc-400 group-hover:border-zinc-700'
          "
        >
          <div
            v-show="mode === 1"
            class="h-2.5 w-2.5 rounded-full bg-zinc-900"
          />
        </div>
        <div class="flex-1 space-y-2">
          <div class="flex items-center gap-2">
            <p class="text-base font-semibold leading-none">
              {{ t("admin.runModeSettings.reverseModeTitle") }}
            </p>
            <span
              class="inline-flex items-center rounded-md border border-zinc-300 bg-white px-2 py-0.5 text-xs font-medium text-zinc-700"
            >
              {{ t("admin.runModeSettings.reverseModeBadge") }}
            </span>
          </div>
          <p class="text-sm text-muted-foreground">
            {{ t("admin.runModeSettings.reverseModeDescription") }}
          </p>
          <DocsLinkButton :href="docsUrls.runModes.reverse" @click.stop />
          <div
            v-if="mode === 1"
            class="grid gap-3 pt-2 sm:grid-cols-2"
            @click.stop
          >
            <button
              type="button"
              class="rounded-lg border px-3 py-3 text-left transition-colors"
              :class="
                reverseProxySubmode === 'path'
                  ? 'border-zinc-900 bg-white shadow-sm'
                  : 'border-zinc-200 bg-white/80 hover:border-zinc-400'
              "
              @click="reverseProxySubmode = 'path'"
            >
              <p class="text-sm font-medium text-zinc-900">
                {{ t("admin.runModeSettings.pathMapping") }}
              </p>
              <p class="mt-1 text-xs leading-5 text-muted-foreground">
                {{ t("admin.runModeSettings.pathSubmodeDescription") }}
              </p>
            </button>
            <button
              type="button"
              class="rounded-lg border px-3 py-3 text-left transition-colors"
              :class="
                reverseProxySubmode === 'subdomain'
                  ? 'border-zinc-900 bg-white shadow-sm'
                  : 'border-zinc-200 bg-white/80 hover:border-zinc-400'
              "
              @click="reverseProxySubmode = 'subdomain'"
            >
              <p class="text-sm font-medium text-zinc-900">
                {{ t("admin.runModeSettings.subdomainMapping") }}
              </p>
              <p class="mt-1 text-xs leading-5 text-muted-foreground">
                {{ t("admin.runModeSettings.subdomainSubmodeDescription") }}
              </p>
            </button>
          </div>
        </div>
      </div>

      <div
        class="group flex items-start space-x-4 rounded-lg border p-4 cursor-pointer transition-all hover:border-primary/50"
        :class="
          mode === 3
            ? 'border-zinc-900 bg-zinc-50 ring-1 ring-zinc-900/10 shadow-sm'
            : 'border-zinc-200 hover:border-zinc-400'
        "
        @click="mode = 3"
      >
        <div
          class="mt-1 flex h-5 w-5 items-center justify-center rounded-full border shrink-0 transition-colors"
          :class="
            mode === 3
              ? 'border-zinc-900'
              : 'border-zinc-400 group-hover:border-zinc-700'
          "
        >
          <div
            v-show="mode === 3"
            class="h-2.5 w-2.5 rounded-full bg-zinc-900"
          />
        </div>
        <div class="flex-1 space-y-2">
          <div class="flex items-center gap-2">
            <p class="text-base font-semibold leading-none">
              {{ t("admin.runModeSettings.subdomainModeTitle") }}
            </p>
            <span
              class="inline-flex items-center rounded-md border border-zinc-300 bg-white px-2 py-0.5 text-xs font-medium text-zinc-700"
            >
              {{ t("admin.runModeSettings.subdomainModeBadge") }}
            </span>
          </div>
          <p class="text-sm text-muted-foreground">
            {{ t("admin.runModeSettings.subdomainModeDescription") }}
          </p>
          <DocsLinkButton :href="docsUrls.runModes.subdomain" @click.stop />
        </div>
      </div>
    </CardContent>
    <CardFooter
      class="flex flex-col gap-4 border-t border-zinc-200/80 pt-6 sm:flex-row sm:items-center sm:justify-between"
    >
      <label
        v-if="canManageHostFirewall"
        class="flex items-start gap-3 text-sm text-zinc-700"
      >
        <Checkbox
          class="mt-0.5"
          :model-value="autoManageFirewall"
          :disabled="isBusy"
          @update:model-value="handleAutoManageFirewallChange"
        />
        <span class="space-y-1">
          <span class="block font-medium text-zinc-900">
            {{ t("admin.runModeSettings.autoFirewallTitle") }}
          </span>
          <span class="block text-xs leading-5 text-muted-foreground">
            {{ t("admin.runModeSettings.autoFirewallDescription") }}
          </span>
        </span>
        <Loader2
          v-if="isAutoManageFirewallPending"
          class="mt-0.5 h-4 w-4 animate-spin text-muted-foreground"
        />
      </label>
      <div
        v-else-if="!configStore.isDockerDeployment"
        class="w-full text-sm leading-6 text-muted-foreground sm:max-w-xl"
      >
        {{ t("admin.runModeSettings.hostFirewallDisabled") }}
      </div>

      <div class="flex w-full justify-end gap-2 sm:w-auto">
        <DropdownMenu v-if="canManageHostFirewall">
          <DropdownMenuTrigger as-child>
            <Button variant="outline" class="w-24 gap-2" :disabled="isBusy">
              <Loader2
                v-if="isFirewallActionPending"
                class="h-4 w-4 animate-spin"
              />
              <span>{{ t("admin.runModeSettings.actions") }}</span>
              <ChevronDown class="h-4 w-4 text-muted-foreground" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-56">
            <DropdownMenuItem
              :disabled="isBusy"
              @select="resetFirewallBySelectedMode"
            >
              <RefreshCw class="h-4 w-4" />
              {{ t("admin.runModeSettings.resetFirewallByMode") }}
            </DropdownMenuItem>
            <DropdownMenuItem
              variant="destructive"
              :disabled="isBusy"
              @select="clearFirewallRules"
            >
              <Trash2 class="h-4 w-4" />
              {{ t("admin.runModeSettings.clearFirewall") }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <Button
          variant="outline"
          class="w-24"
          @click="reset"
          :disabled="isBusy"
        >
          {{ t("admin.runModeSettings.discardChanges") }}
        </Button>
        <Button @click="save" :disabled="isBusy || isModeUnchanged">
          <span
            v-if="isSaving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.runModeSettings.saveChanges") }}
        </Button>
      </div>
    </CardFooter>
  </Card>

  <Dialog
    :open="isConfirmDialogOpen"
    @update:open="handleConfirmDialogOpenChange"
  >
    <DialogContent
      class="overflow-hidden border-zinc-200 bg-white p-0 shadow-xl sm:max-w-[760px]"
    >
      <div class="px-8 pt-8 pb-6">
        <DialogHeader class="space-y-3 text-left">
          <p
            class="text-[11px] font-semibold uppercase tracking-[0.22em] text-zinc-500"
          >
            {{ t("admin.runModeSettings.switchEyebrow") }}
          </p>
          <DialogTitle
            class="text-2xl font-semibold tracking-tight text-zinc-950"
          >
            {{ confirmDialogContent.title }}
          </DialogTitle>
          <DialogDescription
            class="max-w-[56ch] text-sm leading-6 text-zinc-600"
          >
            {{ confirmDialogContent.description }}
          </DialogDescription>
        </DialogHeader>

        <ul class="mt-8 divide-y divide-zinc-200 border-y border-zinc-200">
          <li
            v-for="(item, index) in confirmDialogContent.items"
            :key="item"
            class="grid grid-cols-[auto_1fr] items-start gap-x-4 py-4"
          >
            <span
              class="pt-0.5 font-mono text-[11px] tracking-[0.18em] text-zinc-400"
            >
              {{ String(index + 1).padStart(2, "0") }}
            </span>
            <p class="text-sm leading-6 text-zinc-800">
              {{ item }}
            </p>
          </li>
        </ul>

        <label class="mt-6 flex items-center gap-3 text-sm text-zinc-600">
          <Checkbox
            :model-value="dontShowAgainChecked"
            @update:model-value="dontShowAgainChecked = $event === true"
          />
          <span>{{ t("admin.runModeSettings.dontShowAgain") }}</span>
        </label>
      </div>

      <DialogFooter class="border-t border-zinc-200 bg-zinc-50/60 px-8 py-4">
        <Button variant="outline" @click="isConfirmDialogOpen = false"
          >{{ t("common.cancel") }}</Button
        >
        <Button @click="confirmSave" :disabled="isSaving">
          <span
            v-if="isSaving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.runModeSettings.confirmSwitch") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, Info, Loader2, RefreshCw, Trash2 } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { toast } from "@admin-shared/utils/toast";
import { useConfigStore } from "../../store/config";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import {
  CloudflaredAPI,
  FrpcAPI,
  SystemAPI,
  type AccessEntryInfo,
  type RunModePromptPreferences,
} from "../../lib/api";
import { docsUrls } from "../../lib/docs";
import {
  DEFAULT_REVERSE_PROXY_SUBMODE,
  resolveReverseProxySubmode,
} from "../../lib/reverse-proxy-submode";
import type { ReverseProxySubmode } from "../../types";

const configStore = useConfigStore();
const { locale, t } = useI18n();
const DEFAULT_ROUTE_PLACEHOLDER = "/__select__";
const mode = ref<0 | 1 | 3>(1);
const autoManageFirewall = ref(true);
const reverseProxySubmode = ref<ReverseProxySubmode>(
  DEFAULT_REVERSE_PROXY_SUBMODE,
);
const pendingMode = ref<0 | 1 | 3 | null>(null);
const pendingSubmode = ref<ReverseProxySubmode | null>(null);
const pendingPromptKey = ref<keyof RunModePromptPreferences | null>(null);
const isConfirmDialogOpen = ref(false);
const dontShowAgainChecked = ref(false);
const runModePromptPreferences = ref<RunModePromptPreferences>({
  directToReverseProxy: false,
  reverseProxyToDirect: false,
  switchToSubdomain: false,
  subdomainToReverseProxy: false,
});
const accessEntry = ref<AccessEntryInfo>({
  port: "7999",
  env: "GO_REPROXY_PORT" as const,
  isDefault: true,
});
const formatInlineList = (items: string[]) =>
  items.join(locale.value === "en" ? ", " : "、");

const { isPending: isSaving, run: runSaveMode } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.runModeSettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.runModeSettings.operationFailed"),
      ),
    });
  },
});
const { isPending: isFirewallActionPending, run: runFirewallAction } =
  useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.runModeSettings.firewallActionFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.runModeSettings.operationFailed"),
        ),
      });
    },
  });
const {
  isPending: isAutoManageFirewallPending,
  run: runAutoManageFirewallUpdate,
} = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.runModeSettings.autoFirewallUpdateFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.runModeSettings.operationFailed"),
      ),
    });
  },
});
const isBusy = computed(
  () =>
    isSaving.value ||
    isFirewallActionPending.value ||
    isAutoManageFirewallPending.value,
);
const canUseDirectMode = computed(() => configStore.canUseDirectMode);
const canManageHostFirewall = computed(() => configStore.canManageHostFirewall);
const showHostFirewallUnavailableAlert = computed(
  () => !canManageHostFirewall.value && !configStore.isDockerDeployment,
);
const hostFirewallUnavailableDescription = computed(() => {
  if (configStore.isDockerDeployment) {
    return t("admin.runModeSettings.hostFirewallUnavailableDockerDescription");
  }

  return t("admin.runModeSettings.hostFirewallUnavailableDescription");
});
const savedReverseProxySubmode = computed(() =>
  resolveReverseProxySubmode(configStore.config),
);
const isModeUnchanged = computed(() => {
  const currentMode = configStore.config?.run_type;
  if (currentMode === undefined) return true;
  if (currentMode !== mode.value) return false;
  if (mode.value !== 1) return true;
  return savedReverseProxySubmode.value === reverseProxySubmode.value;
});
const selectedReverseProxySubmodeLabel = computed(() =>
  reverseProxySubmode.value === "subdomain"
    ? t("admin.runModeSettings.subdomainMapping")
    : t("admin.runModeSettings.pathMapping"),
);

const accessAlertTitle = computed(() => {
  if (mode.value === 0) return t("admin.runModeSettings.directAccessTitle");
  if (mode.value === 1) {
    return t("admin.runModeSettings.reverseAccessTitle", {
      submode: selectedReverseProxySubmodeLabel.value,
    });
  }
  return t("admin.runModeSettings.subdomainAccessTitle");
});

const accessAlertDescription = computed(() => {
  const port = accessEntry.value.port;
  if (mode.value === 0) {
    return t("admin.runModeSettings.directAccessDescription", { port });
  }
  if (mode.value === 1) {
    if (reverseProxySubmode.value === "subdomain") {
      return t("admin.runModeSettings.reverseSubdomainAccessDescription", {
        port,
      });
    }
    return t("admin.runModeSettings.reversePathAccessDescription", { port });
  }
  return t("admin.runModeSettings.subdomainAccessDescription", { port });
});

const proxyMappingsCount = computed(
  () => configStore.config?.proxy_mappings?.length ?? 0,
);
const hostMappingsCount = computed(
  () => configStore.config?.host_mappings?.length ?? 0,
);
const streamMappingsCount = computed(
  () => configStore.config?.stream_mappings?.length ?? 0,
);
const hasCustomDefaultRoute = computed(() => {
  const defaultRoute = configStore.config?.default_route?.trim() || "";
  return defaultRoute !== "" && defaultRoute !== DEFAULT_ROUTE_PLACEHOLDER;
});

onMounted(() => {
  if (configStore.config) {
    mode.value = configStore.config.run_type;
    autoManageFirewall.value =
      configStore.config.auto_manage_firewall !== false;
    reverseProxySubmode.value = savedReverseProxySubmode.value;
  }
  loadAccessEntry();
  loadRunModePromptPreferences();
});

watch(
  () => ({
    runType: configStore.config?.run_type,
    submode: configStore.config?.reverse_proxy_submode,
    autoManageFirewall: configStore.config?.auto_manage_firewall,
  }),
  (
    {
      runType: nextMode,
      submode: nextSubmode,
      autoManageFirewall: nextAutoManageFirewall,
    },
    previousState,
  ) => {
    const shouldSyncRunMode =
      nextMode !== undefined &&
      (nextMode !== previousState?.runType ||
        nextSubmode !== previousState?.submode);

    if (shouldSyncRunMode) {
      mode.value = nextMode;
      reverseProxySubmode.value = savedReverseProxySubmode.value;
    }
    autoManageFirewall.value = nextAutoManageFirewall !== false;

    if (!canUseDirectMode.value && mode.value === 0) {
      mode.value = nextMode === 0 ? 1 : (nextMode ?? 1);
    }
  },
);

function reset() {
  if (configStore.config) {
    mode.value = configStore.config.run_type;
    reverseProxySubmode.value = savedReverseProxySubmode.value;
  }
}

async function handleAutoManageFirewallChange(
  value: boolean | "indeterminate",
) {
  if (!canManageHostFirewall.value) return;
  if (isBusy.value) return;

  const nextValue = value === true;
  const previousValue = autoManageFirewall.value;

  if (nextValue === previousValue) return;

  autoManageFirewall.value = nextValue;
  await runAutoManageFirewallUpdate(async () => {
    try {
      const next = await configStore.saveAutoManageFirewall(nextValue);
      autoManageFirewall.value = next.auto_manage_firewall;
      toast.success(
        next.auto_manage_firewall
          ? t("admin.runModeSettings.autoFirewallEnabled")
          : t("admin.runModeSettings.autoFirewallDisabled"),
        {
          description: next.auto_manage_firewall
            ? t("admin.runModeSettings.autoFirewallEnabledDescription")
            : t("admin.runModeSettings.autoFirewallDisabledDescription"),
        },
      );
    } catch (error) {
      autoManageFirewall.value = previousValue;
      throw error;
    }
  });
}

async function save() {
  if (mode.value === 0 && !canUseDirectMode.value) {
    toast.error(t("admin.runModeSettings.directUnsupportedTitle"), {
      description: t("admin.runModeSettings.directUnsupportedDescription"),
    });
    return;
  }

  const currentMode = configStore.config?.run_type;
  const currentSubmode = savedReverseProxySubmode.value;
  if (
    currentMode === undefined ||
    (currentMode === mode.value &&
      (mode.value !== 1 || currentSubmode === reverseProxySubmode.value))
  ) {
    return;
  }

  const promptKey = getPromptPreferenceKey(currentMode, mode.value);
  if (promptKey && !runModePromptPreferences.value[promptKey]) {
    pendingMode.value = mode.value;
    pendingSubmode.value = mode.value === 1 ? reverseProxySubmode.value : null;
    pendingPromptKey.value = promptKey;
    dontShowAgainChecked.value = false;
    isConfirmDialogOpen.value = true;
    return;
  }

  await applyRunModeChange(
    mode.value,
    mode.value === 1 ? reverseProxySubmode.value : null,
  );
}

async function confirmSave() {
  if (pendingMode.value === null) return;
  const nextMode = pendingMode.value;
  const nextSubmode = pendingSubmode.value;

  await applyRunModeChange(nextMode, nextMode === 1 ? nextSubmode : null, {
    promptPreferenceKey: pendingPromptKey.value,
    disablePrompt: dontShowAgainChecked.value,
    onSuccess: () => {
      isConfirmDialogOpen.value = false;
      pendingMode.value = null;
      pendingSubmode.value = null;
      pendingPromptKey.value = null;
      dontShowAgainChecked.value = false;
    },
  });
}

async function loadAccessEntry() {
  try {
    const info = await SystemAPI.getAccessEntry();
    accessEntry.value = info;
  } catch (error) {
    console.warn("load access entry failed:", error);
  }
}

async function loadRunModePromptPreferences() {
  try {
    runModePromptPreferences.value =
      await SystemAPI.getRunModePromptPreferences();
  } catch (error) {
    console.warn("load run mode prompt preferences failed:", error);
  }
}

async function applyRunModeChange(
  nextMode: 0 | 1 | 3,
  nextSubmode: ReverseProxySubmode | null,
  options?: {
    promptPreferenceKey?: keyof RunModePromptPreferences | null;
    disablePrompt?: boolean;
    onSuccess?: () => void;
  },
) {
  await runSaveMode(async () => {
    const successDescription = buildRunModeChangeSuccessDescription(
      nextMode,
      nextSubmode,
    );

    await ensureTunnelsStoppedForTargetMode(nextMode, nextSubmode);

    if (options?.promptPreferenceKey && options.disablePrompt) {
      const nextPreferences = await SystemAPI.updateRunModePromptPreferences({
        [options.promptPreferenceKey]: true,
      });
      runModePromptPreferences.value = nextPreferences;
    }

    await configStore.setRunType(nextMode, nextSubmode ?? undefined);
    options?.onSuccess?.();
    toast.success(t("admin.runModeSettings.updated"), {
      description: successDescription,
    });
  });
}

async function resetFirewallBySelectedMode() {
  if (!canManageHostFirewall.value) {
    toast.error(t("admin.runModeSettings.firewallUnsupportedTitle"), {
      description: hostFirewallUnavailableDescription.value,
    });
    return;
  }

  await runFirewallAction(async () => {
    const result = await SystemAPI.resetFirewallByRunType(mode.value);
    toast.success(t("admin.runModeSettings.firewallReset"), {
      description: `${buildFirewallResetSuccessDescription(
        result,
        mode.value === 1 ? reverseProxySubmode.value : null,
      )}${buildUnsavedModeNotice()}`,
    });
  });
}

async function clearFirewallRules() {
  if (!canManageHostFirewall.value) {
    toast.error(t("admin.runModeSettings.firewallUnsupportedTitle"), {
      description: hostFirewallUnavailableDescription.value,
    });
    return;
  }

  await runFirewallAction(async () => {
    const result = await SystemAPI.clearFirewall();
    toast.success(t("admin.runModeSettings.firewallCleared"), {
      description: t("admin.runModeSettings.firewallClearedDescription", {
        port: result.gatewayPort,
      }),
    });
  });
}

async function ensureTunnelsStoppedForTargetMode(
  nextMode: 0 | 1 | 3,
  nextSubmode: ReverseProxySubmode | null,
) {
  const [frpcStatus, cloudflaredStatus] = await Promise.all([
    FrpcAPI.getStatus(),
    CloudflaredAPI.getStatus(),
  ]);

  const runningTunnels = [
    frpcStatus.running
      ? { key: "frp", label: "FRP", stop: () => FrpcAPI.stop() }
      : null,
    cloudflaredStatus.running
      ? {
          key: "cloudflared",
          label: "Cloudflared",
          stop: () => CloudflaredAPI.stop(),
        }
      : null,
  ].filter(
    (
      item,
    ): item is {
      key: "frp" | "cloudflared";
      label: string;
      stop: () => Promise<void>;
    } => item !== null,
  );
  const tunnelsToStop =
    nextMode === 1 ? [] : runningTunnels;

  if (tunnelsToStop.length === 0) return;

  await Promise.all(tunnelsToStop.map((item) => item.stop()));
  toast.success(t("admin.runModeSettings.tunnelsStopped"), {
    description: t("admin.runModeSettings.tunnelsStoppedDescription", {
      names: formatInlineList(tunnelsToStop.map((item) => item.label)),
      mode: getRunModeLabel(nextMode, nextSubmode ?? undefined),
    }),
  });
}

function getRunModeLabel(
  targetMode: 0 | 1 | 3,
  targetSubmode: ReverseProxySubmode = reverseProxySubmode.value,
) {
  if (targetMode === 0) return t("admin.runModeSettings.directModeName");
  if (targetMode === 1) {
    return t("admin.runModeSettings.reverseModeName", {
      submode:
        targetSubmode === "subdomain"
          ? t("admin.runModeSettings.subdomainMapping")
          : t("admin.runModeSettings.pathMapping"),
    });
  }
  return t("admin.runModeSettings.subdomainModeName");
}

function buildFirewallResetSuccessDescription(
  result: {
    runType: 0 | 1 | 3;
    gatewayPort: number;
    exemptPorts: string[];
    whitelistSynced: number;
  },
  selectedSubmode: ReverseProxySubmode | null,
) {
  if (result.runType === 1) {
    return selectedSubmode === "subdomain"
      ? t("admin.runModeSettings.firewallResetReverseSubdomain")
      : t("admin.runModeSettings.firewallResetReversePath");
  }

  const exemptPortsLabel = formatInlineList(result.exemptPorts);

  if (result.runType === 0) {
    const whitelistDescription =
      result.whitelistSynced > 0
        ? t("admin.runModeSettings.firewallResetDirectWhitelistSynced", {
            count: result.whitelistSynced,
          })
        : t("admin.runModeSettings.firewallResetDirectNoWhitelist");
    return t("admin.runModeSettings.firewallResetDirect", {
      ports: exemptPortsLabel,
      whitelist: whitelistDescription,
    });
  }

  return t("admin.runModeSettings.firewallResetSubdomain", {
    ports: exemptPortsLabel,
  });
}

function buildUnsavedModeNotice() {
  const currentMode = configStore.config?.run_type;
  const currentSubmode = savedReverseProxySubmode.value;
  if (currentMode === undefined) return "";
  const hasChanges =
    currentMode !== mode.value ||
    (mode.value === 1 && currentSubmode !== reverseProxySubmode.value);
  if (!hasChanges) return "";
  return t("admin.runModeSettings.unsavedModeNotice", {
    current: getRunModeLabel(currentMode, currentSubmode),
    target: getRunModeLabel(mode.value, reverseProxySubmode.value),
  });
}

function handleConfirmDialogOpenChange(nextOpen: boolean) {
  isConfirmDialogOpen.value = nextOpen;
  if (!nextOpen) {
    pendingMode.value = null;
    pendingSubmode.value = null;
    pendingPromptKey.value = null;
    dontShowAgainChecked.value = false;
  }
}

function getPromptPreferenceKey(
  currentMode: 0 | 1 | 3,
  nextMode: 0 | 1 | 3,
): keyof RunModePromptPreferences | null {
  if (currentMode === 0 && nextMode === 1) return "directToReverseProxy";
  if (currentMode === 1 && nextMode === 0) return "reverseProxyToDirect";
  if (nextMode === 3) return "switchToSubdomain";
  if (currentMode === 3 && nextMode === 1) return "subdomainToReverseProxy";
  return null;
}

function buildRunModeChangeSuccessDescription(
  nextMode: 0 | 1 | 3,
  nextSubmode: ReverseProxySubmode | null,
) {
  if (nextMode === 3) {
    if (proxyMappingsCount.value > 0) {
      return t("admin.runModeSettings.successSubdomainClearedMappings", {
        count: proxyMappingsCount.value,
        defaultRoute: hasCustomDefaultRoute.value
          ? t("admin.runModeSettings.successDefaultRouteReset")
          : "",
      });
    }
    return t("admin.runModeSettings.successSubdomainNoMappings");
  }

  if (nextMode === 1) {
    if (nextSubmode === "subdomain") {
      if (proxyMappingsCount.value > 0) {
        return t("admin.runModeSettings.successReverseSubdomainWithMappings", {
          count: proxyMappingsCount.value,
        });
      }
      return t("admin.runModeSettings.successReverseSubdomainNoMappings");
    }

    const preservedItems: string[] = [];
    if (hostMappingsCount.value > 0) {
      preservedItems.push(
        t("admin.runModeSettings.hostMappingsCount", {
          count: hostMappingsCount.value,
        }),
      );
    }
    if (streamMappingsCount.value > 0) {
      preservedItems.push(
        t("admin.runModeSettings.streamMappingsCount", {
          count: streamMappingsCount.value,
        }),
      );
    }

    if (preservedItems.length > 0) {
      return t("admin.runModeSettings.successReversePathWithPreserved", {
        items: formatInlineList(preservedItems),
      });
    }

    return t("admin.runModeSettings.successReversePathNoPreserved");
  }

  return t("admin.runModeSettings.successRulesApplied");
}

function buildSubdomainResetMessage() {
  if (proxyMappingsCount.value === 0) {
    return t("admin.runModeSettings.subdomainResetNoMappings");
  }

  return t("admin.runModeSettings.subdomainResetWithMappings", {
    count: proxyMappingsCount.value,
    defaultRoute: hasCustomDefaultRoute.value
      ? t("admin.runModeSettings.successDefaultRouteReset")
      : "",
  });
}

function buildReverseProxyCompatibilityMessage(
  targetSubmode: ReverseProxySubmode,
) {
  if (targetSubmode === "subdomain") {
    if (proxyMappingsCount.value === 0) {
      return t("admin.runModeSettings.compatReverseSubdomainNoMappings");
    }
    return t("admin.runModeSettings.compatReverseSubdomainWithMappings", {
      count: proxyMappingsCount.value,
    });
  }

  const preservedItems: string[] = [];
  if (hostMappingsCount.value > 0) {
    preservedItems.push(
      t("admin.runModeSettings.hostMappingsCount", {
        count: hostMappingsCount.value,
      }),
    );
  }
  if (streamMappingsCount.value > 0) {
    preservedItems.push(
      t("admin.runModeSettings.streamMappingsCount", {
        count: streamMappingsCount.value,
      }),
    );
  }

  if (preservedItems.length === 0) {
    return t("admin.runModeSettings.compatReversePathNoPreserved");
  }

  return t("admin.runModeSettings.compatReversePathWithPreserved", {
    items: formatInlineList(preservedItems),
  });
}

const confirmDialogContent = computed(() => {
  const port = accessEntry.value.port;
  const targetSubmode = pendingSubmode.value ?? reverseProxySubmode.value;

  if (pendingPromptKey.value === "reverseProxyToDirect") {
    return {
      title: t("admin.runModeSettings.promptDirectTitle"),
      description: t("admin.runModeSettings.promptDirectDescription"),
      items: [
        t("admin.runModeSettings.promptDirectItemFirewall", { port }),
        t("admin.runModeSettings.promptDirectItemLoginEntry", { port }),
        t("admin.runModeSettings.promptDirectItemMultiEntry"),
        t("admin.runModeSettings.promptDirectItemLan"),
        t("admin.runModeSettings.promptDirectItemNoTunnel"),
      ],
    };
  }

  if (
    pendingPromptKey.value === "directToReverseProxy" ||
    pendingPromptKey.value === "subdomainToReverseProxy"
  ) {
    return {
      title: t("admin.runModeSettings.promptSwitchTo", {
        mode: getRunModeLabel(1, targetSubmode),
      }),
      description:
        targetSubmode === "subdomain"
          ? t("admin.runModeSettings.promptReverseSubdomainDescription")
          : t("admin.runModeSettings.promptReversePathDescription"),
      items: [
        buildReverseProxyCompatibilityMessage(targetSubmode),
        t("admin.runModeSettings.promptReverseItemClearFirewall"),
        targetSubmode === "subdomain"
          ? t("admin.runModeSettings.promptReverseItemSubdomainEntry", { port })
          : t("admin.runModeSettings.promptReverseItemPathEntry", { port }),
        targetSubmode === "subdomain"
          ? t("admin.runModeSettings.promptReverseItemSubdomainUi")
          : t("admin.runModeSettings.promptReverseItemPathUi"),
      ],
    };
  }

  if (pendingPromptKey.value === "switchToSubdomain") {
    return {
      title: t("admin.runModeSettings.promptSubdomainTitle"),
      description: t("admin.runModeSettings.promptSubdomainDescription"),
      items: [
        buildSubdomainResetMessage(),
        t("admin.runModeSettings.promptSubdomainItemEntry", { port }),
        t("admin.runModeSettings.promptSubdomainItemBindLocal"),
        t("admin.runModeSettings.promptSubdomainItemAuth"),
        t("admin.runModeSettings.promptSubdomainItemIptables"),
      ],
    };
  }

  return {
    title: t("admin.runModeSettings.promptSwitchTo", {
      mode: getRunModeLabel(1, targetSubmode),
    }),
    description: t("admin.runModeSettings.promptReverseGenericDescription"),
    items: [
      buildReverseProxyCompatibilityMessage(targetSubmode),
      t("admin.runModeSettings.promptReverseItemCentralEntry"),
      targetSubmode === "subdomain"
        ? t("admin.runModeSettings.promptReverseItemSubdomainEntry", { port })
        : t("admin.runModeSettings.promptReverseItemPathEntry", { port }),
      targetSubmode === "subdomain"
        ? t("admin.runModeSettings.promptReverseItemSubdomainCompatible")
        : t("admin.runModeSettings.promptReverseItemPathServices"),
    ],
  };
});
</script>
