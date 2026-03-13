<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { Info } from 'lucide-vue-next';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Skeleton } from '@/components/ui/skeleton';
import { toast } from '@admin-shared/utils/toast';
import { SystemAPI } from '../../lib/api';
import type { FnosShareBypassConfig } from '../../types';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { useDelayedLoading } from '@admin-shared/composables/useDelayedLoading';
import { useConfigStore } from '../../store/config';

const configStore = useConfigStore();
const settings = ref<FnosShareBypassConfig | null>(null);
const form = reactive<FnosShareBypassConfig>({
  enabled: false,
  upstream_timeout_ms: 2500,
  validation_cache_ttl_seconds: 30,
  validation_lock_ttl_seconds: 5,
  session_ttl_seconds: 300,
});

const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error('加载失败', { description: extractErrorMessage(error, '无法获取飞牛设置') });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error('保存失败', { description: extractErrorMessage(error, '保存飞牛设置失败') });
  },
});
const isReverseProxyMode = computed(() => configStore.config?.run_type === 1);
const isRestrictedByRunMode = computed(() => configStore.config?.run_type === 0);

const isDirty = computed(() => {
  if (!settings.value) return false;
  return (
    settings.value.enabled !== form.enabled ||
    settings.value.upstream_timeout_ms !== Number(form.upstream_timeout_ms) ||
    settings.value.validation_cache_ttl_seconds !== Number(form.validation_cache_ttl_seconds) ||
    settings.value.validation_lock_ttl_seconds !== Number(form.validation_lock_ttl_seconds) ||
    settings.value.session_ttl_seconds !== Number(form.session_ttl_seconds)
  );
});

const normalizedTimeoutSeconds = computed(() =>
  Math.max(1, Math.round((Number(form.upstream_timeout_ms) || 0) / 100) / 10),
);

const toggleEnabled = () => {
  if (!isReverseProxyMode.value) return;
  form.enabled = !form.enabled;
};

const applyFromSettings = (data: FnosShareBypassConfig) => {
  settings.value = data;
  form.enabled = data.enabled;
  form.upstream_timeout_ms = data.upstream_timeout_ms;
  form.validation_cache_ttl_seconds = data.validation_cache_ttl_seconds;
  form.validation_lock_ttl_seconds = data.validation_lock_ttl_seconds;
  form.session_ttl_seconds = data.session_ttl_seconds;
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const data = await SystemAPI.getFnosShareBypassConfig();
    applyFromSettings(data);
  });
};

const resetForm = () => {
  if (settings.value) applyFromSettings(settings.value);
};

const saveSettings = async () => {
  if (!isReverseProxyMode.value) {
    toast.error('当前运行模式不可用', {
      description: '此功能只有在反代模式下可用。直连模式下，需要完成鉴权后才能开启对应端口供其他人访问。',
    });
    return;
  }

  await runSaveSettings(
    () => {
      const payload: FnosShareBypassConfig = {
        enabled: form.enabled,
        upstream_timeout_ms: Math.max(500, Math.floor(Number(form.upstream_timeout_ms) || 500)),
        validation_cache_ttl_seconds: Math.max(5, Math.floor(Number(form.validation_cache_ttl_seconds) || 5)),
        validation_lock_ttl_seconds: Math.max(1, Math.floor(Number(form.validation_lock_ttl_seconds) || 1)),
        session_ttl_seconds: Math.max(30, Math.floor(Number(form.session_ttl_seconds) || 30)),
      };
      return SystemAPI.updateFnosShareBypassConfig(payload);
    },
    {
      onSuccess: (data) => {
        applyFromSettings(data);
        toast.success('飞牛设置已更新');
      },
    },
  );
};

