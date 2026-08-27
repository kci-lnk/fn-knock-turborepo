<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import ProxyTargetInputField from "@admin-shared/components/common/ProxyTargetInputField.vue";
import { useConfigStore } from "@/store/config";
import type { HostTargetPathMode } from "@/types";
import { useHostTargetCandidates } from "./useHostTargetCandidates";

const props = defineProps<{
  modelValue: string;
  targetPathMode: HostTargetPathMode;
  allowTargetPathMode: boolean;
  open: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
  "update:targetPathMode": [value: HostTargetPathMode];
}>();

const { t } = useI18n();
const configStore = useConfigStore();
const targetModel = computed({
  get: () => props.modelValue,
  set: (value: string) => emit("update:modelValue", value),
});
const targetPathModeModel = computed({
  get: () => props.targetPathMode || "entry",
  set: (value: string) =>
    emit("update:targetPathMode", value === "prefix" ? "prefix" : "entry"),
});
const hasTargetPath = computed(() => {
  try {
    const path = new URL(props.modelValue.trim()).pathname;
    return path !== "" && path !== "/";
  } catch {
    return false;
  }
});
const targetPathModeDescription = computed(() =>
  targetPathModeModel.value === "prefix"
    ? t("admin.subdomainProxy.targetPathModePrefixDescription")
    : t("admin.subdomainProxy.targetPathModeEntryDescription"),
);
const { targetCandidateHint, targetPlaceholder, targetSuggestions } =
  useHostTargetCandidates({
    isDockerDeployment: computed(() => configStore.isDockerDeployment),
    open: computed(() => props.open),
    translate: (key) => t(key),
  });
</script>

<template>
  <div class="space-y-2">
    <Label for="mapping-target">
      {{ t("admin.subdomainProxy.targetLabel") }}
    </Label>
    <ProxyTargetInputField
      v-model="targetModel"
      input-id="mapping-target"
      protocol-id="mapping-target-protocol"
      :placeholder="targetPlaceholder"
      :suggestions="targetSuggestions"
      :hint="t('admin.subdomainProxy.targetHint')"
    />
    <p v-if="targetCandidateHint" class="text-xs text-muted-foreground">
      {{ targetCandidateHint }}
    </p>
    <div
      v-if="allowTargetPathMode && hasTargetPath"
      class="space-y-2 rounded-lg border px-3 py-3"
    >
      <Label for="mapping-target-path-mode">
        {{ t("admin.subdomainProxy.targetPathMode") }}
      </Label>
      <Select v-model="targetPathModeModel">
        <SelectTrigger id="mapping-target-path-mode">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="entry">
            {{ t("admin.subdomainProxy.targetPathModeEntry") }}
          </SelectItem>
          <SelectItem value="prefix">
            {{ t("admin.subdomainProxy.targetPathModePrefix") }}
          </SelectItem>
        </SelectContent>
      </Select>
      <p class="text-xs leading-5 text-muted-foreground">
        {{ targetPathModeDescription }}
      </p>
    </div>
  </div>
</template>
