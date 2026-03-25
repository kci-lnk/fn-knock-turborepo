<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Skeleton } from "@/components/ui/skeleton";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "../../lib/api";
import type { GatewaySettings } from "../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { useConfigStore } from "../../store/config";

const configStore = useConfigStore();
const settings = ref<GatewaySettings | null>(null);
const form = reactive<GatewaySettings>({
  auth_cache_ttl_seconds: 1,
  auth_cache_unauthorized_ttl_seconds: 1,
  reverse_proxy_throttle: {
    enabled: true,
    requests_per_second: 100,
    burst: 200,
    block_seconds: 30,
  },
});

const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error("加载失败", {
      description: extractErrorMessage(error, "无法获取网关设置"),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "保存网关设置失败"),
    });
  },
});

const clampCacheTtl = (value: unknown) =>
  Math.max(0, Math.floor(Number(value) || 0));

const clampPositiveInt = (value: unknown, fallback = 1) =>
  Math.max(1, Math.floor(Number(value) || fallback));

const isDirty = computed(() => {
  if (!settings.value) return false;
  return (
    settings.value.auth_cache_ttl_seconds !==
      Number(form.auth_cache_ttl_seconds) ||
    settings.value.auth_cache_unauthorized_ttl_seconds !==
      Number(form.auth_cache_unauthorized_ttl_seconds) ||
    settings.value.reverse_proxy_throttle.enabled !==
      form.reverse_proxy_throttle.enabled ||
    settings.value.reverse_proxy_throttle.requests_per_second !==
      Number(form.reverse_proxy_throttle.requests_per_second) ||
    settings.value.reverse_proxy_throttle.burst !==
      Number(form.reverse_proxy_throttle.burst) ||
    settings.value.reverse_proxy_throttle.block_seconds !==
      Number(form.reverse_proxy_throttle.block_seconds)
  );
});

const authCacheHint = computed(() =>
  Number(form.auth_cache_ttl_seconds) === 0
    ? "成功鉴权结果缓存已关闭，每次请求都会实时校验。"
    : `成功鉴权结果会缓存 ${clampCacheTtl(form.auth_cache_ttl_seconds)} 秒。`,
);

const authCacheFailHint = computed(() =>
  Number(form.auth_cache_unauthorized_ttl_seconds) === 0
    ? "未通过鉴权结果缓存已关闭，拒绝请求不会复用失败缓存。"
    : `未通过鉴权结果会缓存 ${clampCacheTtl(form.auth_cache_unauthorized_ttl_seconds)} 秒。`,
);

const toggleThrottleEnabled = () => {
  form.reverse_proxy_throttle.enabled = !form.reverse_proxy_throttle.enabled;
};

const applyFromSettings = (data: GatewaySettings) => {
  settings.value = {
    ...data,
    reverse_proxy_throttle: { ...data.reverse_proxy_throttle },
  };
  form.auth_cache_ttl_seconds = data.auth_cache_ttl_seconds;
  form.auth_cache_unauthorized_ttl_seconds =
    data.auth_cache_unauthorized_ttl_seconds;
  form.reverse_proxy_throttle.enabled = data.reverse_proxy_throttle.enabled;
  form.reverse_proxy_throttle.requests_per_second =
    data.reverse_proxy_throttle.requests_per_second;
  form.reverse_proxy_throttle.burst = data.reverse_proxy_throttle.burst;
  form.reverse_proxy_throttle.block_seconds =
    data.reverse_proxy_throttle.block_seconds;
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const data = await ConfigAPI.getGatewaySettings();
    applyFromSettings(data);
  });
};

const resetForm = () => {
  if (settings.value) applyFromSettings(settings.value);
};

const saveSettings = async () => {
  await runSaveSettings(
    () =>
      ConfigAPI.updateGatewaySettings({
        auth_cache_ttl_seconds: clampCacheTtl(form.auth_cache_ttl_seconds),
        auth_cache_unauthorized_ttl_seconds: clampCacheTtl(
          form.auth_cache_unauthorized_ttl_seconds,
        ),
        reverse_proxy_throttle: {
          enabled: form.reverse_proxy_throttle.enabled,
          requests_per_second: clampPositiveInt(
            form.reverse_proxy_throttle.requests_per_second,
            100,
          ),
          burst: clampPositiveInt(form.reverse_proxy_throttle.burst, 200),
          block_seconds: clampPositiveInt(
            form.reverse_proxy_throttle.block_seconds,
            30,
          ),
        },
      }),
    {
      onSuccess: async (data) => {
        applyFromSettings(data);
        await configStore.loadConfig();
        toast.success("网关设置已更新");
      },
    },
  );
};

onMounted(fetchSettings);
</script>