onMounted(fetchSettings);
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle class="text-md">飞牛分享直通</CardTitle>
      <CardDescription class="mt-1.5">
        你可以直接发送分享链接给别人，他们无需登录即可访问分享内容，但不会引发其他安全风险。只能在反代模式下使用
      </CardDescription>
    </CardHeader>

    <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="border-t p-0 divide-y">
      <div v-if="isRestrictedByRunMode" class="border-b border-zinc-200 bg-zinc-50/40 px-6 py-5">
        <Alert class="items-start rounded-xl border-zinc-200 bg-zinc-50/70 text-zinc-900 shadow-none">
          <Info class="mt-0.5 h-4 w-4 shrink-0" />
          <AlertTitle>当前为直连模式，此功能暂不可用</AlertTitle>
          <AlertDescription class="text-sm leading-6 text-zinc-700">
            此功能只有在反代模式下可用。在直连模式下，需要先完成鉴权，再开启对应端口供其他人访问。
          </AlertDescription>
        </Alert>
      </div>

      <div class="flex items-center justify-between bg-muted/10 p-6">
        <div class="space-y-1 pr-6">
          <Label
            class="text-base font-medium"
            :class="isReverseProxyMode ? 'cursor-pointer' : 'cursor-not-allowed text-zinc-500'"
            @click="toggleEnabled"
          >
            启用飞牛分享直通
          </Label>
          <div class="text-sm" :class="isReverseProxyMode ? 'text-muted-foreground' : 'text-zinc-500'">
            开启后，使分享变得简单安全
          </div>
        </div>
        <Switch v-model="form.enabled" :disabled="!isReverseProxyMode || isSaving" />
      </div>

      <div v-show="form.enabled" class="divide-y animate-in fade-in slide-in-from-top-2 duration-300">
        <div class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4">
          <div class="space-y-1 pr-6">
            <Label class="text-base">上游校验超时</Label>
            <div class="text-sm text-muted-foreground">
              访问分享入口时，等待飞牛上游页面返回校验结果的最长时间。
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input
              v-model.number="form.upstream_timeout_ms"
              type="number"
              min="500"
              step="100"
              class="w-28 text-center"
              :disabled="!isReverseProxyMode || isSaving"
            />
            <span class="w-16 text-sm text-muted-foreground">毫秒</span>
          </div>
          <div class="sm:col-span-2 -mt-1 text-xs text-muted-foreground">
            当前约 {{ normalizedTimeoutSeconds }} 秒，超时后会按未通过处理。
          </div>
        </div>

        <div class="flex flex-col justify-between gap-4 p-6 sm:flex-row sm:items-center">
          <div class="space-y-1 pr-6">
            <Label class="text-base">校验缓存时长</Label>
            <div class="text-sm text-muted-foreground">
              相同分享链接命中成功或失败结果后，重复请求会优先读取缓存，减少对上游的探测频率。
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input
              v-model.number="form.validation_cache_ttl_seconds"
              type="number"
              min="5"
              class="w-24 text-center"
              :disabled="!isReverseProxyMode || isSaving"
            />
            <span class="w-12 text-sm text-muted-foreground">秒</span>
          </div>
        </div>

        <div class="flex flex-col justify-between gap-4 p-6 sm:flex-row sm:items-center">
          <div class="space-y-1 pr-6">
            <Label class="text-base">校验锁时长</Label>
            <div class="text-sm text-muted-foreground">
              多个请求同时探测同一个分享时，使用短锁避免重复发起上游校验。
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input
              v-model.number="form.validation_lock_ttl_seconds"
              type="number"
              min="1"
              class="w-24 text-center"
              :disabled="!isReverseProxyMode || isSaving"
            />
            <span class="w-12 text-sm text-muted-foreground">秒</span>
          </div>
        </div>

        <div class="flex flex-col justify-between gap-4 p-6 sm:flex-row sm:items-center">
          <div class="space-y-1 pr-6">
            <Label class="text-base">分享会话时长</Label>
            <div class="text-sm text-muted-foreground">
              通过校验后生成的分享会话有效期。用于放行后续预览、缩略图和静态资源请求。
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input
              v-model.number="form.session_ttl_seconds"
              type="number"
              min="30"
              class="w-24 text-center"
              :disabled="!isReverseProxyMode || isSaving"
            />
            <span class="w-12 text-sm text-muted-foreground">秒</span>
          </div>
        </div>
      </div>
    </CardContent>

    <CardContent v-else class="min-h-[200px]" aria-hidden="true" />

    <div class="flex items-center justify-between rounded-b-xl border-t bg-muted/20 p-6">
      <div class="text-sm text-muted-foreground">
        <span v-if="isDirty">您有未保存的更改</span>
        <span v-else>所有设置已是最新状态</span>
      </div>
      <div class="flex gap-3">
        <Button variant="ghost" @click="resetForm" :disabled="!isDirty || isSaving || !isReverseProxyMode">放弃</Button>
        <Button :disabled="!isDirty || isSaving || !isReverseProxyMode" @click="saveSettings">
          <span v-if="isSaving" class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
          保存更改
        </Button>
      </div>
    </div>
  </Card>
</template>
