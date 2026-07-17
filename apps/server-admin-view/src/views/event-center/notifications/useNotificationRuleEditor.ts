import { computed, ref, type ComputedRef, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import { EventCenterAPI } from "../../../lib/api";
import type {
  NotificationGroupBy,
  NotificationProviderDefinition,
  NotificationProviderView,
  NotificationRule,
  SystemEventType,
} from "../../../types";
import {
  DEFAULT_GROUP_BY_BY_EVENT_TYPE,
  SYSTEM_EVENT_TYPE_OPTIONS,
} from "../constants";
import { createEditableSchemaRecord } from "./form-utils";
import {
  buildRulePayload,
  createEmptyDeliveryPolicy,
  createEmptyRuleForm,
  resolveGroupByForEventType,
  type EditableRuleForm,
  type EditableRuleTarget,
} from "./rule-form";

type UseNotificationRuleEditorOptions = {
  providers: Ref<NotificationProviderView[]>;
  rules: Ref<NotificationRule[]>;
  hasProviders: ComputedRef<boolean>;
  loadData: () => Promise<void>;
  resolveProviderDefinitionById: (
    providerId: string,
  ) => NotificationProviderDefinition | null;
  formatEventTypeLabel: (eventType: SystemEventType) => string;
  formatGroupByLabel: (groupBy: NotificationGroupBy) => string;
  buildRuleDisplayName: (eventType: SystemEventType) => string;
};

export function useNotificationRuleEditor({
  providers,
  rules,
  hasProviders,
  loadData,
  resolveProviderDefinitionById,
  formatEventTypeLabel,
  formatGroupByLabel,
  buildRuleDisplayName,
}: UseNotificationRuleEditorOptions) {
  const { t } = useI18n();
  const dialogOpen = ref(false);
  const dialogMode = ref<"create" | "edit">("create");
  const saving = ref(false);
  const editingRule = ref<NotificationRule | null>(null);

  const allEventTypes = SYSTEM_EVENT_TYPE_OPTIONS.map(
    (option) => option.value,
  ) as SystemEventType[];

  const ruleForm = ref<EditableRuleForm>(createEmptyRuleForm(allEventTypes));

  const isEditMode = computed(() => dialogMode.value === "edit");
  const usedEventTypes = computed(
    () => new Set(rules.value.map((rule) => rule.event_type)),
  );
  const availableEventTypeOptions = computed(() =>
    SYSTEM_EVENT_TYPE_OPTIONS.filter(
      (option) => !usedEventTypes.value.has(option.value),
    ),
  );
  const availableEventTypes = computed(() =>
    availableEventTypeOptions.value.map((option) => option.value),
  );
  const hasAvailableEventTypes = computed(
    () => availableEventTypes.value.length > 0,
  );
  const selectedTargetProviderIds = computed(
    () =>
      new Set(
        ruleForm.value.targets
          .map((target) => target.provider_id)
          .filter((providerId) => Boolean(providerId)),
      ),
  );
  const availableProvidersForAdd = computed(() =>
    providers.value.filter(
      (provider) => !selectedTargetProviderIds.value.has(provider.id),
    ),
  );
  const hasAvailableProvidersForAdd = computed(
    () => availableProvidersForAdd.value.length > 0,
  );

  const selectedEventTypeCount = computed(
    () => ruleForm.value.event_types.length,
  );

  const isAllEventTypesSelected = computed(
    () =>
      availableEventTypes.value.length > 0 &&
      selectedEventTypeCount.value === availableEventTypes.value.length,
  );

  const dialogTitleText = computed(() =>
    isEditMode.value
      ? t("admin.notifications.rules.dialogTitleEdit")
      : t("admin.notifications.rules.dialogTitleCreate"),
  );

  const dialogDescriptionText = computed(() =>
    isEditMode.value
      ? t("admin.notifications.rules.dialogDescriptionEdit")
      : t("admin.notifications.rules.dialogDescriptionCreate"),
  );

  const lockedEventTypeLabel = computed(() => {
    const eventType = ruleForm.value.event_types[0];
    return eventType
      ? formatEventTypeLabel(eventType)
      : t("admin.notifications.rules.noEventSelected");
  });

  const dialogModeBadgeLabel = computed(() =>
    isEditMode.value
      ? t("admin.notifications.rules.modeEdit")
      : t("admin.notifications.rules.modeCreate"),
  );

  const dialogSelectionBadgeLabel = computed(() =>
    isEditMode.value
      ? t("admin.notifications.rules.selectionEvent", {
          event: lockedEventTypeLabel.value,
        })
      : t("admin.notifications.rules.selectedEvents", {
          count: selectedEventTypeCount.value,
        }),
  );

  const dialogTargetsBadgeLabel = computed(() =>
    ruleForm.value.targets.length > 0
      ? t("admin.notifications.rules.targetsCount", {
          count: ruleForm.value.targets.length,
        })
      : t("admin.notifications.rules.noTargetsYet"),
  );

  const groupByHint = computed(() => {
    if (ruleForm.value.group_by !== "auto") {
      return "";
    }

    if (ruleForm.value.event_types.length === 1) {
      const onlyEventType = ruleForm.value.event_types[0]!;
      return t("admin.notifications.rules.groupByAutoHintSingle", {
        group: formatGroupByLabel(
          DEFAULT_GROUP_BY_BY_EVENT_TYPE[onlyEventType],
        ),
      });
    }

    if (ruleForm.value.event_types.length > 1) {
      return t("admin.notifications.rules.groupByAutoHintBatch");
    }

    return "";
  });

  const createTarget = (
    providerId = providers.value[0]?.id || "",
  ): EditableRuleTarget => {
    const definition = resolveProviderDefinitionById(providerId);
    return {
      provider_id: providerId,
      target_config: definition
        ? createEditableSchemaRecord(definition.target_schema)
        : {},
      delivery_policy: createEmptyDeliveryPolicy(),
      template_override_mode: "inherit",
      template_override: null,
    };
  };

  const resetRuleForm = () => {
    ruleForm.value = createEmptyRuleForm(availableEventTypes.value);
  };

  const openCreateDialog = () => {
    dialogMode.value = "create";
    editingRule.value = null;
    resetRuleForm();
    dialogOpen.value = true;
  };

  const getCreateRuleUnavailableTip = () => {
    if (!hasProviders.value) {
      return {
        title: t("admin.notifications.rules.createUnavailableTitle"),
        description: t(
          "admin.notifications.rules.createUnavailableDescription",
        ),
      };
    }

    if (!hasAvailableEventTypes.value) {
      return {
        title: t("admin.notifications.rules.noCreateAvailableTitle"),
        description: t(
          "admin.notifications.rules.noCreateAvailableDescription",
        ),
      };
    }

    return null;
  };

  const handleCreateRuleClick = () => {
    const unavailableTip = getCreateRuleUnavailableTip();
    if (unavailableTip) {
      toast.info(unavailableTip.title, {
        description: unavailableTip.description,
      });
      return;
    }

    openCreateDialog();
  };

  const openEditDialog = (rule: NotificationRule) => {
    dialogMode.value = "edit";
    editingRule.value = rule;
    ruleForm.value = {
      event_types: [rule.event_type],
      window_seconds: String(rule.window_seconds),
      threshold_count: String(rule.threshold_count),
      group_by: rule.group_by,
      cooldown_seconds: String(rule.cooldown_seconds),
      targets: rule.targets.map((target) => {
        const definition = resolveProviderDefinitionById(target.provider_id);
        return {
          id: target.id,
          provider_id: target.provider_id,
          target_config: definition
            ? createEditableSchemaRecord(
                definition.target_schema,
                target.target_config,
              )
            : {},
          delivery_policy: {
            timeout_seconds: String(
              target.delivery_policy?.timeout_seconds ?? "",
            ),
            max_attempts: String(target.delivery_policy?.max_attempts ?? ""),
            backoff_seconds: String(
              target.delivery_policy?.backoff_seconds ?? "",
            ),
          },
          template_override_mode: target.template_override_mode || "inherit",
          template_override: target.template_override ?? null,
        };
      }),
    };
    dialogOpen.value = true;
  };

  const addTarget = (providerId = providers.value[0]?.id || "") => {
    if (!providerId) return;
    if (selectedTargetProviderIds.value.has(providerId)) {
      toast.info(t("admin.notifications.rules.providerAlreadyAdded"));
      return;
    }
    ruleForm.value.targets.push(createTarget(providerId));
  };

  const removeTarget = (index: number) => {
    ruleForm.value.targets.splice(index, 1);
  };

  const toggleAllEventTypes = (checked: unknown) => {
    if (checked) {
      ruleForm.value.event_types = [...availableEventTypes.value];
      return;
    }
    ruleForm.value.event_types = [];
  };

  const toggleEventType = (eventType: SystemEventType, checked: unknown) => {
    const nextChecked = Boolean(checked);
    const nextSelection = new Set(ruleForm.value.event_types);

    if (nextChecked) {
      nextSelection.add(eventType);
    } else {
      nextSelection.delete(eventType);
    }

    ruleForm.value.event_types = availableEventTypes.value.filter((type) =>
      nextSelection.has(type),
    );
  };

  const saveRule = async () => {
    if (!ruleForm.value.targets.length) {
      toast.error(t("admin.notifications.rules.missingTargets"));
      return;
    }

    const selectedEventTypes = ruleForm.value.event_types;
    if (!selectedEventTypes.length) {
      toast.error(t("admin.notifications.rules.missingEvents"));
      return;
    }

    if (dialogMode.value === "edit" && selectedEventTypes.length !== 1) {
      toast.error(t("admin.notifications.rules.editOneEventOnly"));
      return;
    }

    saving.value = true;
    try {
      const currentRuleForm = ruleForm.value;
      if (dialogMode.value === "create") {
        const batchPlans = selectedEventTypes.map((eventType) => {
          return {
            eventType,
            name: buildRuleDisplayName(eventType),
            groupBy: resolveGroupByForEventType({
              eventType,
              form: currentRuleForm,
            }),
          };
        });

        const results = await Promise.allSettled(
          batchPlans.map(async (plan) => {
            const result = await EventCenterAPI.createNotificationRule(
              buildRulePayload({
                eventType: plan.eventType,
                form: currentRuleForm,
                groupBy: plan.groupBy,
                resolveProviderDefinitionById,
              }),
            );

            if (!result.success) {
              throw new Error(
                result.message ||
                  t("admin.notifications.rules.createRuleNamedFailed", {
                    name: plan.name,
                  }),
              );
            }

            return plan.name;
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
              : t("admin.notifications.rules.createRulesFailed"),
          );

        if (failed.length === results.length) {
          throw new Error(
            failed[0] || t("admin.notifications.rules.createRulesFailed"),
          );
        }

        if (failed.length > 0) {
          toast.info(
            t("admin.notifications.rules.createPartial", {
              success: succeeded.length,
              failed: failed.length,
            }),
            {
              description: failed[0],
            },
          );
        } else {
          toast.success(
            succeeded.length > 1
              ? t("admin.notifications.rules.createCount", {
                  count: succeeded.length,
                })
              : t("admin.notifications.rules.createOne"),
          );
        }

        dialogOpen.value = false;
        await loadData();
        return;
      }

      const eventType = selectedEventTypes[0]!;
      const result = await EventCenterAPI.updateNotificationRule(
        editingRule.value!.id,
        buildRulePayload({
          eventType,
          form: currentRuleForm,
          groupBy: resolveGroupByForEventType({
            eventType,
            form: currentRuleForm,
          }),
          resolveProviderDefinitionById,
        }),
      );

      if (!result.success) {
        throw new Error(
          result.message || t("admin.notifications.rules.updateRuleFailed"),
        );
      }

      toast.success(t("admin.notifications.rules.updated"));
      dialogOpen.value = false;
      await loadData();
    } catch (error) {
      toast.error(
        dialogMode.value === "create"
          ? t("admin.notifications.rules.createFailed")
          : t("admin.notifications.rules.updateFailed"),
        {
          description:
            error instanceof Error ? error.message : t("common.tryLater"),
        },
      );
    } finally {
      saving.value = false;
    }
  };

  return {
    dialogOpen,
    saving,
    ruleForm,
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
    handleCreateRuleClick,
    openEditDialog,
    addTarget,
    removeTarget,
    toggleAllEventTypes,
    toggleEventType,
    saveRule,
  };
}
