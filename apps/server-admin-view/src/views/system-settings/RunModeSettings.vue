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
            服务直接对外开放，仅允许白名单 IP 访问
          </p>
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
            没有公网IP，通过内网穿透转发请求。支持路径映射、转发与页面重写等增强能力。
          </p>
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
            <p class="text-base font-semibold leading-none">子域名模式</p>
            <span
              class="inline-flex items-center rounded-md border border-zinc-300 bg-white px-2 py-0.5 text-xs font-medium text-zinc-700"
            >
              Host 网关
            </span>
          </div>
          <p class="text-sm text-muted-foreground">
            使用 `auth.example.com` 与多个业务子域统一接入 Go 网关，不依赖
            iptables，适合公网 Web 服务入口保护。
          </p>
        </div>
      </div>
    </CardContent>
    <CardFooter class="flex justify-end gap-2">
      <Button variant="outline" @click="reset">放弃更改</Button>
      <Button
        data-guide-run-mode-save
        @click="save"
        :disabled="isSaving || configStore.config?.run_type === mode"
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
import { Info } from "lucide-vue-next";
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
  type RunModePromptPreferences,
} from "../../lib/api";

const configStore = useConfigStore();
const mode = ref<0 | 1 | 3>(1);
const pendingMode = ref<0 | 1 | 3 | null>(null);
const pendingPromptKey = ref<keyof RunModePromptPreferences | null>(null);
const isConfirmDialogOpen = ref(false);
const dontShowAgainChecked = ref(false);
const runModePromptPreferences = ref<RunModePromptPreferences>({
  directToReverseProxy: false,
  reverseProxyToDirect: false,
});
const accessEntry = ref({
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

const accessAlertTitle = computed(() => {
  if (mode.value === 0) return "直连模式访问说明";
  if (mode.value === 1) return "反代模式访问说明";
  return "子域名模式访问说明";
});

const accessAlertDescription = computed(() => {
  const port = accessEntry.value.port;
  if (mode.value === 0) {
    return `请让用户直接访问服务器的 ${port} 端口。`;
  }
  if (mode.value === 1) {
    return `将 ${port} 端口通过反向代理或内网穿透映射到外部入口，从对外域名或入口地址访问。`;
  }
  return `将根域名及其子域名统一解析到 ${port} 端口，由网关按 Host 将请求转发到本地服务。`;
});

onMounted(() => {
  if (configStore.config) {
    mode.value = configStore.config.run_type;
  }
  loadAccessEntry();
  loadRunModePromptPreferences();
});

watch(
  () => configStore.config?.run_type,
  (newVal) => {
    if (newVal !== undefined) {
      mode.value = newVal;
    }
  },
);

function reset() {
  if (configStore.config) {
    mode.value = configStore.config.run_type;
  }
}

async function save() {
  const currentMode = configStore.config?.run_type;
  if (currentMode === undefined || currentMode === mode.value) return;

  const promptKey = getPromptPreferenceKey(currentMode, mode.value);
  if (promptKey && !runModePromptPreferences.value[promptKey]) {
    pendingMode.value = mode.value;
    pendingPromptKey.value = promptKey;
    dontShowAgainChecked.value = false;
    isConfirmDialogOpen.value = true;
    return;
  }

  await applyRunModeChange(mode.value);
}

async function confirmSave() {
  if (pendingMode.value === null) return;
  const nextMode = pendingMode.value;

  await applyRunModeChange(nextMode, {
    promptPreferenceKey: pendingPromptKey.value,
    disablePrompt: dontShowAgainChecked.value,
    onSuccess: () => {
      isConfirmDialogOpen.value = false;
      pendingMode.value = null;
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
  options?: {
    promptPreferenceKey?: keyof RunModePromptPreferences | null;
    disablePrompt?: boolean;
    onSuccess?: () => void;
  },
) {
  await runSaveMode(async () => {
    if (nextMode === 0) {
      await ensureTunnelsStoppedForDirectMode();
    }

    if (options?.promptPreferenceKey && options.disablePrompt) {
      const nextPreferences = await SystemAPI.updateRunModePromptPreferences({
        [options.promptPreferenceKey]: true,
      });
      runModePromptPreferences.value = nextPreferences;
    }

    await configStore.setRunType(nextMode);
    options?.onSuccess?.();
    toast.success("运行模式已更新");
  });
}

async function ensureTunnelsStoppedForDirectMode() {
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

  if (runningTunnels.length === 0) return;

  await Promise.all(runningTunnels.map((item) => item.stop()));
  toast.success("已关闭隧道服务", {
    description: `${runningTunnels.map((item) => item.label).join("、")} 已停止，正在切换到直连模式`,
  });
}

function handleConfirmDialogOpenChange(nextOpen: boolean) {
  isConfirmDialogOpen.value = nextOpen;
  if (!nextOpen) {
    pendingMode.value = null;
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
  return null;
}

const confirmDialogContent = computed(() => {
  const port = accessEntry.value.port;

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

  if (pendingMode.value === 3) {
    return {
      title: "切换到子域名模式",
      description:
        "请确认你已经准备好根域名、认证子域名和业务子域名的解析与上游监听方式。",
      items: [
        `所有入口仍通过 ${port} 暴露，但访问心智从“路径映射”变为“子域映射”`,
        "业务服务应尽量只监听 127.0.0.1，避免绕过网关直连",
        "推荐先配置 auth.example.com 作为统一登录入口，再逐步接入业务子域",
        "此模式默认不依赖 iptables，适合公网 Web 服务网关化保护",
      ],
    };
  }

  return {
    title: "切换到反代模式",
    description: "请确认你已经理解反代模式会如何调整对外入口。",
    items: [
      "集中入口访问",
      "会清空 Linux 自带的防火墙配置",
      `所有的入口都在 ${port}，可内网穿透本地 ${port} 到外部任意端口，任何访问都需要先登录`,
      "登录后通过路径来访问子服务",
    ],
  };
});
</script>
