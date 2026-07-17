import { computed, ref, watch, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import { EventCenterAPI } from "@/lib/api";
import type {
  NotificationProviderDefinition,
  NotificationProviderView,
} from "@/types";
import {
  buildNextSequentialName,
  buildSchemaPayload,
  createEditableSchemaRecord,
  type EditableProviderForm,
  type ProviderDialogMode,
  type ProviderFormPayload,
} from "./form-utils";

const PROVIDER_TAIL_ORDER = [
  "webhook",
  "wxpusher",
  "serverchan",
  "pushplus",
] as const;

const resolveProviderSortWeight = (type: string) => {
  const tailIndex = PROVIDER_TAIL_ORDER.indexOf(
    type as (typeof PROVIDER_TAIL_ORDER)[number],
  );
  return tailIndex === -1 ? 0 : 100 + tailIndex;
};

const sortProviderCatalog = (definitions: NotificationProviderDefinition[]) =>
  [...definitions].sort((left, right) => {
    const weightDiff =
      resolveProviderSortWeight(left.type) -
      resolveProviderSortWeight(right.type);
    return weightDiff !== 0 ? weightDiff : 0;
  });

export const useNotificationProviders = (active: Readonly<Ref<boolean>>) => {
  const { t } = useI18n();
  const catalog = ref<NotificationProviderDefinition[]>([]);
  const providers = ref<NotificationProviderView[]>([]);
  const loading = ref(false);
  const dialogOpen = ref(false);
  const dialogMode = ref<ProviderDialogMode>("create");
  const saving = ref(false);
  const testingDraft = ref(false);
  const deletingId = ref<string | null>(null);
  const testingId = ref<string | null>(null);
  const editingId = ref<string | null>(null);
  const editingProvider = ref<NotificationProviderView | null>(null);
  const providerForm = ref<EditableProviderForm>({
    name: "",
    type: "",
    enabled: true,
    connection_config: {},
  });

  const selectedDefinition = computed(
    () =>
      catalog.value.find((item) => item.type === providerForm.value.type) ||
      catalog.value[0] ||
      null,
  );
  const configuredSensitiveFields = computed(() => {
    if (!editingProvider.value || !selectedDefinition.value) return [];
    return selectedDefinition.value.connection_schema
      .filter(
        (field) =>
          field.sensitive &&
          Boolean(editingProvider.value?.connection_config_masked[field.key]),
      )
      .map((field) => field.key);
  });

  const resolveProviderTypeLabel = (type: string) =>
    catalog.value.find((item) => item.type === type)?.label || type;

  const buildGeneratedProviderName = (type: string) => {
    const baseLabel =
      catalog.value.find((item) => item.type === type)?.label ||
      resolveProviderTypeLabel(type) ||
      t("admin.notifications.providers.fallbackProviderLabel");
    return buildNextSequentialName(
      baseLabel,
      providers.value.map((provider) => provider.name),
      t("admin.notifications.providers.unnamed"),
    );
  };

  const generatedProviderName = computed(() =>
    buildGeneratedProviderName(providerForm.value.type),
  );

  const loadData = async () => {
    loading.value = true;
    try {
      const [catalogResult, providersResult] = await Promise.all([
        EventCenterAPI.getNotificationProviderCatalog(),
        EventCenterAPI.getNotificationProviders(),
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
      catalog.value = sortProviderCatalog(catalogResult.data.providers || []);
      providers.value = providersResult.data.providers || [];
      if (!providerForm.value.type && catalog.value[0]) {
        providerForm.value.type = catalog.value[0].type;
      }
    } catch (error) {
      toast.error(t("admin.notifications.providers.loadFailed"), {
        description:
          error instanceof Error ? error.message : t("common.tryLater"),
      });
    } finally {
      loading.value = false;
    }
  };

  const resetProviderForm = (definition = catalog.value[0] || null) => {
    const type = definition?.type || "webhook";
    providerForm.value = {
      name: buildGeneratedProviderName(type),
      type,
      enabled: true,
      connection_config: definition
        ? createEditableSchemaRecord(definition.connection_schema)
        : {},
    };
  };

  const openCreateDialog = () => {
    dialogMode.value = "create";
    editingProvider.value = null;
    resetProviderForm();
    dialogOpen.value = true;
  };

  const openEditDialog = async (provider: NotificationProviderView) => {
    editingId.value = provider.id;
    try {
      const result = await EventCenterAPI.getNotificationProvider(provider.id);
      if (!result.success) {
        throw new Error(
          result.message ||
            t("admin.notifications.providers.providerDetailsLoadFailed"),
        );
      }
      const providerDetail = result.data;
      const definition =
        catalog.value.find((item) => item.type === providerDetail.type) || null;
      dialogMode.value = "edit";
      editingProvider.value = providerDetail;
      providerForm.value = {
        name: providerDetail.name,
        type: providerDetail.type,
        enabled: providerDetail.enabled,
        connection_config: definition
          ? createEditableSchemaRecord(
              definition.connection_schema,
              providerDetail.connection_config,
            )
          : {},
      };
      dialogOpen.value = true;
    } catch (error) {
      toast.error(
        t("admin.notifications.providers.providerDetailsLoadFailed"),
        {
          description:
            error instanceof Error ? error.message : t("common.tryLater"),
        },
      );
    } finally {
      editingId.value = null;
    }
  };

  const handleTypeChange = (value: unknown) => {
    if (!value) return;
    const currentType = providerForm.value.type;
    const previousGeneratedName = buildGeneratedProviderName(currentType);
    const nextType = String(value);
    const nextDefinition =
      catalog.value.find((item) => item.type === nextType) || null;
    const shouldRefreshName =
      dialogMode.value === "create" &&
      (!providerForm.value.name.trim() ||
        providerForm.value.name === previousGeneratedName);
    providerForm.value = {
      ...providerForm.value,
      name: shouldRefreshName
        ? buildGeneratedProviderName(nextType)
        : providerForm.value.name,
      type: nextType,
      connection_config: nextDefinition
        ? createEditableSchemaRecord(nextDefinition.connection_schema)
        : {},
    };
  };

  const buildProviderPayload = (): ProviderFormPayload => {
    const definition = selectedDefinition.value!;
    const connectionConfig = buildSchemaPayload({
      fields: definition.connection_schema,
      value: providerForm.value.connection_config,
      editing: dialogMode.value === "edit",
      configuredSensitiveFields: configuredSensitiveFields.value,
    });
    if (dialogMode.value === "edit" && definition.type === "wxpusher") {
      for (const key of ["uids", "topic_ids", "url"]) {
        if (!(key in connectionConfig)) {
          connectionConfig[key] = String(
            providerForm.value.connection_config[key] ?? "",
          ).trim();
        }
      }
    }
    return {
      name: providerForm.value.name.trim() || undefined,
      type: providerForm.value.type,
      enabled: providerForm.value.enabled,
      connection_config: connectionConfig,
    };
  };

  const saveProvider = async () => {
    if (!selectedDefinition.value) {
      toast.error(t("admin.notifications.providers.unavailableProviderType"));
      return;
    }
    saving.value = true;
    try {
      const payload = buildProviderPayload();
      const result =
        dialogMode.value === "create"
          ? await EventCenterAPI.createNotificationProvider(payload)
          : await EventCenterAPI.updateNotificationProvider(
              editingProvider.value!.id,
              payload,
            );
      if (!result.success) {
        throw new Error(
          result.message ||
            (dialogMode.value === "create"
              ? t("admin.notifications.providers.createProviderFailed")
              : t("admin.notifications.providers.updateProviderFailed")),
        );
      }
      const savedName = String(
        result?.data?.name || payload.name || generatedProviderName.value,
      );
      toast.success(
        dialogMode.value === "create"
          ? t("admin.notifications.providers.createProviderSuccess", {
              name: savedName,
            })
          : t("admin.notifications.providers.updateProviderSuccess", {
              name: savedName,
            }),
      );
      dialogOpen.value = false;
      await loadData();
    } catch (error) {
      toast.error(
        dialogMode.value === "create"
          ? t("admin.notifications.providers.createFailed")
          : t("admin.notifications.providers.updateFailed"),
        {
          description:
            error instanceof Error ? error.message : t("common.tryLater"),
        },
      );
    } finally {
      saving.value = false;
    }
  };

  const testProviderDraft = async () => {
    if (!selectedDefinition.value) {
      toast.error(t("admin.notifications.providers.unavailableProviderType"));
      return;
    }
    testingDraft.value = true;
    try {
      const result = await EventCenterAPI.testNotificationProviderDraft({
        ...buildProviderPayload(),
        id: dialogMode.value === "edit" ? editingProvider.value?.id : undefined,
      });
      if (!result.success) {
        throw new Error(
          result.message || t("admin.notifications.providers.testSendFailed"),
        );
      }
      toast.success(t("admin.notifications.providers.testSendSuccess"));
    } catch (error) {
      toast.error(t("admin.notifications.providers.testSendFailed"), {
        description:
          error instanceof Error
            ? error.message
            : t("admin.notifications.providers.testDraftConfigHint"),
      });
    } finally {
      testingDraft.value = false;
    }
  };

  const deleteProvider = async (provider: NotificationProviderView) => {
    deletingId.value = provider.id;
    try {
      const result = await EventCenterAPI.deleteNotificationProvider(
        provider.id,
      );
      if (!result.success) {
        throw new Error(
          result.message ||
            t("admin.notifications.providers.deleteProviderFailed"),
        );
      }
      toast.success(t("admin.notifications.providers.deleteProviderSuccess"));
      await loadData();
    } catch (error) {
      toast.error(t("admin.notifications.providers.deleteProviderFailed"), {
        description:
          error instanceof Error ? error.message : t("common.tryLater"),
      });
    } finally {
      deletingId.value = null;
    }
  };

  const testProvider = async (provider: NotificationProviderView) => {
    testingId.value = provider.id;
    try {
      const result = await EventCenterAPI.testNotificationProvider(provider.id);
      if (!result.success) {
        throw new Error(
          result.message || t("admin.notifications.providers.testSendFailed"),
        );
      }
      toast.success(t("admin.notifications.providers.testSendSuccess"));
      await loadData();
    } catch (error) {
      toast.error(t("admin.notifications.providers.testSendFailed"), {
        description:
          error instanceof Error
            ? error.message
            : t("admin.notifications.providers.testProviderConfigHint"),
      });
    } finally {
      testingId.value = null;
    }
  };

  watch(
    active,
    (isActive) => {
      if (isActive) void loadData();
    },
    { immediate: true },
  );

  return {
    catalog,
    configuredSensitiveFields,
    deleteProvider,
    deletingId,
    dialogMode,
    dialogOpen,
    editingId,
    generatedProviderName,
    handleTypeChange,
    loadData,
    loading,
    openCreateDialog,
    openEditDialog,
    providerForm,
    providers,
    resolveProviderTypeLabel,
    saveProvider,
    saving,
    selectedDefinition,
    showWxPusherAlert: computed(
      () => selectedDefinition.value?.type === "wxpusher",
    ),
    testProvider,
    testProviderDraft,
    testingDraft,
    testingId,
  };
};
