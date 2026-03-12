<template>
  <Card data-guide-run-mode>
    <CardHeader>
      <CardTitle>运行模式设置</CardTitle>
      <CardDescription>按网络环境选择运行方式。直连适合有公网 IP 的机器；反代适合内网穿透出去使用的用户</CardDescription>
    </CardHeader>
    <CardContent class="grid gap-6">
      <Alert class="items-start rounded-xl border-zinc-200 bg-zinc-50/70 text-zinc-900">
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
        :class="mode === 0 ? 'border-zinc-900 bg-zinc-50 ring-1 ring-zinc-900/10 shadow-sm' : 'border-zinc-200 hover:border-zinc-400'"
        @click="mode = 0"
      >
        <div class="mt-1 flex h-5 w-5 items-center justify-center rounded-full border shrink-0 transition-colors"
             :class="mode === 0 ? 'border-zinc-900' : 'border-zinc-400 group-hover:border-zinc-700'"
        >
          <div v-show="mode === 0" class="h-2.5 w-2.5 rounded-full bg-zinc-900" />
        </div>
        <div class="flex-1 space-y-2">
          <div class="flex items-center gap-2">
            <p class="text-base font-semibold leading-none">直连模式</p>
            <span class="inline-flex items-center rounded-md border border-zinc-300 bg-white px-2 py-0.5 text-xs font-medium text-zinc-700">
              适合有公网 IP
            </span>
          </div>
          <p class="text-sm text-muted-foreground">服务直接对外开放，仅允许白名单 IP 访问</p>
        </div>
      </div>

      <div
        class="group flex items-start space-x-4 rounded-lg border p-4 cursor-pointer transition-all hover:border-primary/50"
        :class="mode === 1 ? 'border-zinc-900 bg-zinc-50 ring-1 ring-zinc-900/10 shadow-sm' : 'border-zinc-200 hover:border-zinc-400'"
        @click="mode = 1"
      >
        <div class="mt-1 flex h-5 w-5 items-center justify-center rounded-full border shrink-0 transition-colors"
             :class="mode === 1 ? 'border-zinc-900' : 'border-zinc-400 group-hover:border-zinc-700'"
        >
          <div v-show="mode === 1" class="h-2.5 w-2.5 rounded-full bg-zinc-900" />
        </div>
        <div class="flex-1 space-y-2">
          <div class="flex items-center gap-2">
            <p class="text-base font-semibold leading-none">反代模式</p>
            <span class="inline-flex items-center rounded-md border border-zinc-300 bg-white px-2 py-0.5 text-xs font-medium text-zinc-700">
              内网穿透专用
            </span>
          </div>
          <p class="text-sm text-muted-foreground">没有公网IP，通过内网穿透转发请求。支持路径映射、转发与页面重写等增强能力。</p>
        </div>
      </div>
    </CardContent>
    <CardFooter class="flex justify-end gap-2">
      <Button variant="outline" @click="reset">放弃更改</Button>
      <Button data-guide-run-mode-save @click="save" :disabled="isSaving || configStore.config?.run_type === mode">
        <span v-if="isSaving" class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
        保存修改
      </Button>
    </CardFooter>
  </Card>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, watch } from 'vue';
import { Info } from 'lucide-vue-next';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { toast } from '@admin-shared/utils/toast';
import { useConfigStore } from '../../store/config';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { SystemAPI } from '../../lib/api';

const configStore = useConfigStore();
const mode = ref<0 | 1>(1);
const accessEntry = ref({
  port: '7999',
  env: 'GO_REPROXY_PORT' as const,
  isDefault: true,
});
const { isPending: isSaving, run: runSaveMode } = useAsyncAction({
  onError: (error) => {
    toast.error('保存失败', { description: extractErrorMessage(error, '操作失败') });
  },
});

const accessAlertTitle = computed(() => mode.value === 0 ? '直连模式访问说明' : '反代模式访问说明');

const accessAlertDescription = computed(() => {
  const port = accessEntry.value.port;
  return mode.value === 0
    ? `请让用户直接访问服务器的 ${port} 端口。`
    : `将 ${port} 端口通过反向代理或内网穿透映射到外部入口，从对外域名或入口地址访问。`;
});

onMounted(() => {
  if (configStore.config) {
    mode.value = configStore.config.run_type;
  }
  loadAccessEntry();
});

watch(() => configStore.config?.run_type, (newVal) => {
  if (newVal !== undefined) {
    mode.value = newVal;
  }
});

function reset() {
  if (configStore.config) {
    mode.value = configStore.config.run_type;
  }
}

async function save() {
  await runSaveMode(async () => {
    await configStore.setRunType(mode.value);
    toast.success('运行模式已更新');
  });
}

async function loadAccessEntry() {
  try {
    const info = await SystemAPI.getAccessEntry();
    accessEntry.value = info;
  } catch (error) {
    console.warn('load access entry failed:', error);
  }
}
</script>
