import { computed, ref, toValue, watch, type MaybeRefOrGetter } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import { EventCenterAPI } from "@/lib/api/events";
import type {
  NotificationGroupBy,
  NotificationProviderDefinition,
  NotificationProviderView,
  NotificationRule,
  SystemEventType,
} from "../../../types";

export function useNotificationRulesResource(
  active: MaybeRefOrGetter<boolean>,
) {
  const { t } = useI18n();
  const catalog = ref<NotificationProviderDefinition[]>([]);
  const providers = ref<NotificationProviderView[]>([]);
  const rules = ref<NotificationRule[]>([]);
  const loading = ref(false);
  const deletingId = ref<string | null>(null);
  const clearAllDialogOpen = ref(false);
  const clearingAll = ref(false);

  const hasProviders = computed(() => providers.value.length > 0);

  const formatEventTypeLabel = (type: SystemEventType) =>
    t(`admin.eventCenter.eventTypes.${type}`);

  const formatGroupByLabel = (value: NotificationGroupBy) =>
    t(`admin.eventCenter.groupBy.${value}`);

  const buildRuleDisplayName = (eventType: SystemEventType) =>
    t("admin.notifications.rules.ruleDisplayName", {
      event: formatEventTypeLabel(eventType),
    });

  const loadData = async () => {
    loading.value = true;
    try {
      const [catalogResult, providersResult, rulesResult] = await Promise.all([
        EventCenterAPI.getNotificationProviderCatalog(),
        EventCenterAPI.getNotificationProviders(),
        EventCenterAPI.getNotificationRules(),
      ]);

      if (!catalogResult.success) {
        throw new Error(
          catalogResult.message ||
            t("admin.notifications.providers.catalogLoadFailed"),
        );
      }
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

      catalog.value = catalogResult.data.providers || [];
      providers.value = providersResult.data.providers || [];
      rules.value = rulesResult.data.rules || [];
    } catch (error) {
      toast.error(t("admin.notifications.rules.loadFailed"), {
        description:
          error instanceof Error ? error.message : t("common.tryLater"),
      });
    } finally {
      loading.value = false;
    }
  };

  const resolveProviderById = (providerId: string) =>
    providers.value.find((provider) => provider.id === providerId) || null;

  const resolveProviderDefinitionById = (providerId: string) => {
    const provider = resolveProviderById(providerId);
    if (!provider) return null;
    return catalog.value.find((item) => item.type === provider.type) || null;
  };

  const deleteRule = async (rule: NotificationRule) => {
    deletingId.value = rule.id;
    try {
      const result = await EventCenterAPI.deleteNotificationRule(rule.id);
      if (!result.success) {
        throw new Error(
          result.message || t("admin.notifications.rules.deleteRuleFailed"),
        );
      }
      toast.success(t("admin.notifications.rules.deleted"));
      await loadData();
    } catch (error) {
      toast.error(t("admin.notifications.rules.deleteRuleFailed"), {
        description:
          error instanceof Error ? error.message : t("common.tryLater"),
      });
    } finally {
      deletingId.value = null;
    }
  };

  const clearAllRules = async () => {
    if (rules.value.length === 0) {
      clearAllDialogOpen.value = false;
      return;
    }

    clearingAll.value = true;
    try {
      const results = await Promise.allSettled(
        rules.value.map(async (rule) => {
          const result = await EventCenterAPI.deleteNotificationRule(rule.id);
          if (!result.success) {
            throw new Error(
              result.message ||
                t("admin.notifications.rules.deleteRuleNamedFailed", {
                  name: rule.name,
                }),
            );
          }
          return rule.name;
        }),
      );

      const succeeded = results
        .filter(
          (item): item is PromiseFulfilledResult<string> =>
            item.status === "fulfilled",
        )
        .map((item) => item.value);
      const failed = results
        .filter(
          (item): item is PromiseRejectedResult => item.status === "rejected",
        )
        .map((item) =>
          item.reason instanceof Error
            ? item.reason.message
            : t("admin.notifications.rules.deleteRulesFailed"),
        );

      if (failed.length === results.length) {
        throw new Error(
          failed[0] || t("admin.notifications.rules.clearRulesFailed"),
        );
      }

      if (failed.length > 0) {
        toast.info(
          t("admin.notifications.rules.clearPartial", {
            success: succeeded.length,
            failed: failed.length,
          }),
          {
            description: failed[0],
          },
        );
      } else {
        toast.success(
          t("admin.notifications.rules.clearSuccess", {
            count: succeeded.length,
          }),
        );
      }

      clearAllDialogOpen.value = false;
      await loadData();
    } catch (error) {
      toast.error(t("admin.notifications.rules.clearFailed"), {
        description:
          error instanceof Error ? error.message : t("common.tryLater"),
      });
    } finally {
      clearingAll.value = false;
    }
  };

  const resolveProviderName = (providerId: string) =>
    resolveProviderById(providerId)?.name || providerId;

  const resolveProviderTypeLabel = (providerId: string) => {
    const definition = resolveProviderDefinitionById(providerId);
    if (definition) {
      return definition.label;
    }

    return (
      resolveProviderById(providerId)?.type ||
      t("admin.notifications.rules.unknownType")
    );
  };

  watch(
    () => toValue(active),
    (active) => {
      if (!active) return;
      void loadData();
    },
    { immediate: true },
  );

  return {
    providers,
    rules,
    loading,
    deletingId,
    clearAllDialogOpen,
    clearingAll,
    hasProviders,
    formatEventTypeLabel,
    formatGroupByLabel,
    buildRuleDisplayName,
    loadData,
    deleteRule,
    clearAllRules,
    resolveProviderName,
    resolveProviderTypeLabel,
    resolveProviderDefinitionById,
  };
}
