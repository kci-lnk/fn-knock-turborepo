<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Checkbox } from "@/components/ui/checkbox";
import { DEFAULT_GROUP_BY_BY_EVENT_TYPE } from "../constants";
import type { NotificationRuleEditorController } from "./notification-rule-editor-contract";

const props = defineProps<{ controller: NotificationRuleEditorController }>();
const {
  availableEventTypeOptions,
  formatEventTypeLabel,
  formatGroupByLabel,
  isAllEventTypesSelected,
  ruleForm,
  toggleAllEventTypes,
  toggleEventType,
} = props.controller;
const { t } = useI18n();
</script>

<template>
  <section class="space-y-4 border-b border-border/60 pb-6">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="space-y-1">
        <div class="text-sm font-semibold">
          {{ t("admin.notifications.rules.triggerEvents") }}
        </div>
        <div class="text-xs text-muted-foreground">
          {{ t("admin.notifications.rules.triggerEventsDescription") }}
        </div>
      </div>
      <div class="flex items-center gap-2 px-3 py-2">
        <Checkbox
          :model-value="isAllEventTypesSelected"
          :aria-label="t('admin.notifications.rules.selectAll')"
          @update:model-value="toggleAllEventTypes"
        />
        <span class="text-xs text-muted-foreground">
          {{ t("admin.notifications.rules.selectAll") }}
        </span>
      </div>
    </div>

    <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
      <label
        v-for="option in availableEventTypeOptions"
        :key="option.value"
        class="flex cursor-pointer items-start gap-3 rounded-xl border px-4 py-3 transition-all"
        :class="
          ruleForm.event_types.includes(option.value)
            ? 'border-primary/40 bg-primary/5 shadow-sm'
            : 'border-border/70 hover:border-primary/20 hover:bg-muted/30'
        "
      >
        <Checkbox
          :model-value="ruleForm.event_types.includes(option.value)"
          @update:model-value="toggleEventType(option.value, $event)"
        />
        <div class="space-y-1">
          <div class="text-sm font-medium leading-5">
            {{ formatEventTypeLabel(option.value) }}
          </div>
          <div class="text-xs text-muted-foreground">
            {{ t("admin.notifications.rules.recommendedGroupBy") }}
            {{
              formatGroupByLabel(
                DEFAULT_GROUP_BY_BY_EVENT_TYPE[option.value],
              )
            }}
          </div>
        </div>
      </label>
    </div>
  </section>
</template>
