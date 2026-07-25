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
import type { HostMappingGroup } from "@/types";

const props = defineProps<{
  disabled: boolean;
  groups: HostMappingGroup[];
  modelValue: string | null;
}>();
const emit = defineEmits<{
  "update:modelValue": [groupId: string | null];
}>();
const { t } = useI18n();
const model = computed({
  get: () => props.modelValue ?? "__ungrouped__",
  set: (value: string) =>
    emit("update:modelValue", value === "__ungrouped__" ? null : value),
});
</script>

<template>
  <div class="space-y-2">
    <Label for="mapping-group">{{ t("admin.subdomainProxy.groupName") }}</Label>
    <Select v-model="model" :disabled="disabled">
      <SelectTrigger id="mapping-group" class="w-full">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="__ungrouped__">
          {{ t("admin.subdomainProxy.ungrouped") }}
        </SelectItem>
        <SelectItem v-for="group in groups" :key="group.id" :value="group.id">
          {{ group.name }}
        </SelectItem>
      </SelectContent>
    </Select>
  </div>
</template>
