<script setup lang="ts">
import { Trash2 } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import CidrRegionSelector from "@/components/CidrRegionSelector.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { AdvancedAuthRegionSelectorText } from "../subdomain-proxy/advanced-auth-rule-contract";
import type {
  StreamBypassCondition,
  StreamBypassGroup,
  StreamBypassOperator,
  StreamBypassRuleEditor,
  StreamBypassTarget,
} from "./stream-bypass-policy-form";

defineProps<{
  condition: StreamBypassCondition;
  conditionIndex: number;
  editor: StreamBypassRuleEditor;
  group: StreamBypassGroup;
  multiple: boolean;
  regionText: AdvancedAuthRegionSelectorText;
  saving: boolean;
}>();
const { t } = useI18n();
const targets: Array<{ labelKey: string; value: StreamBypassTarget }> = [
  { labelKey: "admin.advancedAuth.targetSourceIp", value: "source_ip" },
  {
    labelKey: "admin.advancedAuth.targetSourceRegion",
    value: "source_region",
  },
];
</script>

<template>
  <div
    v-if="conditionIndex > 0"
    class="flex items-center gap-2 py-0.5 sm:hidden"
  >
    <span class="h-px flex-1 bg-border/80"></span>
    <span
      class="rounded border border-border/80 bg-background px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground"
    >
      AND
    </span>
    <span class="h-px flex-1 bg-border/80"></span>
  </div>

  <div
    class="relative rounded-lg border border-border/60 bg-background/80 p-3 shadow-none"
    :class="
      multiple
        ? 'sm:before:absolute sm:before:top-1/2 sm:before:-left-[1.375rem] sm:before:h-px sm:before:w-[1.375rem] sm:before:bg-border'
        : ''
    "
  >
    <div class="flex min-w-0 items-start gap-2">
      <div
        class="grid min-w-0 flex-1 gap-3 sm:grid-cols-2 xl:grid-cols-[minmax(9rem,0.8fr)_minmax(9rem,0.8fr)_minmax(15rem,1.8fr)]"
      >
        <div class="min-w-0 space-y-1.5">
          <Label :for="`stream-bypass-target-${condition.id}`" class="text-xs">
            {{ t("admin.advancedAuth.matchTarget") }}
          </Label>
          <select
            :id="`stream-bypass-target-${condition.id}`"
            class="h-9 w-full min-w-0 rounded-md border border-input bg-background px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
            :value="condition.target"
            :disabled="saving"
            @change="
              editor.updateTarget(
                condition,
                ($event.target as HTMLSelectElement)
                  .value as StreamBypassTarget,
              )
            "
          >
            <option
              v-for="target in targets"
              :key="target.value"
              :value="target.value"
            >
              {{ t(target.labelKey) }}
            </option>
          </select>
        </div>

        <div class="min-w-0 space-y-1.5">
          <Label
            :for="`stream-bypass-operator-${condition.id}`"
            class="text-xs"
          >
            {{ t("admin.advancedAuth.matchOperator") }}
          </Label>
          <select
            :id="`stream-bypass-operator-${condition.id}`"
            class="h-9 w-full min-w-0 rounded-md border border-input bg-background px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
            :value="condition.operator"
            :disabled="saving"
            @change="
              editor.updateOperator(
                condition,
                ($event.target as HTMLSelectElement)
                  .value as StreamBypassOperator,
              )
            "
          >
            <option
              v-for="operator in editor.operatorsFor(condition.target)"
              :key="operator.value"
              :value="operator.value"
            >
              {{ t(operator.labelKey) }}
            </option>
          </select>
        </div>

        <div
          v-if="condition.target === 'source_region'"
          class="min-w-0 space-y-1.5 sm:col-span-2 xl:col-span-1"
        >
          <div class="text-xs font-medium">
            {{ t("admin.advancedAuth.matchValue") }}
          </div>
          <CidrRegionSelector
            v-model="condition.selections"
            layout="compact"
            :disabled="saving"
            :text="regionText"
            :description="t('admin.streamMappings.policyRegionDescription')"
          />
        </div>

        <div v-else class="min-w-0 space-y-1.5 sm:col-span-2 xl:col-span-1">
          <Label
            :for="`stream-bypass-value-${condition.id}`"
            class="text-xs"
            :title="t(editor.sourceNetworkTranslationKey(condition, 'Hint'))"
          >
            {{ t(editor.sourceNetworkTranslationKey(condition, "Label")) }}
          </Label>
          <Input
            :id="`stream-bypass-value-${condition.id}`"
            :model-value="editor.valueInputText(condition)"
            class="font-mono"
            :placeholder="
              t(editor.sourceNetworkTranslationKey(condition, 'Placeholder'))
            "
            :title="t(editor.sourceNetworkTranslationKey(condition, 'Hint'))"
            :disabled="saving"
            @update:model-value="
              editor.setSourceValue(condition, String($event))
            "
            @blur="editor.normalizeValueDraft(condition)"
          />
        </div>
      </div>

      <Button
        variant="ghost"
        size="icon"
        class="group absolute top-1.5 right-1.5 h-7 w-7 shrink-0 sm:static sm:mt-5.5 sm:h-8 sm:w-8"
        :disabled="saving"
        :aria-label="t('admin.advancedAuth.deleteCondition')"
        @click="editor.removeCondition(group, conditionIndex)"
      >
        <Trash2
          class="h-4 w-4 text-muted-foreground transition-colors group-hover:text-destructive"
        />
      </Button>
    </div>
  </div>
</template>
