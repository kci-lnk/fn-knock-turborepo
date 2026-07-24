<script setup lang="ts">
import { toRef } from "vue";
import { useI18n } from "vue-i18n";
import { Eye, Loader2, Trash2 } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import RefreshButton from "@/components/RefreshButton.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import { useNotificationDeliveries } from "./useNotificationDeliveries";

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
  activeDelivery,
  clearDeliveries,
  clearDialogDescription,
  clearing,
  currentPage,
  deliveries,
  detailCopyText,
  detailsOpen,
  formatDeliveryStatusLabel,
  limit,
  loadData,
  loading,
  openDetails,
  parsedLimit,
  resolveProviderName,
  resolveRuleName,
  statusBadgeClass,
  total,
} = useNotificationDeliveries({
  active: toRef(props, "active"),
});
</script>

<template>
  <div class="space-y-4 p-4 sm:p-6">
    <div class="flex flex-wrap items-center gap-2">
      <div class="text-sm text-muted-foreground">
        {{ t("admin.notifications.deliveries.intro") }}
      </div>

      <div class="ml-auto flex items-center gap-2">
        <ConfirmDangerPopover
          :title="t('admin.notifications.deliveries.clearTitle')"
          :description="clearDialogDescription"
          :confirm-text="t('admin.notifications.deliveries.confirmClear')"
          :loading="clearing"
          :disabled="loading || clearing || total === 0"
          content-class="w-80 text-left"
          :on-confirm="clearDeliveries"
        >
          <template #trigger>
            <Button
              variant="outline"
              class="border-destructive/20 text-destructive hover:bg-destructive/5 hover:text-destructive"
              :disabled="loading || clearing || total === 0"
            >
              <Trash2 class="mr-2 h-4 w-4" />
              {{ t("admin.notifications.deliveries.clearRecords") }}
            </Button>
          </template>
        </ConfirmDangerPopover>

        <RefreshButton
          :loading="loading"
          :disabled="loading || clearing"
          @click="loadData"
        />
      </div>
    </div>

    <div class="overflow-hidden rounded-md border bg-background">
      <div class="overflow-auto">
        <Table class="min-w-[980px]">
          <TableHeader>
            <TableRow>
              <TableHead>{{
                t("admin.notifications.deliveries.time")
              }}</TableHead>
              <TableHead>{{
                t("admin.notifications.deliveries.rule")
              }}</TableHead>
              <TableHead>{{
                t("admin.notifications.deliveries.provider")
              }}</TableHead>
              <TableHead>{{
                t("admin.notifications.deliveries.status")
              }}</TableHead>
              <TableHead>{{
                t("admin.notifications.deliveries.message")
              }}</TableHead>
              <TableHead>{{
                t("admin.notifications.deliveries.attempts")
              }}</TableHead>
              <TableHead class="w-[110px] text-right">
                {{ t("admin.notifications.deliveries.actions") }}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-if="loading && deliveries.length === 0">
              <TableCell colspan="7" class="py-10 text-center">
                <Loader2
                  class="mx-auto h-5 w-5 animate-spin text-muted-foreground"
                />
              </TableCell>
            </TableRow>
            <TableRow v-else-if="deliveries.length === 0">
              <TableCell
                colspan="7"
                class="py-10 text-center text-muted-foreground"
              >
                {{ t("admin.notifications.deliveries.empty") }}
              </TableCell>
            </TableRow>
            <TableRow v-for="delivery in deliveries" :key="delivery.id">
              <TableCell class="text-sm text-muted-foreground">
                <HumanFriendlyTime :value="delivery.triggered_at" />
              </TableCell>
              <TableCell>{{ resolveRuleName(delivery.rule_id) }}</TableCell>
              <TableCell>
                {{ resolveProviderName(delivery.provider_id) }}
              </TableCell>
              <TableCell>
                <Badge
                  variant="outline"
                  :class="statusBadgeClass(delivery.status)"
                >
                  {{ formatDeliveryStatusLabel(delivery.status) }}
                </Badge>
              </TableCell>
              <TableCell class="max-w-[380px]">
                <div class="space-y-1">
                  <div class="line-clamp-1 font-medium">
                    {{ delivery.message_snapshot.title }}
                  </div>
                  <div class="line-clamp-2 text-xs text-muted-foreground">
                    {{ delivery.message_snapshot.summary }}
                  </div>
                  <div
                    v-if="delivery.reason"
                    class="line-clamp-2 text-xs text-amber-700"
                  >
                    {{ delivery.reason }}
                  </div>
                </div>
              </TableCell>
              <TableCell>{{ delivery.attempt_count }}</TableCell>
              <TableCell class="text-right">
                <Button
                  variant="ghost"
                  size="icon"
                  :aria-label="t('common.viewDetails')"
                  :disabled="clearing"
                  @click="openDetails(delivery)"
                >
                  <Eye class="h-4 w-4" />
                </Button>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>

      <PagedTableFooter
        :total="total"
        :page="currentPage"
        :limit="limit"
        :items-per-page="parsedLimit"
        :total-text="t('admin.notifications.deliveries.totalText')"
        :floating="props.active"
        @update:page="(value) => (currentPage = value)"
        @update:limit="(value) => (limit = value)"
      />
    </div>
  </div>

  <DetailDialog
    v-model:open="detailsOpen"
    :title="t('admin.notifications.deliveries.detailTitle')"
    :description="t('admin.notifications.deliveries.detailDescription')"
    max-width-class="sm:max-w-[860px]"
    :copy-text="detailCopyText"
  >
    <div v-if="activeDelivery" class="space-y-5">
      <div class="grid gap-4 md:grid-cols-2">
        <div class="rounded-md border p-4">
          <div class="mb-2 text-sm font-medium">
            {{ t("admin.notifications.deliveries.basicInfo") }}
          </div>
          <div class="space-y-1 text-sm">
            <div>
              {{
                t("admin.notifications.deliveries.ruleLabel", {
                  value: resolveRuleName(activeDelivery.rule_id),
                })
              }}
            </div>
            <div>
              {{
                t("admin.notifications.deliveries.providerLabel", {
                  value: resolveProviderName(activeDelivery.provider_id),
                })
              }}
            </div>
            <div>
              {{
                t("admin.notifications.deliveries.statusLabel", {
                  value: formatDeliveryStatusLabel(activeDelivery.status),
                })
              }}
            </div>
            <div>
              {{
                t("admin.notifications.deliveries.attemptsLabel", {
                  value: activeDelivery.attempt_count,
                })
              }}
            </div>
            <div>
              {{
                t("admin.notifications.deliveries.triggeredAtLabel", {
                  value: activeDelivery.triggered_at,
                })
              }}
            </div>
            <div v-if="activeDelivery.sent_at">
              {{
                t("admin.notifications.deliveries.sentAtLabel", {
                  value: activeDelivery.sent_at,
                })
              }}
            </div>
            <div v-if="activeDelivery.next_retry_at">
              {{
                t("admin.notifications.deliveries.nextRetryAtLabel", {
                  value: activeDelivery.next_retry_at,
                })
              }}
            </div>
            <div v-if="activeDelivery.reason">
              {{
                t("admin.notifications.deliveries.reasonLabel", {
                  value: activeDelivery.reason,
                })
              }}
            </div>
          </div>
        </div>

        <div class="rounded-md border p-4">
          <div class="mb-2 text-sm font-medium">
            {{ t("admin.notifications.deliveries.messageSnapshot") }}
          </div>
          <div class="space-y-2 text-sm">
            <div class="font-medium">
              {{ activeDelivery.message_snapshot.title }}
            </div>
            <div class="text-muted-foreground">
              {{ activeDelivery.message_snapshot.summary }}
            </div>
            <pre
              class="max-h-[220px] overflow-auto rounded bg-muted p-3 text-xs whitespace-pre-wrap"
              >{{ activeDelivery.message_snapshot.body_text }}</pre
            >
          </div>
        </div>
      </div>

      <div class="grid gap-4 md:grid-cols-2">
        <div class="rounded-md border p-4">
          <div class="mb-2 text-sm font-medium">
            {{ t("admin.notifications.deliveries.requestSummary") }}
          </div>
          <pre
            class="max-h-[220px] overflow-auto rounded bg-muted p-3 text-xs whitespace-pre-wrap"
            >{{
              JSON.stringify(activeDelivery.request_summary || {}, null, 2)
            }}</pre
          >
        </div>

        <div class="rounded-md border p-4">
          <div class="mb-2 text-sm font-medium">
            {{ t("admin.notifications.deliveries.responseSummary") }}
          </div>
          <pre
            class="max-h-[220px] overflow-auto rounded bg-muted p-3 text-xs whitespace-pre-wrap"
            >{{
              JSON.stringify(activeDelivery.response_summary || {}, null, 2)
            }}</pre
          >
        </div>
      </div>
    </div>
  </DetailDialog>
</template>
