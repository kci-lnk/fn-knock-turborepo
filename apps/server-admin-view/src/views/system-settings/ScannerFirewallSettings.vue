<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { useRouter } from 'vue-router';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Skeleton } from '@/components/ui/skeleton';
import { toast } from '@admin-shared/utils/toast';
import { ScannerAPI, type ScannerSettings } from '../../lib/api';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { useDelayedLoading } from '@admin-shared/composables/useDelayedLoading';

const settings = ref<ScannerSettings | null>(null);
const baseWindowMinutes = 5;
const router = useRouter();
const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error('加载失败', { description: extractErrorMessage(error, '无法获取配置') });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error('保存失败', { description: extractErrorMessage(error, '保存失败') });
  },
});

const form = reactive({
  enabled: true,
  windowMinutes: 5,
  threshold: 3,
  blacklistTtlDays: 90,
});

const derivedWindowMinutes = computed(() => Math.max(baseWindowMinutes, Number(form.windowMinutes) || 0));
const isDirty = computed(() => {
  if (!settings.value) return false;
  const compareDays = Math.ceil(settings.value.blacklistTtlSeconds / 86400);
  return (
    settings.value.enabled !== form.enabled ||
    settings.value.windowMinutes !== Number(form.windowMinutes) ||
    settings.value.threshold !== Number(form.threshold) ||
    compareDays !== Number(form.blacklistTtlDays)
  );
});

const applyFromSettings = (data: ScannerSettings) => {
  settings.value = data;
  form.enabled = data.enabled;
  form.windowMinutes = data.windowMinutes;
  form.threshold = data.threshold;
  form.blacklistTtlDays = Math.max(1, Math.ceil(data.blacklistTtlSeconds / 86400));
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const data = await ScannerAPI.getSettings();
    applyFromSettings(data);
  });
};

const resetForm = () => {
  if (settings.value) applyFromSettings(settings.value);
};

const saveSettings = async () => {
  await runSaveSettings(
    () => {
      const payload = {
        enabled: form.enabled,
        windowMinutes: Math.max(1, Number(form.windowMinutes) || 1),
        threshold: Math.max(1, Number(form.threshold) || 1),
        blacklistTtlSeconds: Math.max(60, Math.floor((Number(form.blacklistTtlDays) || 1) * 86400)),
      };
      return ScannerAPI.saveSettings(payload);
    },
    {
      onSuccess: (data) => {
        applyFromSettings(data);
        toast.success('扫描器防火墙设置已更新');
      },
    },
  );
};

onMounted(fetchSettings);

const goToBlacklist = () => {
  router.push({ path: '/sessions', query: { tab: 'ip-blacklist' } });
};
</script>
<template>
  <Card>
    <CardHeader>
      <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <CardTitle class="text-md">扫描器防火墙</CardTitle>
          <CardDescription class="mt-1.5">
            配置防扫描策略，自动识别并拦截恶意的路径嗅探和扫描行为。
          </CardDescription>
        </div>
        <Button variant="secondary" size="sm" @click="goToBlacklist" class="shrink-0">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="mr-2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10"></path></svg>
          查看黑名单
        </Button>
      </div>
    </CardHeader>

    <CardContent v-if="isLoading && showLoadingSkeleton" class="p-0 border-t">
      <div class="p-6 space-y-4">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="p-0 border-t divide-y">
      <div class="flex items-center justify-between p-6 bg-muted/10">
        <div class="space-y-1 pr-6">
          <Label class="text-base font-medium cursor-pointer" @click="form.enabled = !form.enabled">
            启用防御策略
          </Label>
          <div class="text-sm text-muted-foreground">
            开启后，系统将在后台监控访问频率，并在触发阈值时自动将恶意 IP 加入黑名单。
          </div>
        </div>
        <Switch v-model="form.enabled" />
      </div>

      <div v-show="form.enabled" class="divide-y animate-in fade-in slide-in-from-top-2 duration-300">
        
        <div class="flex flex-col sm:flex-row sm:items-center justify-between p-6 gap-4">
          <div class="space-y-1 pr-6">
            <Label class="text-base">统计窗口</Label>
            <div class="text-sm text-muted-foreground">
              系统计算异常访问次数的时间周期。
              <span v-if="derivedWindowMinutes > form.windowMinutes" class="text-destructive block sm:inline sm:ml-1">
                (系统强制下限为 {{ baseWindowMinutes }} 分钟)
              </span>
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input v-model.number="form.windowMinutes" type="number" min="1" class="w-24 text-center" />
            <span class="text-sm text-muted-foreground w-12">分钟</span>
          </div>
        </div>

        <div class="flex flex-col sm:flex-row sm:items-center justify-between p-6 gap-4">
          <div class="space-y-1 pr-6">
            <Label class="text-base">触发阈值</Label>
            <div class="text-sm text-muted-foreground">
              在一个统计窗口内，累计异常访问达到此次数将立即触发封禁。
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input v-model.number="form.threshold" type="number" min="1" class="w-24 text-center" />
            <span class="text-sm text-muted-foreground w-12">次</span>
          </div>
        </div>

        <div class="flex flex-col sm:flex-row sm:items-center justify-between p-6 gap-4">
          <div class="space-y-1 pr-6">
            <Label class="text-base">黑名单保留时长</Label>
            <div class="text-sm text-muted-foreground">
              攻击者 IP 被拦截后的封禁时间，到期后自动解封。
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input v-model.number="form.blacklistTtlDays" type="number" min="1" class="w-24 text-center" />
            <span class="text-sm text-muted-foreground w-12">天</span>
          </div>
        </div>
      </div>
    </CardContent>

    <CardContent v-else class="min-h-[200px]" aria-hidden="true"></CardContent>

    <div class="flex items-center justify-between p-6 border-t bg-muted/20 rounded-b-xl">
      <div class="text-sm text-muted-foreground">
        <span v-if="isDirty">您有未保存的更改</span>
        <span v-else>所有设置已是最新状态</span>
      </div>
      <div class="flex gap-3">
        <Button variant="ghost" @click="resetForm" :disabled="!isDirty || isSaving">放弃</Button>
        <Button :disabled="!isDirty || isSaving" @click="saveSettings">
          <span v-if="isSaving" class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
          保存更改
        </Button>
      </div>
    </div>
  </Card>
</template>
