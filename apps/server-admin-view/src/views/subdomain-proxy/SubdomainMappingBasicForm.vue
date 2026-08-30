<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import SubdomainMappingAdvancedSettings from "./SubdomainMappingAdvancedSettings.vue";
import SubdomainMappingGroupField from "./SubdomainMappingGroupField.vue";
import SubdomainMappingIconEntry from "./SubdomainMappingIconEntry.vue";
import SubdomainMappingTargetEditor from "./SubdomainMappingTargetEditor.vue";
import type { SubdomainMappingDialogProps } from "./subdomain-mapping-dialog-contract";

const { dialog } = defineProps<{ dialog: SubdomainMappingDialogProps }>();
const { t } = useI18n();
const titleOverrideModel = computed({
  get: () => dialog.mappingForm.title_override,
  set: (value: string) => dialog.updateMappingForm({ title_override: value }),
});
const mappingSubdomainModel = computed({
  get: () => dialog.mappingSubdomain,
  set: (value: string) => dialog.setMappingSubdomain(value),
});
</script>

<template>
  <div class="grid gap-4 pb-4 pt-6">
    <div class="space-y-2">
      <div class="flex items-center justify-between gap-3">
        <Label for="mapping-display-title">
          {{ t("admin.subdomainProxy.displayTitle") }}
        </Label>
        <Button
          v-if="dialog.mappingForm.target_type === 'proxy'"
          variant="link"
          size="sm"
          data-affordance="edit"
          class="h-auto p-0 text-xs"
          :disabled="
            !dialog.canRefreshMappingMetadata ||
            dialog.isRefreshingMappingMetadata
          "
          @click="dialog.refreshMappingMetadata"
        >
          <RefreshCw
            v-if="dialog.isRefreshingMappingMetadata"
            class="mr-1 h-3.5 w-3.5 animate-spin"
          />
          {{
            dialog.isRefreshingMappingMetadata
              ? t("admin.subdomainProxy.refreshing")
              : t("admin.subdomainProxy.refreshTitle")
          }}
        </Button>
      </div>
      <Input
        id="mapping-display-title"
        v-model="titleOverrideModel"
        :placeholder="
          t(
            dialog.mappingForm.target_type === 'proxy'
              ? 'admin.subdomainProxy.titleAutoPlaceholder'
              : 'admin.subdomainProxy.staticServe.titlePlaceholder',
          )
        "
      />
      <p class="text-xs text-muted-foreground">
        {{
          t(
            dialog.mappingForm.target_type === "proxy"
              ? "admin.subdomainProxy.titleHelp"
              : "admin.subdomainProxy.staticServe.titleHint",
          )
        }}
        <span v-if="dialog.mappingResolvedTitle">
          {{
            t("admin.subdomainProxy.fetchedTitle", {
              title: dialog.mappingResolvedTitle,
            })
          }}
        </span>
        <span v-else-if="dialog.mappingForm.target.trim()">
          {{ t("admin.subdomainProxy.noFetchedTitle") }}
        </span>
      </p>
    </div>

    <SubdomainMappingIconEntry
      :icon-editor="dialog.iconEditor"
      :open-editor="dialog.visibilityEditor.openIconView"
    />

    <div class="space-y-2">
      <div
        class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between"
      >
        <div class="space-y-1">
          <Label for="mapping-subdomain">{{ dialog.mappingInputLabel }}</Label>
          <p class="text-xs text-muted-foreground">
            {{ dialog.mappingModeDescription }}
          </p>
        </div>
        <div
          role="group"
          :aria-label="t('admin.subdomainProxy.hostInputModeAria')"
          class="grid w-full grid-cols-2 rounded-lg bg-muted p-[3px] text-muted-foreground sm:w-[216px]"
        >
          <button
            type="button"
            :aria-pressed="dialog.mappingInputMode === 'subdomain'"
            :disabled="!dialog.canUseRootDomainSuffix"
            class="inline-flex h-8 items-center justify-center rounded-md px-2 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50"
            :class="
              dialog.mappingInputMode === 'subdomain'
                ? 'bg-background text-foreground shadow-sm'
                : 'hover:text-foreground'
            "
            @click="dialog.handleInputModeChange('subdomain')"
          >
            {{ t("admin.subdomainProxy.fixedSuffix") }}
          </button>
          <button
            type="button"
            :aria-pressed="dialog.mappingInputMode === 'full_host'"
            class="inline-flex h-8 items-center justify-center rounded-md px-2 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            :class="
              dialog.mappingInputMode === 'full_host'
                ? 'bg-background text-foreground shadow-sm'
                : 'hover:text-foreground'
            "
            @click="dialog.handleInputModeChange('full_host')"
          >
            {{ t("admin.subdomainProxy.fullHost") }}
          </button>
        </div>
      </div>
      <template v-if="dialog.mappingInputMode === 'subdomain'">
        <div class="flex items-stretch rounded-md border">
          <Input
            id="mapping-subdomain"
            v-model="mappingSubdomainModel"
            placeholder="redis"
            class="rounded-none border-0 shadow-none focus-visible:ring-0"
          />
          <div
            class="flex items-center border-l bg-muted/30 px-3 text-sm text-muted-foreground"
          >
            .{{ dialog.savedRootDomain }}
          </div>
        </div>
        <p class="text-xs text-muted-foreground">
          {{
            t("admin.subdomainProxy.finalHost", {
              host:
                dialog.composedPreviewHost ||
                t("admin.subdomainProxy.notFilled"),
            })
          }}
        </p>
      </template>
      <template v-else>
        <Input
          id="mapping-subdomain"
          v-model="mappingSubdomainModel"
          placeholder="auth.other-domain.example"
        />
        <p class="text-xs text-muted-foreground">
          {{ dialog.fullHostInputHint }}
        </p>
      </template>
    </div>

    <SubdomainMappingTargetEditor
      :mapping-form="dialog.mappingForm"
      :allow-target-path-mode="!dialog.isMappingAuthService"
      :open="dialog.open"
      :update-mapping-form="dialog.updateMappingForm"
    />
    <SubdomainMappingGroupField
      v-if="dialog.groups.length > 0 && !dialog.isMappingAuthService"
      :model-value="dialog.mappingForm.group_id"
      :groups="dialog.groups"
      :disabled="dialog.isSavingMappings"
      @update:model-value="dialog.updateMappingForm({ group_id: $event })"
    />
    <SubdomainMappingAdvancedSettings :dialog="dialog" />
  </div>
</template>
