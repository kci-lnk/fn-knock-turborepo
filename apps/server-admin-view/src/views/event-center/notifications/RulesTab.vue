<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ChevronDown, Plus, Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import RefreshButton from "@/components/RefreshButton.vue";
import NotificationRuleEditorDialog from "./NotificationRuleEditorDialog.vue";
import NotificationRulesClearDialog from "./NotificationRulesClearDialog.vue";
import RulesListTable from "./RulesListTable.vue";
import { useNotificationRules } from "./useNotificationRules";

const props = withDefaults(defineProps<{ active?: boolean }>(), {
  active: false,
});
const { t } = useI18n();
const controller = useNotificationRules(() => props.active);
const {
  buildRuleDisplayName,
  clearAllDialogOpen,
  clearingAll,
  deleteRule,
  deletingId,
  formatEventTypeLabel,
  formatGroupByLabel,
  handleCreateRuleClick,
  hasAvailableEventTypes,
  hasProviders,
  loadData,
  loading,
  openEditDialog,
  resolveProviderName,
  rules,
} = controller;
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

  <NotificationRuleEditorDialog :controller="controller" />

  <NotificationRulesClearDialog :controller="controller" />
</template>
