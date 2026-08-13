<script setup lang="ts">
import { useId } from "vue";
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { NOTIFICATION_GROUP_BY_OPTIONS } from "../constants";
import type { NotificationRuleEditorController } from "./notification-rule-editor-contract";
import {
  DEFAULT_RULE_COOLDOWN_SECONDS,
  DEFAULT_RULE_WINDOW_SECONDS,
} from "./rule-form";

const props = defineProps<{ controller: NotificationRuleEditorController }>();
const { formatGroupByLabel, groupByHint, ruleForm } = props.controller;
const { t } = useI18n();
const a11yId = useId();
</script>

<template>
  <section class="space-y-3 border-b border-border/60 pb-6">
    <div class="space-y-1">
      <div class="text-sm font-semibold">
        {{ t("admin.notifications.rules.triggerConditions") }}
      </div>
      <div class="text-xs text-muted-foreground">
        {{ t("admin.notifications.rules.triggerConditionsDescription") }}
      </div>
    </div>
    <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
      <div class="space-y-2">
        <Label :for="`${a11yId}-window`">
          {{ t("admin.notifications.rules.windowSeconds") }}
        </Label>
        <Input
          :id="`${a11yId}-window`"
          v-model="ruleForm.window_seconds"
          type="number"
          min="1"
          :placeholder="DEFAULT_RULE_WINDOW_SECONDS"
        />
      </div>
      <div class="space-y-2">
        <Label :for="`${a11yId}-threshold`">
          {{ t("admin.notifications.rules.thresholdCount") }}
        </Label>
        <Input
          :id="`${a11yId}-threshold`"
          v-model="ruleForm.threshold_count"
          type="number"
          min="1"
          placeholder="1"
        />
      </div>
      <div class="space-y-2">
        <Label :for="`${a11yId}-group-by`">
          {{ t("admin.notifications.rules.groupBy") }}
        </Label>
        <Select v-model="ruleForm.group_by">
          <SelectTrigger :id="`${a11yId}-group-by`">
            <SelectValue
              :placeholder="t('admin.notifications.rules.selectGroupBy')"
            />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="auto">
              {{ t("admin.notifications.rules.autoGroupBy") }}
            </SelectItem>
            <SelectItem
              v-for="option in NOTIFICATION_GROUP_BY_OPTIONS"
              :key="option.value"
              :value="option.value"
            >
              {{ formatGroupByLabel(option.value) }}
            </SelectItem>
          </SelectContent>
        </Select>
        <div v-if="groupByHint" class="text-xs text-muted-foreground">
          {{ groupByHint }}
        </div>
      </div>
      <div class="space-y-2">
        <Label :for="`${a11yId}-cooldown`">
          {{ t("admin.notifications.rules.cooldownSeconds") }}
        </Label>
        <Input
          :id="`${a11yId}-cooldown`"
          v-model="ruleForm.cooldown_seconds"
          type="number"
          min="0"
          :placeholder="DEFAULT_RULE_COOLDOWN_SECONDS"
        />
      </div>
    </div>
  </section>
</template>
