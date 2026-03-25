<template>
  <Card data-guide-run-mode>
    <CardHeader>
      <CardTitle>运行模式设置</CardTitle>
      <CardDescription
        >按网络环境选择运行方式。直连适合有公网 IP
        的机器；反代适合内网穿透出去使用的用户</CardDescription
      >
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

      <div
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
            <p class="text-base font-semibold leading-none">直连模式</p>
            <span
              class="inline-flex items-center rounded-md border border-zinc-300 bg-white px-2 py-0.5 text-xs font-medium text-zinc-700"
            >
              适合有公网 IP
            </span>
          </div>
          <p class="text-sm text-muted-foreground">
            使用端口访问你的服务，比如 example.com:7999 登录后可以访问
            example.com:5666的飞牛服务。
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
            <p class="text-base font-semibold leading-none">反代模式</p>
            <span
              class="inline-flex items-center rounded-md border border-zinc-300 bg-white px-2 py-0.5 text-xs font-medium text-zinc-700"
            >
              内网穿透专用
            </span>
          </div>
          <p class="text-sm text-muted-foreground">
            没有公网IP，通过内网穿透转发请求。
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
              <p class="text-sm font-medium text-zinc-900">路径映射</p>
              <p class="mt-1 text-xs leading-5 text-muted-foreground">
                继续使用当前路径映射入口，可搭配 FRP 或 Cloudflared。
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
              <p class="text-sm font-medium text-zinc-900">子域映射</p>
              <p class="mt-1 text-xs leading-5 text-muted-foreground">
                隐藏路径映射并展示子域映射，且该子模式仅支持 FRP。
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
            <p class="text-base font-semibold leading-none">子域模式</p>
            <span
              class="inline-flex items-center rounded-md border border-zinc-300 bg-white px-2 py-0.5 text-xs font-medium text-zinc-700"
            >
              Host 网关
            </span>
          </div>
          <p class="text-sm text-muted-foreground">
            使用子域名访问你的服务，比如 fnos.example.com
            访问飞牛，不经过中转和内网穿透，直连
          </p>
          <DocsLinkButton :href="docsUrls.runModes.subdomain" @click.stop />
        </div>
      </div>
    </CardContent>
    <CardFooter class="flex justify-end gap-2">
      <DropdownMenu>
        <DropdownMenuTrigger as-child>
          <Button variant="outline" class="w-24 gap-2" :disabled="isBusy">
            <Loader2
              v-if="isFirewallActionPending"
              class="h-4 w-4 animate-spin"
            />
            <span>操作</span>
            <ChevronDown class="h-4 w-4 text-muted-foreground" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" class="w-56">
          <DropdownMenuItem
            :disabled="isBusy"
            @select="resetFirewallBySelectedMode"
          >
            <RefreshCw class="h-4 w-4" />
            按所选模式重设防火墙
          </DropdownMenuItem>
          <DropdownMenuItem
            variant="destructive"
            :disabled="isBusy"
            @select="clearFirewallRules"
          >
            <Trash2 class="h-4 w-4" />
            清空防火墙
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <Button variant="outline" class="w-24" @click="reset" :disabled="isBusy">
        放弃更改
      </Button>
      <Button
        data-guide-run-mode-save
        @click="save"
        :disabled="isBusy || isModeUnchanged"
      >
        <span
          v-if="isSaving"
          class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
        ></span>
        保存修改
      </Button>
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
            运行模式切换
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
          <span>不再提示</span>
        </label>
      </div>

      <DialogFooter class="border-t border-zinc-200 bg-zinc-50/60 px-8 py-4">
        <Button variant="outline" @click="isConfirmDialogOpen = false"
          >取消</Button
        >
        <Button @click="confirmSave" :disabled="isSaving">
          <span
            v-if="isSaving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          确认切换
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, watch } from "vue";
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
const DEFAULT_ROUTE_PLACEHOLDER = "/__select__";
const mode = ref<0 | 1 | 3>(1);
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
const { isPending: isSaving, run: runSaveMode } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "操作失败"),
    });
  },
});
const { isPending: isFirewallActionPending, run: runFirewallAction } =
  useAsyncAction({
    onError: (error) => {
      toast.error("防火墙操作失败", {
        description: extractErrorMessage(error, "操作失败"),
      });
    },
  });
const isBusy = computed(() => isSaving.value || isFirewallActionPending.value);
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
  reverseProxySubmode.value === "subdomain" ? "子域映射" : "路径映射",
);

const accessAlertTitle = computed(() => {
  if (mode.value === 0) return "直连模式访问说明";
  if (mode.value === 1) {
    return `反代模式 / ${selectedReverseProxySubmodeLabel.value}访问说明`;
  }
  return "子域名模式访问说明";
});

