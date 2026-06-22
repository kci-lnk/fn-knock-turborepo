<script setup lang="ts">
import { computed, onMounted, reactive, ref, toRef } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRouter } from 'vue-router';
import { Plus, Shield } from 'lucide-vue-next';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import {
  TagsInput,
  TagsInputItem,
  TagsInputItemDelete,
  TagsInputItemText,
} from '@/components/ui/tags-input';
import { Textarea } from '@/components/ui/textarea';
import { Skeleton } from '@/components/ui/skeleton';
import { toast } from '@admin-shared/utils/toast';
import { parseCidrTextarea } from '@admin-shared/utils/cidr';
import { CidrAPI, ScannerAPI, type ScannerSettings } from '../../lib/api';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { useDelayedLoading } from '@admin-shared/composables/useDelayedLoading';
import type { CidrProvinceOption, GatewayVisibilitySelection } from '../../types';
import { useSSHAllowedRegions } from '../ssh-security/useSSHAllowedRegions';

const settings = ref<ScannerSettings | null>(null);
const provinces = ref<CidrProvinceOption[]>([]);
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
  cidrExemptionRegions: [] as GatewayVisibilitySelection[],
  cidrExemptionsText: '',
});

const scannerRegionTranslate = (
  key: string,
  params?: Record<string, string | number>,
) => {
  const keyMap: Record<string, string> = {
    'admin.sshSecurity.loading': 'admin.scannerFirewallSettings.loading',
    'admin.sshSecurity.selectProvinceFirst':
      'admin.scannerFirewallSettings.selectProvinceFirst',
    'admin.sshSecurity.selectCityOrProvince':
      'admin.scannerFirewallSettings.selectCityOrProvinceWide',
    'admin.sshSecurity.selectCity':
      'admin.scannerFirewallSettings.selectCity',
    'admin.sshSecurity.regionsLoadFailed':
      'admin.scannerFirewallSettings.regionsLoadFailed',
    'admin.sshSecurity.regionsLoadDescription':
      'admin.scannerFirewallSettings.regionsLoadDescription',
  };
  const resolvedKey = keyMap[key] ?? key;
  return params ? t(resolvedKey, params) : t(resolvedKey);
};

const derivedWindowMinutes = computed(() => Math.max(baseWindowMinutes, Number(form.windowMinutes) || 0));
const cidrExemptionsState = computed(() =>
  parseCidrTextarea(form.cidrExemptionsText),
);
const invalidCidrExemptions = computed(() => cidrExemptionsState.value.invalid);
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
  allowedRegions: toRef(form, 'cidrExemptionRegions'),
  isEnabled: toRef(form, 'enabled'),
  loadCities: (province) => CidrAPI.getCities(province),
  provinces,
  regionInputsDisabled,
  translate: scannerRegionTranslate,
});
const isDirty = computed(() => {
  if (!settings.value) return false;
  const compareDays = Math.ceil(settings.value.blacklistTtlSeconds / 86400);
  return (
    settings.value.enabled !== form.enabled ||
    settings.value.commonLocationExemptEnabled !==
      form.commonLocationExemptEnabled ||
    settings.value.windowMinutes !== Number(form.windowMinutes) ||
    settings.value.threshold !== Number(form.threshold) ||
    compareDays !== Number(form.blacklistTtlDays) ||
    JSON.stringify((settings.value.cidrExemptionRegions ?? []).map((item) => selectionKey(item))) !==
      JSON.stringify(form.cidrExemptionRegions.map((item) => selectionKey(item))) ||
    JSON.stringify(settings.value.cidrExemptions ?? []) !==
      JSON.stringify(cidrExemptionsState.value.cidrs)
  );
});
const saveBlockedReason = computed(() => {
  if (invalidCidrExemptions.value.length > 0) {
    return t('admin.scannerFirewallSettings.fixCidrExemptions');
  }
  return '';
});

const applyFromSettings = (data: ScannerSettings) => {
  settings.value = data;
  form.enabled = data.enabled;
  form.commonLocationExemptEnabled = data.commonLocationExemptEnabled === true;
  form.windowMinutes = data.windowMinutes;
  form.threshold = data.threshold;
  form.blacklistTtlDays = Math.max(1, Math.ceil(data.blacklistTtlSeconds / 86400));
  form.cidrExemptionRegions = (data.cidrExemptionRegions ?? []).map((item) => ({
    ...item,
  }));
  form.cidrExemptionsText = (data.cidrExemptions ?? []).join('\n');
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const [data, provincePayload] = await Promise.all([
      ScannerAPI.getSettings(),
      CidrAPI.getProvinces().catch((error) => {
        toast.error(t('admin.scannerFirewallSettings.regionsLoadFailed'), {
          description: extractErrorMessage(
            error,
            t('admin.scannerFirewallSettings.regionsLoadDescription'),
          ),
        });
        return null;
      }),
    ]);
    provinces.value = provincePayload?.options ?? [];
    applyFromSettings(data);
  });
};

const resetForm = () => {
  if (settings.value) applyFromSettings(settings.value);
};

