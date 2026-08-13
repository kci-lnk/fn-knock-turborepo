import { computed, ref, watch, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import { EventCenterAPI } from "@/lib/api/events";
import type {
  NotificationDelivery,
  NotificationDeliveryStatus,
  NotificationProviderView,
  NotificationRule,
} from "@/types";

export const useNotificationDeliveries = ({
  active,
}: {
  active: Ref<boolean>;
}) => {
  const { t } = useI18n();
  const deliveries = ref<NotificationDelivery[]>([]);
  const providers = ref<NotificationProviderView[]>([]);
  const rules = ref<NotificationRule[]>([]);
  const loading = ref(false);
  const currentPage = ref(1);
  const limit = ref("20");
  const total = ref(0);
  const activeDelivery = ref<NotificationDelivery | null>(null);
  const detailsOpen = ref(false);
  const clearing = ref(false);
  let loadGeneration = 0;

  const parsedLimit = computed(() => Number.parseInt(limit.value, 10) || 20);
  const clearDialogDescription = computed(() =>
    t("admin.notifications.deliveries.clearDialogDescription", {
      count: total.value,
    }),
  );
  const formatDeliveryStatusLabel = (status: NotificationDeliveryStatus) =>
    t(`admin.eventCenter.deliveryStatus.${status}`);

  const loadData = async () => {
    const generation = ++loadGeneration;
    loading.value = true;
    try {
      const [providersResult, rulesResult, deliveriesResult] =
        await Promise.all([
          EventCenterAPI.getNotificationProviders(),
          EventCenterAPI.getNotificationRules(),
          EventCenterAPI.getNotificationDeliveries({
            page: currentPage.value,
            limit: parsedLimit.value,
          }),
        ]);
      if (!providersResult.success) {
        throw new Error(
          providersResult.message ||
            t("admin.notifications.providers.providersLoadFailed"),
        );
      }
      if (!rulesResult.success) {
        throw new Error(
          rulesResult.message || t("admin.notifications.rules.rulesLoadFailed"),
        );
      }
      if (!deliveriesResult.success) {
        throw new Error(
          deliveriesResult.message ||
            t("admin.notifications.deliveries.deliveriesLoadFailed"),
        );
      }
      if (generation !== loadGeneration) return;
      providers.value = providersResult.data.providers || [];
      rules.value = rulesResult.data.rules || [];
      deliveries.value = deliveriesResult.data.deliveries || [];
      total.value = deliveriesResult.data.total || 0;
    } catch (error) {
      if (generation !== loadGeneration) return;
      toast.error(t("admin.notifications.deliveries.loadFailed"), {
        description:
          error instanceof Error ? error.message : t("common.tryLater"),
      });
    } finally {
      if (generation === loadGeneration) loading.value = false;
    }
  };

  const statusBadgeClass = (status: NotificationDeliveryStatus) => {
    switch (status) {
      case "success":
        return "border-emerald-500/25 bg-emerald-500/10 text-emerald-700";
      case "failed":
        return "border-amber-500/25 bg-amber-500/10 text-amber-700";
      case "gave_up":
        return "border-rose-500/25 bg-rose-500/10 text-rose-700";
      case "queued":
      case "sending":
        return "border-sky-500/25 bg-sky-500/10 text-sky-700";
      case "skipped":
        return "border-muted-foreground/20 bg-muted text-muted-foreground";
      default:
        return "";
    }
  };
  const resolveRuleName = (ruleId: string) =>
    rules.value.find((rule) => rule.id === ruleId)?.name || ruleId;
  const resolveProviderName = (providerId: string) =>
    providers.value.find((provider) => provider.id === providerId)?.name ||
    providerId;
  const openDetails = (delivery: NotificationDelivery) => {
    activeDelivery.value = delivery;
    detailsOpen.value = true;
  };
  const formatCopyValue = (value: unknown) => {
    const text = String(value ?? "").trim();
    return text || "-";
  };
  const formatJsonCopyBlock = (value: unknown) =>
    JSON.stringify(value || {}, null, 2);

  const detailCopyText = computed(() => {
    const delivery = activeDelivery.value;
    if (!delivery) return "";
    return [
      t("admin.notifications.deliveries.basicInfo"),
      t("admin.notifications.deliveries.ruleLabel", {
        value: resolveRuleName(delivery.rule_id),
      }),
      t("admin.notifications.deliveries.providerLabel", {
        value: resolveProviderName(delivery.provider_id),
      }),
      t("admin.notifications.deliveries.statusLabel", {
        value: formatDeliveryStatusLabel(delivery.status),
      }),
      t("admin.notifications.deliveries.attemptsLabel", {
        value: delivery.attempt_count,
      }),
      t("admin.notifications.deliveries.triggeredAtLabel", {
        value: formatCopyValue(delivery.triggered_at),
      }),
      t("admin.notifications.deliveries.sentAtLabel", {
        value: formatCopyValue(delivery.sent_at),
      }),
      t("admin.notifications.deliveries.nextRetryAtLabel", {
        value: formatCopyValue(delivery.next_retry_at),
      }),
      t("admin.notifications.deliveries.reasonLabel", {
        value: formatCopyValue(delivery.reason),
      }),
      "",
      t("admin.notifications.deliveries.messageSnapshot"),
      t("admin.notifications.deliveries.titleLabel", {
        value: formatCopyValue(delivery.message_snapshot.title),
      }),
      t("admin.notifications.deliveries.summaryLabel", {
        value: formatCopyValue(delivery.message_snapshot.summary),
      }),
      t("admin.notifications.deliveries.bodyLabel"),
      formatCopyValue(delivery.message_snapshot.body_text),
      "",
      t("admin.notifications.deliveries.requestSummary"),
      formatJsonCopyBlock(delivery.request_summary),
      "",
      t("admin.notifications.deliveries.responseSummary"),
      formatJsonCopyBlock(delivery.response_summary),
    ].join("\n");
  });

  const clearDeliveries = async () => {
    if (total.value === 0 || clearing.value) return;
    clearing.value = true;
    try {
      const result = await EventCenterAPI.clearNotificationDeliveries({});
      if (!result.success) {
        throw new Error(
          result.message || t("admin.notifications.deliveries.clearFailed"),
        );
      }
      const deletedCount = result.data.deleted_count || 0;
      toast.success(
        deletedCount > 0
          ? t("admin.notifications.deliveries.clearSuccess", {
              count: deletedCount,
            })
          : t("admin.notifications.deliveries.clearEmpty"),
      );
      activeDelivery.value = null;
      detailsOpen.value = false;
      if (currentPage.value !== 1) {
        currentPage.value = 1;
        return;
      }
      await loadData();
    } catch (error) {
      toast.error(t("admin.notifications.deliveries.clearFailed"), {
        description:
          error instanceof Error ? error.message : t("common.tryLater"),
      });
    } finally {
      clearing.value = false;
    }
  };

  watch([currentPage, limit], () => {
    if (active.value) void loadData();
  });
  watch(
    active,
    (isActive) => {
      if (isActive) void loadData();
    },
    { immediate: true },
  );

  return {
    activeDelivery,
    clearDeliveries,
    clearDialogDescription,
    clearing,
    currentPage,
    deliveries,
    detailCopyText,
    detailsOpen,
    formatCopyValue,
    formatDeliveryStatusLabel,
    formatJsonCopyBlock,
    limit,
    loadData,
    loading,
    openDetails,
    parsedLimit,
    resolveProviderName,
    resolveRuleName,
    statusBadgeClass,
    total,
  };
};