<template>
  <Card>
    <CardHeader>
      <div class="flex items-start justify-between gap-3">
        <div class="space-y-1.5">
          <CardTitle class="text-md">网关</CardTitle>
          <CardDescription>
            这里的配置会直接保存到管理端配置，并立即下发到 Go
            网关，包含鉴权结果缓存与反向代理节流策略。
          </CardDescription>
        </div>
      </div>
    </CardHeader>

    <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="border-t p-0 divide-y">
      <div
        class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
      >
        <div class="space-y-1 pr-6">
          <Label class="text-base">成功鉴权缓存时长</Label>
          <div class="text-sm text-muted-foreground">
            对同一客户端和同一鉴权目标，成功结果缓存多少秒。填
            <code>0</code> 表示关闭成功缓存。
          </div>
        </div>
        <div class="flex items-center gap-2 shrink-0">
          <Input
            v-model.number="form.auth_cache_ttl_seconds"
            type="number"
            min="0"
            step="1"
            class="w-24 text-center"
            :disabled="isSaving"
          />
          <span class="w-12 text-sm text-muted-foreground">秒</span>
        </div>
        <div class="sm:col-span-2 -mt-1 text-xs text-muted-foreground">
          {{ authCacheHint }}
        </div>
      </div>

      <div
        class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
      >
        <div class="space-y-1 pr-6">
          <Label class="text-base">失败鉴权缓存时长</Label>
          <div class="text-sm text-muted-foreground">
            未通过鉴权时，拒绝结果缓存多少秒。填
            <code>0</code> 表示关闭失败缓存。
          </div>
        </div>
        <div class="flex items-center gap-2 shrink-0">
          <Input
            v-model.number="form.auth_cache_unauthorized_ttl_seconds"
            type="number"
            min="0"
            step="1"
            class="w-24 text-center"
            :disabled="isSaving"
          />
          <span class="w-12 text-sm text-muted-foreground">秒</span>
        </div>
        <div class="sm:col-span-2 -mt-1 text-xs text-muted-foreground">
          {{ authCacheFailHint }}
        </div>
      </div>

      <div class="flex items-center justify-between bg-muted/10 p-6">
        <div class="space-y-1 pr-6">
          <Label
            class="cursor-pointer text-base font-medium"
            @click="toggleThrottleEnabled"
          >
            启用网关反代节流
          </Label>
          <div class="text-sm text-muted-foreground">
            按客户端 IP
            做限速与短时封禁，适合拦住高频探测、错误重试风暴和异常刷请求。
          </div>
        </div>
        <Switch
          v-model="form.reverse_proxy_throttle.enabled"
          :disabled="isSaving"
        />
      </div>

      <div
        v-show="form.reverse_proxy_throttle.enabled"
        class="divide-y animate-in fade-in slide-in-from-top-2 duration-300"
      >
        <div
          class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
        >
          <div class="space-y-1 pr-6">
            <Label class="text-base">每秒请求数</Label>
            <div class="text-sm text-muted-foreground">
              每个客户端 IP 每秒允许通过的请求数，超过后会消耗突发额度。
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input
              v-model.number="form.reverse_proxy_throttle.requests_per_second"
              type="number"
              min="1"
              step="1"
              class="w-24 text-center"
              :disabled="isSaving"
            />
            <span class="w-16 text-sm text-muted-foreground">req/s</span>
          </div>
        </div>

        <div
          class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
        >
          <div class="space-y-1 pr-6">
            <Label class="text-base">突发额度</Label>
            <div class="text-sm text-muted-foreground">
              短时间突增流量时可以额外放行的令牌数，适合容忍页面并发资源请求。
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input
              v-model.number="form.reverse_proxy_throttle.burst"
              type="number"
              min="1"
              step="1"
              class="w-24 text-center"
              :disabled="isSaving"
            />
            <span class="w-16 text-sm text-muted-foreground">tokens</span>
          </div>
        </div>

        <div
          class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
        >
          <div class="space-y-1 pr-6">
            <Label class="text-base">超限封禁时长</Label>
            <div class="text-sm text-muted-foreground">
              超出额度后，网关会直接断开连接并持续封禁这段时间；被中断的请求不会写入
              access log。
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input
              v-model.number="form.reverse_proxy_throttle.block_seconds"
              type="number"
              min="1"
              step="1"
              class="w-24 text-center"
              :disabled="isSaving"
            />
            <span class="w-12 text-sm text-muted-foreground">秒</span>
          </div>
        </div>
      </div>

      <div class="flex items-center justify-end gap-3 p-6">
        <Button
          variant="outline"
          :disabled="!isDirty || isSaving"
          @click="resetForm"
        >
          重置
        </Button>
        <Button :disabled="!isDirty || isSaving" @click="saveSettings">
          保存设置
        </Button>
      </div>
    </CardContent>
  </Card>
</template>
