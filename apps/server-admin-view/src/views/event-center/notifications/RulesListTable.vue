<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Pencil, Trash2, Loader2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import type {
  NotificationGroupBy,
  NotificationRule,
  SystemEventType,
} from "../../../types";

defineProps<{
  buildRuleDisplayName: (eventType: SystemEventType) => string;
  clearingAll: boolean;
  deletingId: string | null;
  formatEventTypeLabel: (eventType: SystemEventType) => string;
  formatGroupByLabel: (groupBy: NotificationGroupBy) => string;
  loading: boolean;
  resolveProviderName: (providerId: string) => string;
  rules: NotificationRule[];
}>();

const emit = defineEmits<{
  edit: [rule: NotificationRule];
  "delete-rule": [rule: NotificationRule];
}>();

const { t } = useI18n();
</script>

<template>
  <div class="overflow-hidden rounded-md border bg-background">
    <div class="overflow-x-auto">
      <Table class="min-w-[700px] sm:min-w-[760px]">
        <TableHeader>
          <TableRow>
            <TableHead
              class="sticky left-0 z-20 w-[168px] min-w-[168px] border-r bg-background sm:w-[220px] sm:min-w-[220px]"
            >
              {{ t("admin.notifications.rules.name") }}
            </TableHead>
            <TableHead>
              {{ t("admin.notifications.rules.eventType") }}
            </TableHead>
            <TableHead>
              {{ t("admin.notifications.rules.triggerCondition") }}
            </TableHead>
            <TableHead>
              {{ t("admin.notifications.rules.groupBy") }}
            </TableHead>
            <TableHead>
              {{ t("admin.notifications.rules.targetCount") }}
            </TableHead>
            <TableHead>
              {{ t("admin.notifications.rules.lastTriggered") }}
            </TableHead>
            <TableHead class="w-[140px] text-right">
              {{ t("admin.notifications.rules.actions") }}
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-if="loading && rules.length === 0">
            <TableCell colspan="7" class="py-10 text-center">
              <Loader2
                class="mx-auto h-5 w-5 animate-spin text-muted-foreground"
              />
            </TableCell>
          </TableRow>
          <TableRow v-else-if="rules.length === 0">
            <TableCell
              colspan="7"
              class="py-10 text-center text-muted-foreground"
            >
              {{ t("admin.notifications.rules.empty") }}
            </TableCell>
          </TableRow>
          <TableRow v-for="rule in rules" :key="rule.id">
            <TableCell
              class="sticky left-0 z-10 w-[168px] min-w-[168px] border-r bg-background sm:w-[220px] sm:min-w-[220px]"
            >
              <div class="space-y-1">
                <div class="font-medium">
                  {{ buildRuleDisplayName(rule.event_type) }}
                </div>
                <div class="line-clamp-2 text-xs text-muted-foreground">
                  <span
                    v-for="target in rule.targets"
                    :key="target.id"
                    class="mr-2 inline-block"
                  >
                    {{ resolveProviderName(target.provider_id) }}
                  </span>
                </div>
              </div>
            </TableCell>
            <TableCell>{{ formatEventTypeLabel(rule.event_type) }}</TableCell>
            <TableCell>
              {{
                t("admin.notifications.rules.triggerSummary", {
                  seconds: rule.window_seconds,
                  count: rule.threshold_count,
                })
              }}
            </TableCell>
            <TableCell>
              {{ formatGroupByLabel(rule.group_by) }}
            </TableCell>
            <TableCell>{{ rule.targets.length }}</TableCell>
            <TableCell class="text-sm text-muted-foreground">
              <span v-if="rule.last_triggered_at">
                <HumanFriendlyTime :value="rule.last_triggered_at" />
              </span>
              <span v-else>-</span>
            </TableCell>
            <TableCell class="text-right">
              <div class="inline-flex items-center gap-1">
                <Button
                  variant="ghost"
                  size="icon"
                  :disabled="clearingAll"
                  @click="emit('edit', rule)"
                >
                  <Pencil class="h-4 w-4" />
                </Button>
                <ConfirmDangerPopover
                  :title="t('admin.notifications.rules.deleteTitle')"
                  :description="t('admin.notifications.rules.deleteDescription')"
                  :loading="deletingId === rule.id"
                  :disabled="deletingId === rule.id || clearingAll"
                  :on-confirm="() => emit('delete-rule', rule)"
                >
                  <template #trigger>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="text-destructive"
                      :disabled="deletingId === rule.id || clearingAll"
                    >
                      <Trash2 class="h-4 w-4" />
                    </Button>
                  </template>
                </ConfirmDangerPopover>
              </div>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>
  </div>
</template>
