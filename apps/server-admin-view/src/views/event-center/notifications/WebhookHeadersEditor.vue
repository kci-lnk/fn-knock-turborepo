<script setup lang="ts">
import { computed, useId } from "vue";
import { Plus, Trash2 } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type {
  NotificationHeaderConstraints,
  NotificationHeaderEntry,
} from "../../../types";
import {
  coerceWebhookHeaderEntries,
  resolveWebhookHeaderConstraints,
  validateWebhookHeaderEntries,
  type WebhookHeaderValidationIssue,
} from "./webhook-headers";

const props = defineProps<{
  modelValue: NotificationHeaderEntry[];
  constraints?: NotificationHeaderConstraints;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: NotificationHeaderEntry[]];
}>();

const { t } = useI18n();
const a11yId = useId();
const entries = computed(() => coerceWebhookHeaderEntries(props.modelValue));
const resolvedConstraints = computed(() =>
  resolveWebhookHeaderConstraints(props.constraints),
);
const issues = computed(() =>
  validateWebhookHeaderEntries(entries.value, resolvedConstraints.value),
);
const globalIssues = computed(() =>
  issues.value.filter((issue) => issue.row === undefined),
);

const rowIssues = (row: number) =>
  issues.value.filter((issue) => issue.row === row);
const rowHasValueIssue = (row: number) =>
  rowIssues(row).some((issue) =>
    ["valueTooLong", "invalidValue"].includes(issue.code),
  );
const rowHasNameIssue = (row: number) =>
  rowIssues(row).some(
    (issue) => !["valueTooLong", "invalidValue"].includes(issue.code),
  );

const formatIssue = (issue: WebhookHeaderValidationIssue) =>
  t(`admin.notifications.headers.errors.${issue.code}`, {
    name: issue.name || "",
    max: issue.max || "",
  });

const updateEntry = (
  index: number,
  key: keyof NotificationHeaderEntry,
  value: string | number,
) => {
  const next = entries.value.map((entry) => ({ ...entry }));
  next[index] = {
    ...(next[index] || { name: "", value: "" }),
    [key]: String(value),
  };
  emit("update:modelValue", next);
};

const addEntry = () => {
  emit("update:modelValue", [
    ...entries.value.map((entry) => ({ ...entry })),
    { name: "", value: "" },
  ]);
};

const removeEntry = (index: number) => {
  emit(
    "update:modelValue",
    entries.value.filter((_, row) => row !== index),
  );
};
</script>

<template>
  <div class="space-y-3">
    <div
      v-if="entries.length === 0"
      class="rounded-md border border-dashed px-3 py-4 text-center text-xs text-muted-foreground"
    >
      {{ t("admin.notifications.headers.empty") }}
    </div>

    <div v-else class="space-y-3">
      <div
        v-for="(entry, index) in entries"
        :key="index"
        class="grid gap-2 rounded-md border border-border/60 p-3 md:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)_auto] md:items-start"
      >
        <div class="space-y-1.5">
          <Label :for="`${a11yId}-header-name-${index}`" class="text-xs">
            {{ t("admin.notifications.headers.name") }}
          </Label>
          <Input
            :id="`${a11yId}-header-name-${index}`"
            :model-value="entry.name"
            :placeholder="t('admin.notifications.headers.namePlaceholder')"
            :aria-invalid="rowHasNameIssue(index)"
            :aria-describedby="
              rowIssues(index).length
                ? `${a11yId}-header-errors-${index}`
                : undefined
            "
            autocomplete="off"
            @update:model-value="updateEntry(index, 'name', $event)"
          />
        </div>
        <div class="space-y-1.5">
          <Label :for="`${a11yId}-header-value-${index}`" class="text-xs">
            {{ t("admin.notifications.headers.value") }}
          </Label>
          <Input
            :id="`${a11yId}-header-value-${index}`"
            :model-value="entry.value"
            :placeholder="t('admin.notifications.headers.valuePlaceholder')"
            :aria-invalid="rowHasValueIssue(index)"
            :aria-describedby="
              rowIssues(index).length
                ? `${a11yId}-header-errors-${index}`
                : undefined
            "
            autocomplete="off"
            @update:model-value="updateEntry(index, 'value', $event)"
          />
        </div>
        <Button
          variant="ghost"
          size="icon"
          class="mt-5 text-destructive md:mt-6"
          :aria-label="t('admin.notifications.headers.remove')"
          @click="removeEntry(index)"
        >
          <Trash2 class="h-4 w-4" />
        </Button>
        <div
          v-if="rowIssues(index).length"
          :id="`${a11yId}-header-errors-${index}`"
          class="text-xs text-destructive md:col-span-3"
          role="alert"
        >
          <p v-for="issue in rowIssues(index)" :key="issue.code">
            {{ formatIssue(issue) }}
          </p>
        </div>
      </div>
    </div>

    <div
      v-if="globalIssues.length"
      class="text-xs text-destructive"
      role="alert"
    >
      <p v-for="issue in globalIssues" :key="issue.code">
        {{ formatIssue(issue) }}
      </p>
    </div>

    <Button
      type="button"
      variant="outline"
      size="sm"
      :disabled="entries.length >= resolvedConstraints.max_items"
      @click="addEntry"
    >
      <Plus class="mr-2 h-4 w-4" />
      {{ t("admin.notifications.headers.add") }}
    </Button>
  </div>
</template>
