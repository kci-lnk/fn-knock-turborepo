<script setup lang="ts">
import { Plus, Trash2 } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import type { AdvancedAuthRegionSelectorText } from "../subdomain-proxy/advanced-auth-rule-contract";
import StreamBypassConditionEditor from "./StreamBypassConditionEditor.vue";
import {
  MAX_STREAM_BYPASS_CONDITIONS,
  type StreamBypassGroup,
  type StreamBypassRuleEditor,
} from "./stream-bypass-policy-form";

defineProps<{
  editor: StreamBypassRuleEditor;
  group: StreamBypassGroup;
  groupIndex: number;
  regionText: AdvancedAuthRegionSelectorText;
  saving: boolean;
}>();
const { t } = useI18n();
</script>

<template>
  <div
    class="group/rule space-y-3 rounded-xl border border-border/65 bg-muted/25 p-3 shadow-none ring-2 ring-transparent transition-[border-color,background-color,box-shadow] duration-[280ms] ease-out hover:border-primary/25 hover:bg-muted/35 hover:ring-primary/5 focus-within:border-primary/50 focus-within:bg-muted/35 focus-within:ring-primary/15 motion-reduce:transition-none dark:bg-muted/20 sm:p-5"
  >
    <div class="flex items-center justify-between gap-3">
      <div class="flex min-w-0 items-center gap-2 text-sm font-medium">
        <span
          class="shrink-0 rounded-md border border-primary/20 bg-primary/10 px-2 py-1 text-xs font-semibold text-primary"
        >
          OR {{ groupIndex + 1 }}
        </span>
        <span class="truncate">{{
          t("admin.streamMappings.policyGroupAll")
        }}</span>
      </div>
      <Button
        variant="ghost"
        size="icon"
        class="h-8 w-8 shrink-0"
        :disabled="saving"
        :aria-label="t('admin.advancedAuth.deleteGroup')"
        @click="editor.removeGroup(groupIndex)"
      >
        <Trash2 class="h-4 w-4 text-destructive" />
      </Button>
    </div>

    <div
      class="relative space-y-3"
      :class="group.conditions.length > 1 ? 'sm:pl-9' : ''"
    >
      <div
        v-if="group.conditions.length > 1"
        class="absolute inset-y-8 left-3.5 hidden w-px bg-border sm:block"
      ></div>
      <span
        v-if="group.conditions.length > 1"
        class="absolute top-1/2 left-3.5 z-10 hidden -translate-x-1/2 -translate-y-1/2 rounded border border-border bg-background px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground sm:block"
      >
        AND
      </span>
      <StreamBypassConditionEditor
        v-for="(condition, conditionIndex) in group.conditions"
        :key="condition.id"
        :condition="condition"
        :condition-index="conditionIndex"
        :editor="editor"
        :group="group"
        :multiple="group.conditions.length > 1"
        :region-text="regionText"
        :saving="saving"
      />
    </div>

    <div :class="group.conditions.length > 1 ? 'sm:pl-9' : ''">
      <Button
        variant="outline"
        size="sm"
        class="w-full min-[480px]:w-auto"
        :disabled="
          group.conditions.length >= MAX_STREAM_BYPASS_CONDITIONS || saving
        "
        @click="editor.addCondition(group)"
      >
        <Plus class="mr-2 h-4 w-4" />
        {{ t("admin.advancedAuth.addAndCondition") }}
      </Button>
    </div>
  </div>
</template>
