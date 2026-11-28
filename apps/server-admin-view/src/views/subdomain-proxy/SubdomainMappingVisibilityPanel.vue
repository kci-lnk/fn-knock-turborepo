<script setup lang="ts">
import type { UnwrapNestedRefs } from "vue";
import { useI18n } from "vue-i18n";
import { ShieldCheck } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import MappingVisibilityFields from "@/components/MappingVisibilityFields.vue";
import type { HostMapping } from "@/types";
import type { useMappingVisibility } from "./useMappingVisibility";

defineProps<{
  composedPreviewHost: string;
  mappingForm: HostMapping;
  visibilityEditor: UnwrapNestedRefs<ReturnType<typeof useMappingVisibility>>;
}>();

const { t } = useI18n();
</script>

<template>
  <div class="space-y-4 pb-4 pt-4">
    <div class="rounded-lg border bg-muted/20 px-4 py-2.5">
      <p class="text-xs text-muted-foreground">Host</p>
      <p class="truncate text-sm font-medium">
        {{ composedPreviewHost || mappingForm.host || "-" }}
      </p>
    </div>

    <Alert class="border-primary/25 bg-primary/5">
      <ShieldCheck class="h-4 w-4" />
      <AlertTitle>
        {{ t("admin.subdomainProxy.visibilityPriorityAlertTitle") }}
      </AlertTitle>
      <AlertDescription class="leading-6">
        {{ t("admin.subdomainProxy.visibilityPriorityAlertDescription") }}
      </AlertDescription>
    </Alert>

    <MappingVisibilityFields
      v-model:mode="visibilityEditor.visibilityMode"
      v-model:selections="mappingForm.visibility.selections"
      v-model:custom-cidrs-text="visibilityEditor.customCidrsText"
      :behavior-description="
        t('admin.subdomainProxy.visibilityBehaviorDescription')
      "
      :regions-description="
        t('admin.subdomainProxy.visibilityRegionsDescription')
      "
      :cidrs-description="t('admin.subdomainProxy.visibilityCidrsDescription')"
      :validation-message="visibilityEditor.visibilityValidationMessage"
    />
  </div>
</template>