const accessAlertDescription = computed(() => {
  const port = accessEntry.value.port;
  if (mode.value === 0) {
    return `请让用户直接访问服务器的 ${port} 端口。`;
  }
  if (mode.value === 1) {
    if (reverseProxySubmode.value === "subdomain") {
      return `将 ${port} 端口通过 FRP 映射到外部入口，再通过不同子域名访问服务，网关会按 Host 转发到本地服务。`;
    }
    return `将 ${port} 端口通过反向代理或内网穿透映射到外部入口，从对外域名或入口地址访问。`;
  }
  return `将根域名及其子域名统一解析到 ${port} 端口，由网关按 Host 将请求转发到本地服务。`;
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
    reverseProxySubmode.value = savedReverseProxySubmode.value;
  }
  loadAccessEntry();
  loadRunModePromptPreferences();
});

watch(
  () => ({
    runType: configStore.config?.run_type,
    submode: configStore.config?.reverse_proxy_submode,
  }),
  ({ runType: nextMode }) => {
    if (nextMode !== undefined) {
      mode.value = nextMode;
      reverseProxySubmode.value = savedReverseProxySubmode.value;
    }
  },
);

function reset() {
  if (configStore.config) {
    mode.value = configStore.config.run_type;
    reverseProxySubmode.value = savedReverseProxySubmode.value;
  }
}

async function save() {
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
    toast.success("运行模式已更新", {
      description: successDescription,
    });
  });
}

async function resetFirewallBySelectedMode() {
  await runFirewallAction(async () => {
    const result = await SystemAPI.resetFirewallByRunType(mode.value);
    toast.success("防火墙已重设", {
      description: `${buildFirewallResetSuccessDescription(
        result,
        mode.value === 1 ? reverseProxySubmode.value : null,
      )}${buildUnsavedModeNotice()}`,
    });
  });
}

