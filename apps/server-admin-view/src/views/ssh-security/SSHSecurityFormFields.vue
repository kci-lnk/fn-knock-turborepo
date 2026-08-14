<script setup lang="ts">
import { useId } from "vue";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import CidrRegionSelector from "@/components/CidrRegionSelector.vue";
import type { SSHSecurityController } from "./ssh-security-contract";

const props = defineProps<{ controller: SSHSecurityController }>();
const a11yId = useId();
const {
  customCidrsState,
  details,
  form,
  invalidCustomCidrs,
  isSaving,
  regionInputsDisabled,
  t,
} = props.controller;
</script>

<template>
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
      <Label
        :for="`${a11yId}-sshsecurity-1`"
        class="text-sm font-medium"
      >
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
        :id="`${a11yId}-sshsecurity-1`"
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
        <SelectTrigger
          :aria-label="t('admin.sshSecurity.blockDuration')"
          :disabled="isSaving"
          ><SelectValue
        /></SelectTrigger>
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
          <SelectItem value="month">
            {{ t("admin.sshSecurity.month") }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>
  </div>

  <div
    class="grid gap-3 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
  >
    <div class="space-y-1">
      <div class="text-sm font-medium">
        {{ t("admin.sshSecurity.allowedRegions") }}
      </div>
    </div>
    <div class="w-full max-w-2xl space-y-3">
      <CidrRegionSelector
        v-model="form.allowedRegions"
        :disabled="regionInputsDisabled"
        :description="t('admin.sshSecurity.allowedRegionsDescription')"
        :text="{
          add: t('admin.gatewayVisibilitySettings.saveSelection'),
          addRegion: t('admin.gatewayVisibilitySettings.manageRegions'),
          cancel: t('common.cancel'),
          dialogDescription: t(
            'admin.sshSecurity.addRegionDescription',
          ),
          loadFailed: t('admin.sshSecurity.regionsLoadFailed'),
          loadFailedDescription: t(
            'admin.sshSecurity.regionsLoadDescription',
          ),
          loading: t('admin.sshSecurity.loading'),
          noRegions: t('admin.sshSecurity.noRegions'),
          province: t('admin.sshSecurity.province'),
          retry: t('admin.subdomainProxy.retry'),
          selectedCount: (count) =>
            t('admin.gatewayVisibilitySettings.selectedRegionCount', {
              count,
            }),
          scope: t('admin.sshSecurity.scope'),
          selectCity: t('admin.sshSecurity.selectCity'),
          selectProvince: t('admin.sshSecurity.selectProvince'),
          selectProvinceFirst: t(
            'admin.sshSecurity.selectProvinceFirst',
          ),
          unavailable: t(
            'admin.gatewayVisibilitySettings.unavailableSelection',
          ),
        }"
      />
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
