<script setup lang="ts">
import { useId } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, Loader2, Plus, Trash2 } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import RefreshButton from "@/components/RefreshButton.vue";
import {
  DEFAULT_GROUP_BY_BY_EVENT_TYPE,
  NOTIFICATION_GROUP_BY_OPTIONS,
} from "../constants";
import SchemaFieldsEditor from "./SchemaFieldsEditor.vue";
import {
  DEFAULT_RULE_COOLDOWN_SECONDS,
  DEFAULT_RULE_WINDOW_SECONDS,
} from "./rule-form";
import RulesListTable from "./RulesListTable.vue";
import { useNotificationRules } from "./useNotificationRules";

const a11yId = useId();

const props = withDefaults(
  defineProps<{
    active?: boolean;
  }>(),
  {
    active: false,
  },
);

const { t } = useI18n();
const {
  rules,
  loading,
  dialogOpen,
  saving,
  deletingId,
  clearAllDialogOpen,
  clearingAll,
  ruleForm,
  hasProviders,
  isEditMode,
  availableEventTypeOptions,
  hasAvailableEventTypes,
  availableProvidersForAdd,
  hasAvailableProvidersForAdd,
  isAllEventTypesSelected,
  dialogTitleText,
  dialogDescriptionText,
  dialogModeBadgeLabel,
  dialogSelectionBadgeLabel,
  dialogTargetsBadgeLabel,
  groupByHint,
  formatEventTypeLabel,
  formatGroupByLabel,
  buildRuleDisplayName,
  loadData,
  handleCreateRuleClick,
  openEditDialog,
  addTarget,
  removeTarget,
  toggleAllEventTypes,
  toggleEventType,
  saveRule,
  deleteRule,
  clearAllRules,
  resolveProviderName,
  resolveProviderTypeLabel,
  resolveProviderDefinitionById,
} = useNotificationRules(() => props.active);
</script>