async function clearFirewallRules() {
  await runFirewallAction(async () => {
    const result = await SystemAPI.clearFirewall();
    toast.success("防火墙已清空", {
      description: `已清理 Linux 防火墙规则，并移除 ${result.gatewayPort} 端口相关的历史重定向。当前配置未变更，后续切换模式或同步运行配置时会重新写入规则。`,
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
    nextMode === 1 && nextSubmode === "subdomain"
      ? runningTunnels.filter((item) => item.key === "cloudflared")
      : nextMode === 1
        ? []
        : runningTunnels;

  if (tunnelsToStop.length === 0) return;

  await Promise.all(tunnelsToStop.map((item) => item.stop()));
  toast.success("已关闭隧道服务", {
    description: `${tunnelsToStop.map((item) => item.label).join("、")} 已停止，正在切换到${getRunModeLabel(nextMode, nextSubmode ?? undefined)}`,
  });
}

function getRunModeLabel(
  targetMode: 0 | 1 | 3,
  targetSubmode: ReverseProxySubmode = reverseProxySubmode.value,
) {
  if (targetMode === 0) return "直连模式";
  if (targetMode === 1) {
    return `反代模式 / ${targetSubmode === "subdomain" ? "子域映射" : "路径映射"}`;
  }
  return "子域模式";
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
      ? "已按反代模式 / 子域映射清理 Linux 防火墙规则。"
      : "已按反代模式 / 路径映射清理 Linux 防火墙规则。";
  }

  const exemptPortsLabel = result.exemptPorts.join("、");

  if (result.runType === 0) {
    const whitelistDescription =
      result.whitelistSynced > 0
        ? `，并同步 ${result.whitelistSynced} 条白名单 IP`
        : "，当前没有需要重新放行的白名单 IP";
    return `已按直连模式重建防火墙，仅保留 ${exemptPortsLabel} 端口作为登录入口${whitelistDescription}。`;
  }

  return `已按子域模式重建防火墙，保留 ${exemptPortsLabel} 端口供网关与协议映射使用。`;
}

function buildUnsavedModeNotice() {
  const currentMode = configStore.config?.run_type;
  const currentSubmode = savedReverseProxySubmode.value;
  if (currentMode === undefined) return "";
  const hasChanges =
    currentMode !== mode.value ||
    (mode.value === 1 && currentSubmode !== reverseProxySubmode.value);
  if (!hasChanges) return "";
  return ` 当前保存的运行模式仍是${getRunModeLabel(currentMode, currentSubmode)}，如需长期按${getRunModeLabel(mode.value, reverseProxySubmode.value)}生效，请点击“保存修改”。`;
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
      return `已清空 ${proxyMappingsCount.value} 条路径映射${hasCustomDefaultRoute.value ? "，并重置默认路径入口" : ""}。`;
    }
    return "已进入子域模式，路径映射入口已停用。";
  }

  if (nextMode === 1) {
    if (nextSubmode === "subdomain") {
      if (proxyMappingsCount.value > 0) {
        return `已切换到反代模式 / 子域映射，现有 ${proxyMappingsCount.value} 条路径映射会保留，但入口已隐藏，且仅保留 FRP。`;
      }
      return "已切换到反代模式 / 子域映射，路径映射入口已隐藏，且仅保留 FRP。";
    }

    const preservedItems: string[] = [];
    if (hostMappingsCount.value > 0) {
      preservedItems.push(`${hostMappingsCount.value} 条子域映射`);
    }
    if (streamMappingsCount.value > 0) {
      preservedItems.push(`${streamMappingsCount.value} 条协议映射`);
    }

    if (preservedItems.length > 0) {
      return `已切换到反代模式 / 路径映射，现有 ${preservedItems.join("、")} 会保留，但当前子模式下不显示。`;
    }

    return "已切换到反代模式 / 路径映射，可继续使用 FRP 或 Cloudflared。";
  }

  return "入口规则已按目标模式重新生效。";
}

function buildSubdomainResetMessage() {
  if (proxyMappingsCount.value === 0) {
    return "切换后不再使用“路径映射”，当前没有需要清理的路径规则。";
  }

  return `将清空现有 ${proxyMappingsCount.value} 条“路径映射”${hasCustomDefaultRoute.value ? "，并重置默认路径入口" : ""}。`;
}

function buildReverseProxyCompatibilityMessage(
  targetSubmode: ReverseProxySubmode,
) {
  if (targetSubmode === "subdomain") {
    if (proxyMappingsCount.value === 0) {
      return "路径映射入口会隐藏；当前没有需要额外说明的路径规则。";
    }
    return `现有 ${proxyMappingsCount.value} 条路径映射会继续保留，但在该子模式下不会显示。`;
  }

  const preservedItems: string[] = [];
  if (hostMappingsCount.value > 0) {
    preservedItems.push(`${hostMappingsCount.value} 条子域映射`);
  }
  if (streamMappingsCount.value > 0) {
    preservedItems.push(`${streamMappingsCount.value} 条协议映射`);
  }

  if (preservedItems.length === 0) {
    return "子域映射入口会隐藏，但当前没有需要额外说明的历史配置。";
  }

  return `现有 ${preservedItems.join("、")} 会继续保留，但在该子模式下不会显示。`;
}

const confirmDialogContent = computed(() => {
  const port = accessEntry.value.port;
  const targetSubmode = pendingSubmode.value ?? reverseProxySubmode.value;

  if (pendingPromptKey.value === "reverseProxyToDirect") {
    return {
      title: "切换到直连模式",
      description: "请确认你已经理解直连模式的访问方式和风险变化。",
      items: [
        `直连模式通过设置防火墙来实现，默认屏蔽所有端口，除 ${port}`,
        `${port} 端口会仅起到一个登录入口的作用，登录成功后对该 IP 开放所有端口`,
        "多入口，登录后可使用 5666 等端口访问飞牛等服务",
        "不会屏蔽局域网的访问",
        "不要在这个模式内网穿透",
      ],
    };
  }

  if (
    pendingPromptKey.value === "directToReverseProxy" ||
    pendingPromptKey.value === "subdomainToReverseProxy"
  ) {
    return {
      title: `切换到${getRunModeLabel(1, targetSubmode)}`,
      description:
        targetSubmode === "subdomain"
          ? "请确认你已经理解这个反代子模式会改为按子域名转发，并且只支持 FRP。"
          : "请确认你已经理解这个反代子模式会继续按路径访问，并且可以继续使用 FRP 或 Cloudflared。",
      items: [
        buildReverseProxyCompatibilityMessage(targetSubmode),
        "会清空 Linux 自带的防火墙配置",
        targetSubmode === "subdomain"
          ? `所有的入口都在 ${port}，通过 FRP 暴露到外部后，登录后按子域名访问不同服务`
          : `所有的入口都在 ${port}，可内网穿透本地 ${port} 到外部任意端口，任何访问都需要先登录`,
        targetSubmode === "subdomain"
          ? "路径映射入口会隐藏，改为显示子域映射，且不会影响现有的 run_type=3"
          : "登录后通过路径来访问子服务，FRP 与 Cloudflared 都可继续使用",
      ],
    };
  }

  if (pendingPromptKey.value === "switchToSubdomain") {
    return {
      title: "切换到子域名模式",
      description:
        "请确认你已经准备好根域名、认证子域名和业务子域名的解析与上游监听方式，切换时会清理路径映射配置。",
      items: [
        buildSubdomainResetMessage(),
        `所有入口仍通过 ${port} 暴露，但访问心智会从“路径映射”切换为“子域映射”`,
        "业务服务应尽量只监听 127.0.0.1，避免绕过网关直连",
        "推荐先配置 auth.example.com 作为统一登录入口，再逐步接入业务子域",
        "此模式默认不依赖 iptables，适合公网 Web 服务网关化保护",
      ],
    };
  }

  return {
    title: `切换到${getRunModeLabel(1, targetSubmode)}`,
    description: "请确认你已经理解反代模式会如何调整对外入口。",
    items: [
      buildReverseProxyCompatibilityMessage(targetSubmode),
      "集中入口访问",
      targetSubmode === "subdomain"
        ? `所有的入口都在 ${port}，通过 FRP 暴露到外部后，登录后按子域名访问不同服务`
        : `所有的入口都在 ${port}，可内网穿透本地 ${port} 到外部任意端口，任何访问都需要先登录`,
      targetSubmode === "subdomain"
        ? "该子模式仅支持 FRP，并且完全不影响现有的子域模式"
        : "登录后通过路径来访问子服务",
    ],
  };
});
</script>
