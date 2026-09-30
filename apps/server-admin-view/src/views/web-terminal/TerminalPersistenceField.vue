<script setup lang="ts">
import { useId } from "vue";
import { useI18n } from "vue-i18n";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";

defineProps<{ disabled: boolean }>();
const persistent = defineModel<boolean>({ required: true });
const { t } = useI18n();
const fieldId = useId();
</script>

<template>
  <div class="flex items-start gap-3">
    <Checkbox
      :id="fieldId"
      :model-value="persistent"
      :disabled="disabled"
      :aria-describedby="`${fieldId}-help`"
      class="mt-0.5 shrink-0"
      @update:model-value="persistent = $event === true"
    />
    <div class="min-w-0 space-y-2">
      <Label :for="fieldId">{{
        t("admin.webTerminal.persistentConnection")
      }}</Label>
      <div
        :id="`${fieldId}-help`"
        class="space-y-1 text-xs leading-relaxed text-muted-foreground"
      >
        <p>{{ t("admin.webTerminal.persistenceConnectionScope") }}</p>
        <p>{{ t("admin.webTerminal.persistenceDescription") }}</p>
        <p>{{ t("admin.webTerminal.persistenceLeaseDescription") }}</p>
      </div>
    </div>
  </div>
</template>
