<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import SessionDurationFieldRow from "../SessionDurationFieldRow.vue";
import type { SessionSettingsController } from "./useSessionSettingsController";

const props = defineProps<{
  controller: SessionSettingsController;
  mobilitySwitchId: string;
}>();
const { t } = useI18n();
const {
  effectiveSharedCookieDomain,
  form,
  grantModeSummary,
  incompatibleCookieScopeHosts,
  ipGrantDurationUnits,
  isDirectMode,
  isSaving,
  isSubdomainRoutingMode,
  mobilityWindowDurationUnits,
  postLoginIpGrantModeOptions,
  sessionIpMobilitySummary,
} = props.controller;
</script>

<template>
  <div class="space-y-4 p-6">
    <div
      v-if="isDirectMode"
      class="rounded-xl border border-border bg-muted/20 px-4 py-4"
    >
      <div class="text-sm font-medium text-foreground">
        {{ t("admin.sessionSettings.directModeTitle") }}
      </div>
      <div class="mt-1 text-sm leading-6 text-muted-foreground">
        {{ t("admin.sessionSettings.directModeDescription") }}
      </div>
    </div>

    <div class="space-y-1">
      <div class="text-base font-medium">
        {{ t("admin.sessionSettings.postLoginIpGrantMode") }}
      </div>
      <div class="text-sm text-muted-foreground">
        {{ t("admin.sessionSettings.postLoginIpGrantModeDescription") }}
      </div>
    </div>

    <div
      role="group"
      :aria-label="t('admin.sessionSettings.postLoginIpGrantMode')"
      class="grid gap-3 md:grid-cols-3"
    >
      <button
        v-for="option in postLoginIpGrantModeOptions"
        :key="option.value"
        type="button"
        class="rounded-xl border px-4 py-4 text-left transition-colors"
        :class="
          form.postLoginIpGrantMode === option.value
            ? 'border-primary bg-primary/5'
            : 'border-border bg-background hover:border-primary/40 hover:bg-muted/30'
        "
        :disabled="isSaving"
        :aria-pressed="form.postLoginIpGrantMode === option.value"
        @click="form.postLoginIpGrantMode = option.value"
      >
        <div class="text-sm font-medium text-foreground">
          {{ option.title }}
        </div>
        <div class="mt-1 text-sm leading-6 text-muted-foreground">
          {{ option.description }}
        </div>
      </button>
    </div>

    <SessionDurationFieldRow
      v-if="form.postLoginIpGrantMode === 'custom'"
      v-model="form.customGrant"
      :title="t('admin.sessionSettings.customGrantDuration')"
      :description="t('admin.sessionSettings.customGrantDurationDescription')"
      :units="ipGrantDurationUnits"
      :disabled="isSaving"
      framed
    />

    <div class="rounded-lg bg-muted/20 px-4 py-3 text-sm text-muted-foreground">
      {{ grantModeSummary }}
    </div>

    <div class="border-t border-border/60 pt-5">
      <div
        class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
      >
        <div class="space-y-1 pr-6">
          <Label :for="mobilitySwitchId" class="text-base">
            {{ t("admin.sessionSettings.sessionIpMobility") }}
          </Label>
          <div class="text-sm leading-6 text-muted-foreground">
            {{ t("admin.sessionSettings.sessionIpMobilityDescription") }}
          </div>
        </div>

        <Switch
          :id="mobilitySwitchId"
          class="shrink-0 sm:justify-self-end"
          :model-value="form.sessionIpMobilityEnabled"
          :disabled="isSaving"
          @update:model-value="form.sessionIpMobilityEnabled = $event === true"
        />
      </div>

      <SessionDurationFieldRow
        v-if="form.sessionIpMobilityEnabled"
        v-model="form.sessionIpMobilityWindow"
        class="mt-4"
        :title="t('admin.sessionSettings.ipRetentionTime')"
        :description="t('admin.sessionSettings.ipRetentionTimeDescription')"
        :units="mobilityWindowDurationUnits"
        :disabled="isSaving"
        framed
      />

      <div class="mt-3 text-sm leading-6 text-muted-foreground">
        {{ sessionIpMobilitySummary }}
      </div>
    </div>

    <div
      v-if="form.postLoginIpGrantMode === 'disabled' && isSubdomainRoutingMode"
      class="rounded-lg border border-border bg-muted/20 px-4 py-3 text-sm text-muted-foreground"
    >
      <template v-if="effectiveSharedCookieDomain">
        {{ t("admin.sessionSettings.sharedCookiePrefix") }}
        <code>{{ effectiveSharedCookieDomain }}</code>
        {{ t("admin.sessionSettings.sharedCookieSuffix") }}
        <template v-if="incompatibleCookieScopeHosts.length > 0">
          {{ t("admin.sessionSettings.incompatibleHostsPrefix") }}
          <code>{{
            incompatibleCookieScopeHosts.join(
              t("admin.sessionSettings.listSeparator"),
            )
          }}</code>
          {{ t("admin.sessionSettings.incompatibleHostsSuffix") }}
        </template>
        <template v-else>
          {{ t("admin.sessionSettings.allHostsCompatible") }}
        </template>
      </template>
      <template v-else>
        {{ t("admin.sessionSettings.noSharedCookieDomain") }}
      </template>
    </div>
  </div>
</template>