const saveSettings = async () => {
  if (invalidCidrExemptions.value.length > 0) {
    toast.error(t('admin.scannerFirewallSettings.cidrValidationFailed'), {
      description: t('admin.scannerFirewallSettings.cidrExemptionsInvalid', {
        items: invalidCidrExemptions.value.join('、'),
      }),
    });
    return;
  }

  await runSaveSettings(
    () => {
      const payload = {
        enabled: form.enabled,
        commonLocationExemptEnabled: form.commonLocationExemptEnabled,
        windowMinutes: Math.max(1, Number(form.windowMinutes) || 1),
        threshold: Math.max(1, Number(form.threshold) || 1),
        blacklistTtlSeconds: Math.max(60, Math.floor((Number(form.blacklistTtlDays) || 1) * 86400)),
        cidrExemptionRegions: form.cidrExemptionRegions.map((item) => ({
          province: item.province,
          query_city: item.query_city,
        })),
        cidrExemptions: cidrExemptionsState.value.cidrs,
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

        <div class="flex flex-col p-6 gap-4">
          <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
            <div class="space-y-1">
              <Label class="text-base">
                {{ t('admin.scannerFirewallSettings.cidrExemptionRegionsTitle') }}
              </Label>
              <div class="text-sm text-muted-foreground">
                {{ t('admin.scannerFirewallSettings.cidrExemptionRegionsDescription') }}
              </div>
            </div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              :disabled="regionInputsDisabled || provinces.length === 0"
              @click="openRegionDialog"
            >
              <Plus class="h-4 w-4" />
              {{ t('admin.scannerFirewallSettings.addRegion') }}
            </Button>
          </div>

          <div class="rounded-xl bg-muted/20 px-4 py-4">
            <TagsInput
              :model-value="
                form.cidrExemptionRegions.map((item) => selectionKey(item))
              "
              class="min-h-0 items-start gap-2 border-none bg-transparent px-0 py-0 shadow-none"
            >
              <template v-if="form.cidrExemptionRegions.length > 0">
                <TagsInputItem
                  v-for="selection in form.cidrExemptionRegions"
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
                {{ t('admin.scannerFirewallSettings.noRegions') }}
              </span>
            </TagsInput>
          </div>
        </div>

        <div class="flex flex-col p-6 gap-4">
          <div class="space-y-1">
            <Label for="scanner-cidr-exemptions" class="text-base">
              {{ t('admin.scannerFirewallSettings.cidrExemptionsTitle') }}
            </Label>
            <div class="text-sm text-muted-foreground">
              {{ t('admin.scannerFirewallSettings.cidrExemptionsDescription') }}
            </div>
          </div>
          <div class="w-full space-y-2">
            <Textarea
              id="scanner-cidr-exemptions"
              v-model="form.cidrExemptionsText"
              class="min-h-32 font-mono text-sm"
              :placeholder="t('admin.scannerFirewallSettings.cidrExemptionsPlaceholder')"
              :disabled="isSaving"
            />
            <div class="flex flex-wrap gap-x-4 gap-y-2 text-sm">
              <span class="text-muted-foreground">
                {{
                  t('admin.scannerFirewallSettings.cidrExemptionsRecognized', {
                    count: cidrExemptionsState.cidrs.length,
                  })
                }}
              </span>
              <span
                v-if="invalidCidrExemptions.length > 0"
                class="text-destructive"
              >
                {{
                  t('admin.scannerFirewallSettings.cidrExemptionsInvalid', {
                    items: invalidCidrExemptions.join('、'),
                  })
                }}
              </span>
              <span v-else class="text-emerald-600">
                {{ t('admin.scannerFirewallSettings.cidrExemptionsValid') }}
              </span>
            </div>
          </div>
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
        <Button :disabled="!isDirty || isSaving || Boolean(saveBlockedReason)" @click="saveSettings">
          <span v-if="isSaving" class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
          {{ t('admin.scannerFirewallSettings.saveChanges') }}
        </Button>
      </div>
    </div>
  </Card>

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
            {{ t('admin.scannerFirewallSettings.addRegion') }}
          </DialogTitle>
          <DialogDescription class="text-sm leading-6 text-muted-foreground">
            {{ t('admin.scannerFirewallSettings.addRegionDescription') }}
          </DialogDescription>
        </DialogHeader>
      </div>

      <div class="space-y-4 border-t border-border/60 px-6 py-5">
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-2">
            <Label class="text-sm font-medium">
              {{ t('admin.scannerFirewallSettings.province') }}
            </Label>
            <Select v-model="regionDraft.province">
              <SelectTrigger
                class="h-11 w-full rounded-lg border-border/70 bg-background px-3 shadow-none"
                :disabled="regionInputsDisabled || provinces.length === 0"
              >
                <SelectValue
                  :placeholder="t('admin.scannerFirewallSettings.selectProvince')"
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
              {{ t('admin.scannerFirewallSettings.scope') }}
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
                <span
                  v-if="cityOptionsLoading"
                  class="h-4 w-4 animate-spin rounded-full border-2 border-muted-foreground/40 border-t-foreground"
                ></span>
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

      <DialogFooter class="border-t border-border/60 px-6 py-4 sm:justify-end">
        <Button
          variant="outline"
          @click="handleRegionDialogOpenChange(false)"
        >
          {{ t('common.cancel') }}
        </Button>
        <Button
          :disabled="!canAddRegion || isSaving"
          @click="addRegion"
        >
          {{ t('admin.scannerFirewallSettings.add') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
