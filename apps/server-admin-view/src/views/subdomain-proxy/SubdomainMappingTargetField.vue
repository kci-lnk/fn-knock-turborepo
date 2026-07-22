<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Label } from "@/components/ui/label";
import ProxyTargetInputField from "@admin-shared/components/common/ProxyTargetInputField.vue";
import { useConfigStore } from "@/store/config";
import { useDockerHostTargetCandidates } from "./useDockerHostTargetCandidates";

const props = defineProps<{
  modelValue: string;
  open: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const { t } = useI18n();
const configStore = useConfigStore();
const targetModel = computed({
  get: () => props.modelValue,
  set: (value: string) => emit("update:modelValue", value),
});
const { targetCandidateHint, targetPlaceholder, targetSuggestions } =
  useDockerHostTargetCandidates({
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
    />
    <p v-if="targetCandidateHint" class="text-xs text-muted-foreground">
      {{ targetCandidateHint }}
    </p>
  </div>
</template>