<template>
  <div class="space-y-4 p-4 sm:p-6">
    <div class="flex flex-wrap items-center gap-2">
      <div class="space-y-1">
        <div class="text-xs text-muted-foreground">
          {{ t("admin.notifications.rules.toolbarHint") }}
        </div>
      </div>
      <div class="ml-auto flex items-center gap-2">
        <RefreshButton
          :loading="loading"
          :disabled="loading || clearingAll"
          @click="loadData"
        />
        <div class="flex">
          <Button
            class="rounded-r-none"
            :disabled="loading || clearingAll"
            @click="handleCreateRuleClick"
          >
            <Plus class="mr-2 h-4 w-4" />
            {{ t("admin.notifications.rules.addRule") }}
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button
                variant="default"
                size="icon"
                :aria-label="t('common.moreActions')"
                class="rounded-l-none border-l border-primary-foreground/20 px-2"
                :disabled="loading || clearingAll"
              >
                <ChevronDown class="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" class="w-52">
              <DropdownMenuItem
                variant="destructive"
                :disabled="rules.length === 0 || clearingAll"
                @click="clearAllDialogOpen = true"
              >
                <Trash2 class="mr-2 h-4 w-4" />
                {{ t("admin.notifications.rules.clearAllRules") }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>
    </div>

    <div
      v-if="!hasProviders"
      class="rounded-md border border-dashed bg-muted/30 px-4 py-6 text-sm text-muted-foreground"
    >
      {{ t("admin.notifications.rules.noProviders") }}
    </div>

    <div
      v-else-if="!hasAvailableEventTypes"
      class="rounded-md border border-dashed bg-muted/30 px-4 py-6 text-sm text-muted-foreground"
    >
      {{ t("admin.notifications.rules.noAvailableEventTypes") }}
    </div>

    <RulesListTable
      :build-rule-display-name="buildRuleDisplayName"
      :clearing-all="clearingAll"
      :deleting-id="deletingId"
      :format-event-type-label="formatEventTypeLabel"
      :format-group-by-label="formatGroupByLabel"
      :loading="loading"
      :resolve-provider-name="resolveProviderName"
      :rules="rules"
      @delete-rule="deleteRule"
      @edit="openEditDialog"
    />
  </div>

  <Dialog v-model:open="dialogOpen">
    <DialogContent
      class="flex max-h-[92vh] flex-col gap-0 overflow-hidden p-0 sm:max-w-[1040px]"
    >
      <DialogHeader
        class="border-b bg-gradient-to-r from-muted/40 via-background to-background px-4 py-5 sm:px-6"
      >
        <div class="space-y-3">
          <div class="space-y-1.5">
            <DialogTitle>{{ dialogTitleText }}</DialogTitle>
            <DialogDescription>{{ dialogDescriptionText }}</DialogDescription>
          </div>

          <div class="flex flex-wrap items-center gap-2">
            <Badge
              variant="secondary"
              class="rounded-full bg-primary/10 px-3 py-1 text-primary"
            >
              {{ dialogModeBadgeLabel }}
            </Badge>
            <Badge variant="outline" class="rounded-full px-3 py-1">
              {{ dialogSelectionBadgeLabel }}
            </Badge>
            <Badge variant="outline" class="rounded-full px-3 py-1">
              {{ dialogTargetsBadgeLabel }}
            </Badge>
          </div>
        </div>
      </DialogHeader>

      <div
        class="flex-1 space-y-6 overflow-y-auto bg-background px-4 py-5 sm:px-6"
      >
        <section
          v-if="!isEditMode"
          class="space-y-4 border-b border-border/60 pb-6"
        >
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div class="space-y-1">
              <div class="text-sm font-semibold">
                {{ t("admin.notifications.rules.triggerEvents") }}
              </div>
              <div class="text-xs text-muted-foreground">
                {{ t("admin.notifications.rules.triggerEventsDescription") }}
              </div>
            </div>
            <div class="flex items-center gap-2">
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
                @update:model-value="
                  (value) => toggleEventType(option.value, value)
                "
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

        <section
          v-if="isEditMode"
          class="space-y-3 border-b border-border/60 pb-6"
        >
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
              <Label :for="`${a11yId}-rulestab-1`">{{
                t("admin.notifications.rules.windowSeconds")
              }}</Label>
              <Input
                :id="`${a11yId}-rulestab-1`"
                v-model="ruleForm.window_seconds"
                type="number"
                min="1"
                :placeholder="DEFAULT_RULE_WINDOW_SECONDS"
              />
            </div>

            <div class="space-y-2">
              <Label :for="`${a11yId}-rulestab-2`">{{
                t("admin.notifications.rules.thresholdCount")
              }}</Label>
              <Input
                :id="`${a11yId}-rulestab-2`"
                v-model="ruleForm.threshold_count"
                type="number"
                min="1"
                placeholder="1"
              />
            </div>

            <div class="space-y-2">
              <Label :for="`${a11yId}-rulestab-3`">{{
                t("admin.notifications.rules.groupBy")
              }}</Label>
              <Select v-model="ruleForm.group_by">
                <SelectTrigger :id="`${a11yId}-rulestab-3`">
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
              <Label :for="`${a11yId}-rulestab-4`">{{
                t("admin.notifications.rules.cooldownSeconds")
              }}</Label>
              <Input
                :id="`${a11yId}-rulestab-4`"
                v-model="ruleForm.cooldown_seconds"
                type="number"
                min="0"
                :placeholder="DEFAULT_RULE_COOLDOWN_SECONDS"
              />
            </div>
          </div>
        </section>

        <section class="space-y-4">
          <div
            class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
          >
            <div class="space-y-1">
              <div class="text-sm font-semibold">
                {{ t("admin.notifications.rules.notificationTargets") }}
              </div>
            </div>
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <Button
                  variant="outline"
                  size="sm"
                  class="self-start"
                  :disabled="!hasProviders || !hasAvailableProvidersForAdd"
                >
                  <Plus class="mr-2 h-4 w-4" />
                  {{ t("admin.notifications.rules.addTarget") }}
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" class="w-64">
                <DropdownMenuItem
                  v-for="provider in availableProvidersForAdd"
                  :key="provider.id"
                  @click="addTarget(provider.id)"
                >
                  <div class="flex min-w-0 flex-col">
                    <span class="truncate font-medium">{{
                      provider.name
                    }}</span>
                    <span class="text-xs text-muted-foreground">
                      {{ resolveProviderTypeLabel(provider.id) }}
                    </span>
                  </div>
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>

          <div
            v-if="!ruleForm.targets.length"
            class="text-sm text-muted-foreground"
          >
            {{ t("admin.notifications.rules.targetEmpty") }}
          </div>

          <div
            v-else
            class="overflow-hidden rounded-lg border border-border/70 bg-background"
          >
            <div
              class="hidden grid-cols-[minmax(0,1fr)_180px_auto] gap-4 border-b bg-muted/20 px-4 py-3 text-xs font-medium text-muted-foreground sm:grid"
            >
              <div>{{ t("admin.notifications.rules.targetName") }}</div>
              <div>{{ t("admin.notifications.rules.providerType") }}</div>
              <div class="text-right">
                {{ t("admin.notifications.rules.actions") }}
              </div>
            </div>

            <div
              v-for="(target, index) in ruleForm.targets"
              :key="target.id || index"
              class="grid grid-cols-[minmax(0,1fr)_auto] gap-x-3 gap-y-3 border-b border-border/60 px-4 py-4 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_180px_auto] sm:gap-4"
            >
              <div class="min-w-0 pr-2 sm:pr-0">
                <div
                  class="mb-1 text-[11px] font-medium tracking-wide text-muted-foreground sm:hidden"
                >
                  {{ t("admin.notifications.rules.targetName") }}
                </div>
                <div class="break-words text-sm font-medium sm:truncate">
                  {{ resolveProviderName(target.provider_id) }}
                </div>
              </div>

              <div class="col-span-2 min-w-0 sm:col-span-1 sm:pt-0.5">
                <div
                  class="mb-1 text-[11px] font-medium tracking-wide text-muted-foreground sm:hidden"
                >
                  {{ t("admin.notifications.rules.providerType") }}
                </div>
                <div class="text-sm text-muted-foreground">
                  {{ resolveProviderTypeLabel(target.provider_id) }}
                </div>
              </div>

              <div
                class="col-start-2 row-start-1 flex items-start justify-end sm:col-start-auto sm:row-start-auto"
              >
                <Button
                  variant="ghost"
                  size="icon"
                  :aria-label="t('common.confirmDelete')"
                  class="text-destructive"
                  :disabled="ruleForm.targets.length <= 1"
                  @click="removeTarget(index)"
                >
                  <Trash2 class="h-4 w-4" />
                </Button>
              </div>

              <div
                v-if="
                  resolveProviderDefinitionById(target.provider_id) &&
                  resolveProviderDefinitionById(target.provider_id)!
                    .target_schema.length > 0
                "
                class="col-span-2 rounded-md border border-dashed bg-muted/10 p-3 sm:col-span-3"
              >
                <div class="mb-3 text-xs font-medium text-muted-foreground">
                  {{ t("admin.notifications.rules.targetConfig") }}
                </div>
                <SchemaFieldsEditor
                  :fields="
                    resolveProviderDefinitionById(target.provider_id)!
                      .target_schema
                  "
                  :model-value="target.target_config"
                  @update:model-value="
                    (value) => {
                      ruleForm.targets[index]!.target_config = value;
                    }
                  "
                />
              </div>
            </div>
          </div>
        </section>
      </div>

      <div
        class="flex flex-col-reverse gap-2 border-t bg-background px-4 py-4 sm:flex-row sm:items-center sm:justify-between sm:px-6"
      >
        <div class="text-xs text-muted-foreground">
          {{
            isEditMode
              ? t("admin.notifications.rules.saveEditHint")
              : t("admin.notifications.rules.saveCreateHint")
          }}
        </div>
        <DialogFooter class="gap-2 sm:flex-row">
          <Button variant="outline" @click="dialogOpen = false">
            {{ t("common.cancel") }}
          </Button>
          <Button :disabled="saving" @click="saveRule">
            <Loader2 v-if="saving" class="mr-2 h-4 w-4 animate-spin" />
            {{ t("common.save") }}
          </Button>
        </DialogFooter>
      </div>
    </DialogContent>
  </Dialog>

  <Dialog v-model:open="clearAllDialogOpen">
    <DialogContent class="sm:max-w-[420px]">
      <DialogHeader>
        <DialogTitle>
          {{ t("admin.notifications.rules.clearDialogTitle") }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.notifications.rules.clearDialogDescription") }}
        </DialogDescription>
      </DialogHeader>

      <DialogFooter class="gap-2">
        <Button
          variant="outline"
          :disabled="clearingAll"
          @click="clearAllDialogOpen = false"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          variant="destructive"
          :disabled="clearingAll || rules.length === 0"
          @click="clearAllRules"
        >
          <Loader2 v-if="clearingAll" class="mr-2 h-4 w-4 animate-spin" />
          {{ t("admin.notifications.rules.clearAllRules") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
