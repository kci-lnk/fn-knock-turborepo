<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { GatewayLocationsPageController } from "./useGatewayLocationsPage";

const props = defineProps<{ controller: GatewayLocationsPageController }>();
const { t } = useI18n();
const {
  availableMappings,
  draftLocations,
  getMappingTitleForDisplay,
  isAvailable,
  openHostPicker,
  selectedMapping,
} = props.controller;
</script>

<template>
  <button
    type="button"
    class="grid w-full gap-4 rounded-md border border-border/60 bg-background px-5 py-4 text-left transition-colors hover:border-primary/30 hover:bg-muted/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60 sm:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)_minmax(0,1fr)_5rem] sm:items-center"
    :disabled="!isAvailable || availableMappings.length === 0"
    :aria-label="
      t('admin.gatewayLocationsSettings.switchHostAria', {
        host:
          selectedMapping?.host || t('admin.gatewayLocationsSettings.noHost'),
        title: getMappingTitleForDisplay(selectedMapping),
      })
    "
    @click="openHostPicker"
  >
    <span class="min-w-0 space-y-1">
      <span class="block text-xs font-medium text-muted-foreground">
        {{ t("admin.gatewayLocationsSettings.currentHost") }}
      </span>
      <span class="block truncate text-base font-semibold leading-6">
        {{ selectedMapping?.host || t("admin.gatewayLocationsSettings.noHost") }}
      </span>
      <span class="block truncate text-sm text-muted-foreground">
        {{
          availableMappings.length > 0
            ? t("admin.gatewayLocationsSettings.switchObject")
            : t("admin.gatewayLocationsSettings.createHostHint")
        }}
      </span>
    </span>

    <span
      class="min-w-0 space-y-1 border-t border-border/60 pt-3 sm:border-l sm:border-t-0 sm:pl-5 sm:pt-0"
    >
      <span class="block text-xs font-medium text-muted-foreground">
        {{ t("admin.gatewayLocationsSettings.siteTitle") }}
      </span>
      <span class="flex min-w-0 items-center gap-2">
        <span class="truncate text-sm font-medium">
          {{ getMappingTitleForDisplay(selectedMapping) }}
        </span>
      </span>
    </span>

    <span
      class="min-w-0 space-y-1 border-t border-border/60 pt-3 sm:border-l sm:border-t-0 sm:pl-5 sm:pt-0"
    >
      <span class="block text-xs font-medium text-muted-foreground">
        {{ t("admin.gatewayLocationsSettings.target") }}
      </span>
      <span class="block truncate text-sm font-medium">
        {{
          selectedMapping?.target ||
          t("admin.gatewayLocationsSettings.notSelected")
        }}
      </span>
    </span>

    <span
      class="space-y-1 border-t border-border/60 pt-3 sm:border-l sm:border-t-0 sm:pl-5 sm:pt-0 sm:text-right"
    >
      <span class="block text-xs font-medium text-muted-foreground">
        {{ t("admin.gatewayLocationsSettings.ruleCount") }}
      </span>
      <span class="block text-sm font-medium">{{ draftLocations.length }}</span>
    </span>
  </button>
</template>
