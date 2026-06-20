<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Download, Eye, Loader2, Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Switch } from "@/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import type { WAFRuleFile } from "../../../types";

const props = withDefaults(
  defineProps<{
    activeRuleActionsKey: string;
    deleteRule?: (filename: string) => Promise<void> | void;
    downloadingRuleKey: string;
    emptyLabel: string;
    formatRuleAside?: (rule: WAFRuleFile) => string;
    formatRuleMeta: (rule: WAFRuleFile) => string;
    formatRuleName: (rule: WAFRuleFile) => string;
    isBusy: boolean;
    isChangingRules?: boolean;
    loadingRuleKey: string;
    rules: WAFRuleFile[];
    selectedFilenames: string[];
    showDelete?: boolean;
  }>(),
  {
    deleteRule: undefined,
    formatRuleAside: undefined,
    isChangingRules: false,
    showDelete: false,
  },
);

const emit = defineEmits<{
  activateRuleActions: [rule: WAFRuleFile];
  downloadRuleFile: [rule: WAFRuleFile];
  openRulePreview: [rule: WAFRuleFile];
  setAllSelected: [checked: boolean];
  setRuleSelected: [filename: string, checked: boolean];
  toggleAllRules: [];
  toggleRule: [rule: WAFRuleFile, enabled: boolean];
  updateSelectedRules: [enabled: boolean];
}>();

const { t } = useI18n();

const selectedCount = computed(() => props.selectedFilenames.length);
const isAllSelected = computed(
  () => props.rules.length > 0 && selectedCount.value === props.rules.length,
);
const toggleAllRulesLabel = computed(() =>
  props.rules.length > 0 && props.rules.every((rule) => rule.enabled)
    ? t("admin.wafSettings.disableAll")
    : t("admin.wafSettings.enableAll"),
);

const ruleKey = (rule: WAFRuleFile) => `${rule.source}:${rule.filename}`;

const ruleActionsClass = (rule: WAFRuleFile) =>
  props.activeRuleActionsKey === ruleKey(rule)
    ? "visible opacity-100"
    : "invisible opacity-0 group-hover:visible group-hover:opacity-100 group-focus-within:visible group-focus-within:opacity-100";
</script>

<template>
  <div v-if="rules.length === 0" class="text-sm text-muted-foreground">
    {{ emptyLabel }}
  </div>
  <div v-else class="overflow-hidden rounded-md border">
    <div
      class="flex flex-col gap-3 border-b bg-muted/10 px-4 py-3 sm:flex-row sm:items-center sm:justify-between"
    >
      <label class="flex items-center gap-3 text-sm">
        <Checkbox
          :model-value="isAllSelected"
          :disabled="isBusy"
          @update:model-value="(value) => emit('setAllSelected', value === true)"
        />
        <span>
          {{
            t("admin.wafSettings.selectedCount", {
              count: selectedCount,
            })
          }}
        </span>
      </label>
      <div class="flex flex-wrap gap-2">
        <Button
          v-if="selectedCount > 0"
          variant="outline"
          size="sm"
          :disabled="isBusy"
          @click="emit('updateSelectedRules', true)"
        >
          {{ t("admin.wafSettings.enableSelected") }}
        </Button>
        <Button
          v-if="selectedCount > 0"
          variant="outline"
          size="sm"
          :disabled="isBusy"
          @click="emit('updateSelectedRules', false)"
        >
          {{ t("admin.wafSettings.disableSelected") }}
        </Button>
        <Button
          variant="outline"
          size="sm"
          :disabled="isBusy"
          @click="emit('toggleAllRules')"
        >
          {{ toggleAllRulesLabel }}
        </Button>
      </div>
    </div>

    <div class="divide-y">
      <div
        v-for="rule in rules"
        :key="rule.filename"
        class="group flex flex-col gap-3 px-4 py-3 sm:flex-row sm:items-center"
        @pointerdown.passive="emit('activateRuleActions', rule)"
        @touchstart.passive="emit('activateRuleActions', rule)"
      >
        <Checkbox
          :model-value="selectedFilenames.includes(rule.filename)"
          :disabled="isBusy"
          @update:model-value="
            (value) => emit('setRuleSelected', rule.filename, value === true)
          "
        />
        <div class="min-w-0 flex-1 space-y-1">
          <div class="flex min-w-0 items-center gap-2">
            <div class="min-w-0 truncate font-mono text-sm">
              {{ formatRuleName(rule) }}
            </div>
            <div
              class="flex h-8 shrink-0 items-center gap-1 transition-opacity duration-150"
              :class="ruleActionsClass(rule)"
            >
              <Tooltip>
                <TooltipTrigger as-child>
                  <Button
                    variant="ghost"
                    size="icon"
                    class="h-8 w-8 text-muted-foreground hover:text-foreground"
                    :disabled="loadingRuleKey === ruleKey(rule)"
                    :title="t('admin.wafSettings.viewRule')"
                    :aria-label="t('admin.wafSettings.viewRule')"
                    @click.stop="emit('openRulePreview', rule)"
                  >
                    <Loader2
                      v-if="loadingRuleKey === ruleKey(rule)"
                      class="h-4 w-4 animate-spin"
                    />
                    <Eye v-else class="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  {{ t("admin.wafSettings.viewRule") }}
                </TooltipContent>
              </Tooltip>
              <Tooltip>
                <TooltipTrigger as-child>
                  <Button
                    variant="ghost"
                    size="icon"
                    class="h-8 w-8 text-muted-foreground hover:text-foreground"
                    :disabled="downloadingRuleKey === ruleKey(rule)"
                    :title="t('admin.wafSettings.downloadRule')"
                    :aria-label="t('admin.wafSettings.downloadRule')"
                    @click.stop="emit('downloadRuleFile', rule)"
                  >
                    <Loader2
                      v-if="downloadingRuleKey === ruleKey(rule)"
                      class="h-4 w-4 animate-spin"
                    />
                    <Download v-else class="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  {{ t("admin.wafSettings.downloadRule") }}
                </TooltipContent>
              </Tooltip>
            </div>
          </div>
          <div class="text-sm text-muted-foreground">
            {{ formatRuleMeta(rule) }}
          </div>
        </div>
        <div
          :class="[
            'flex items-center justify-between sm:justify-end',
            showDelete ? 'gap-3' : 'gap-4',
          ]"
        >
          <span
            v-if="formatRuleAside?.(rule)"
            class="text-xs text-muted-foreground"
          >
            {{ formatRuleAside(rule) }}
          </span>
          <Switch
            :model-value="rule.enabled"
            :disabled="isBusy"
            @update:model-value="
              (value) => emit('toggleRule', rule, value === true)
            "
          />
          <ConfirmDangerPopover
            v-if="showDelete && deleteRule"
            :title="
              t('admin.wafSettings.deleteConfirmTitle', {
                filename: rule.filename,
              })
            "
            :description="t('admin.wafSettings.deleteConfirmDescription')"
            :loading="isChangingRules"
            :disabled="isBusy"
            :on-confirm="() => deleteRule?.(rule.filename)"
          >
            <template #trigger>
              <Button
                variant="outline"
                size="icon"
                class="h-8 w-8 border-destructive/30 text-destructive hover:bg-destructive/10 hover:text-destructive"
                :disabled="isBusy"
              >
                <Trash2 class="h-4 w-4" />
              </Button>
            </template>
          </ConfirmDangerPopover>
        </div>
      </div>
    </div>
  </div>
</template>
