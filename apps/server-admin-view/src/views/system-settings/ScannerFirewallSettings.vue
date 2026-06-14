<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRouter } from 'vue-router';
import { Shield } from 'lucide-vue-next';
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
const { t } = useI18n();
const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t('admin.scannerFirewallSettings.loadFailed'), {
      description: extractErrorMessage(
        error,
        t('admin.scannerFirewallSettings.loadDescription'),
      ),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t('admin.scannerFirewallSettings.saveFailed'), {
      description: extractErrorMessage(
        error,
        t('admin.scannerFirewallSettings.saveDescription'),
      ),
    });
  },
});

const form = reactive({
  enabled: true,
  commonLocationExemptEnabled: false,
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
    settings.value.commonLocationExemptEnabled !==
      form.commonLocationExemptEnabled ||
    settings.value.windowMinutes !== Number(form.windowMinutes) ||
    settings.value.threshold !== Number(form.threshold) ||
    compareDays !== Number(form.blacklistTtlDays)
  );
});

const applyFromSettings = (data: ScannerSettings) => {
  settings.value = data;
  form.enabled = data.enabled;
  form.commonLocationExemptEnabled = data.commonLocationExemptEnabled === true;
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
        commonLocationExemptEnabled: form.commonLocationExemptEnabled,
        windowMinutes: Math.max(1, Number(form.windowMinutes) || 1),
        threshold: Math.max(1, Number(form.threshold) || 1),
        blacklistTtlSeconds: Math.max(60, Math.floor((Number(form.blacklistTtlDays) || 1) * 86400)),
      };
      return ScannerAPI.saveSettings(payload);
    },
    {
      onSuccess: (data) => {
        applyFromSettings(data);
        toast.success(t('admin.scannerFirewallSettings.updated'));
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
          <CardTitle class="text-md">
            {{ t('admin.scannerFirewallSettings.title') }}
          </CardTitle>
          <CardDescription class="mt-1.5">
            {{ t('admin.scannerFirewallSettings.description') }}
          </CardDescription>
        </div>
        <Button variant="secondary" size="sm" @click="goToBlacklist" class="shrink-0">
          <Shield class="mr-2 h-4 w-4" />
          {{ t('admin.scannerFirewallSettings.viewBlacklist') }}
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
            {{ t('admin.scannerFirewallSettings.enableTitle') }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{ t('admin.scannerFirewallSettings.enableDescription') }}
          </div>
        </div>
        <Switch v-model="form.enabled" />
      </div>

      <div v-show="form.enabled" class="divide-y animate-in fade-in slide-in-from-top-2 duration-300">
        <div class="flex items-center justify-between p-6 gap-4">
          <div class="space-y-1 pr-6">
            <Label class="text-base font-medium cursor-pointer" @click="form.commonLocationExemptEnabled = !form.commonLocationExemptEnabled">
              {{ t('admin.scannerFirewallSettings.commonLocationExemptTitle') }}
            </Label>
            <div class="text-sm text-muted-foreground">
              {{ t('admin.scannerFirewallSettings.commonLocationExemptDescription') }}
            </div>
          </div>
          <Switch v-model="form.commonLocationExemptEnabled" />
        </div>

        
        <div class="flex flex-col sm:flex-row sm:items-center justify-between p-6 gap-4">
          <div class="space-y-1 pr-6">
            <Label class="text-base">
              {{ t('admin.scannerFirewallSettings.windowTitle') }}
            </Label>
            <div class="text-sm text-muted-foreground">
              {{ t('admin.scannerFirewallSettings.windowDescription') }}
              <span v-if="derivedWindowMinutes > form.windowMinutes" class="text-destructive block sm:inline sm:ml-1">
                {{
                  t('admin.scannerFirewallSettings.enforcedMinimum', {
                    minutes: baseWindowMinutes,
                  })
                }}
              </span>
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input v-model.number="form.windowMinutes" type="number" min="1" class="w-24 text-center" />
            <span class="text-sm text-muted-foreground w-12">
              {{ t('admin.scannerFirewallSettings.minutesUnit') }}
            </span>
          </div>
        </div>

        <div class="flex flex-col sm:flex-row sm:items-center justify-between p-6 gap-4">
          <div class="space-y-1 pr-6">
            <Label class="text-base">
              {{ t('admin.scannerFirewallSettings.thresholdTitle') }}
            </Label>
            <div class="text-sm text-muted-foreground">
              {{ t('admin.scannerFirewallSettings.thresholdDescription') }}
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input v-model.number="form.threshold" type="number" min="1" class="w-24 text-center" />
            <span class="text-sm text-muted-foreground w-12">
              {{ t('admin.scannerFirewallSettings.timesUnit') }}
            </span>
          </div>
        </div>

        <div class="flex flex-col sm:flex-row sm:items-center justify-between p-6 gap-4">
          <div class="space-y-1 pr-6">
            <Label class="text-base">
              {{ t('admin.scannerFirewallSettings.blacklistTtlTitle') }}
            </Label>
            <div class="text-sm text-muted-foreground">
              {{ t('admin.scannerFirewallSettings.blacklistTtlDescription') }}
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Input v-model.number="form.blacklistTtlDays" type="number" min="1" class="w-24 text-center" />
            <span class="text-sm text-muted-foreground w-12">
              {{ t('admin.scannerFirewallSettings.daysUnit') }}
            </span>
          </div>
        </div>
      </div>
    </CardContent>

    <CardContent v-else class="min-h-[200px]" aria-hidden="true"></CardContent>

    <div class="flex items-center justify-between p-6 border-t bg-muted/20 rounded-b-xl">
      <div class="text-sm text-muted-foreground">
        <span v-if="isDirty">{{ t('admin.scannerFirewallSettings.dirty') }}</span>
        <span v-else>{{ t('admin.scannerFirewallSettings.clean') }}</span>
      </div>
      <div class="flex gap-3">
        <Button variant="ghost" @click="resetForm" :disabled="!isDirty || isSaving">
          {{ t('admin.scannerFirewallSettings.discard') }}
        </Button>
        <Button :disabled="!isDirty || isSaving" @click="saveSettings">
          <span v-if="isSaving" class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
          {{ t('admin.scannerFirewallSettings.saveChanges') }}
        </Button>
      </div>
    </div>
  </Card>
</template>
