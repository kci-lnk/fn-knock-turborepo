import type { ComputedRef, Ref } from "vue";
import { toast } from "@admin-shared/utils/toast";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import type {
  DDNSTargetDetailPayload,
  DDNSTargetSummaryPayload,
} from "@/lib/api";
import {
  INTERFACE_IPV4_INDEX_KEY,
  INTERFACE_IPV4_SELECTOR_KEY,
  INTERFACE_IPV6_INDEX_KEY,
  INTERFACE_IPV6_SELECTOR_KEY,
  NETWORK_INTERFACE_KEY,
  extractCommonTargetConfig,
  normalizeTargetConfigValues,
  validateDDNSTargetConfig,
  type DDNSAddressOption,
  type DDNSUpdateScope,
  type DDNSValidationIssue,
  type Provider,
  type TargetDialogState,
} from "./model";

type TargetDialogMode = "create" | "edit";
type Translate = (key: string, params?: Record<string, unknown>) => string;
type AsyncActionRunner = <T>(
  action: () => Promise<T>,
  hooks?: {
    onSuccess?: (result: T) => void | Promise<void>;
    onFinally?: () => void;
  },
) => Promise<T | undefined>;

type TargetPayload = {
  name?: string;
  provider: string;
  enabled: boolean;
  config: Record<string, string>;
};

type TargetActionsApi = {
  createTarget: (payload: TargetPayload) => Promise<unknown>;
  deleteTarget: (targetId: string) => Promise<unknown>;
  getTarget: (targetId: string) => Promise<DDNSTargetDetailPayload>;
  setTargetEnabled: (targetId: string, enabled: boolean) => Promise<unknown>;
  testTarget: (
    targetId: string,
  ) => Promise<{ success: boolean; message?: string }>;
  updateTarget: (targetId: string, payload: TargetPayload) => Promise<unknown>;
};

export const useDDNSTargetActions = ({
  api,
  loadStatus,
  providerConfig,
  providers,
  refreshPolling,
  resetTargetFieldVisibility,
  runDeleteTarget,
  runSaveTarget,
  runTestTarget,
  runToggleTarget,
  selectedProvider,
  showTargetDialog,
  showValidationIssue,
  targetDialogIPv4Options,
  targetDialogIPv6Options,
  targetDialogMode,
  targetDialogProviderDef,
  targetDialogState,
  targetDialogUpdateScope,
  deletingTargetId,
  testingTargetId,
  togglingTargetId,
  translate,
  normalizeDomainForSubmit,
}: {
  api: TargetActionsApi;
  loadStatus: () => Promise<void>;
  providerConfig: Ref<Record<string, string>>;
  providers: Ref<Provider[]>;
  refreshPolling: () => void;
  resetTargetFieldVisibility: () => void;
  runDeleteTarget: AsyncActionRunner;
  runSaveTarget: AsyncActionRunner;
  runTestTarget: AsyncActionRunner;
  runToggleTarget: AsyncActionRunner;
  selectedProvider: Ref<string>;
  showTargetDialog: Ref<boolean>;
  showValidationIssue: (issue: DDNSValidationIssue | null) => boolean;
  targetDialogIPv4Options: ComputedRef<DDNSAddressOption[]>;
  targetDialogIPv6Options: ComputedRef<DDNSAddressOption[]>;
  targetDialogMode: Ref<TargetDialogMode>;
  targetDialogProviderDef: ComputedRef<Provider | null>;
  targetDialogState: Ref<TargetDialogState>;
  targetDialogUpdateScope: ComputedRef<DDNSUpdateScope>;
  deletingTargetId: Ref<string>;
  testingTargetId: Ref<string>;
  togglingTargetId: Ref<string>;
  translate: Translate;
  normalizeDomainForSubmit: () => void;
}) => {
  const resetTargetDialogState = (next?: Partial<TargetDialogState>) => {
    resetTargetFieldVisibility();
    targetDialogState.value = {
      id: next?.id ?? null,
      name: next?.name ?? "",
      enabled: next?.enabled ?? true,
      provider: next?.provider ?? selectedProvider.value,
      config: normalizeTargetConfigValues(
        next?.config ?? extractCommonTargetConfig(providerConfig.value),
      ),
      lastIP: next?.lastIP ?? { ipv4: null, ipv6: null },
      selectionAnchor: next?.selectionAnchor ??
        next?.lastIP ?? {
          ipv4: null,
          ipv6: null,
        },
    };
  };

  const openCreateTargetDialog = () => {
    targetDialogMode.value = "create";
    resetTargetDialogState({
      provider: selectedProvider.value,
      enabled: true,
    });
    showTargetDialog.value = true;
  };

  const applyTargetDetailToDialog = (detail: DDNSTargetDetailPayload) => {
    targetDialogMode.value = "edit";
    resetTargetDialogState({
      id: detail.id,
      name: detail.rawName || "",
      enabled: detail.enabled,
      provider: detail.provider || "",
      config: detail.config,
      lastIP: detail.lastIP,
      selectionAnchor: detail.selectionAnchor,
    });
    showTargetDialog.value = true;
  };

  const openEditTargetDialog = async (targetId: string) => {
    try {
      const detail = await api.getTarget(targetId);
      applyTargetDetailToDialog(detail);
    } catch (error) {
      toast.error(translate("admin.ddns.loadTargetFailed"), {
        description: extractErrorMessage(
          error,
          translate("admin.ddns.loadTargetFailed"),
        ),
      });
    }
  };

  const updateTargetDialogNetworkInterface = (value: string) => {
    targetDialogState.value.config = {
      ...targetDialogState.value.config,
      [NETWORK_INTERFACE_KEY]: value,
      [INTERFACE_IPV4_INDEX_KEY]: "",
      [INTERFACE_IPV6_INDEX_KEY]: "",
      [INTERFACE_IPV4_SELECTOR_KEY]: "",
      [INTERFACE_IPV6_SELECTOR_KEY]: "",
    };
  };

  const handleTargetDialogProviderChange = (value: string) => {
    resetTargetFieldVisibility();
    targetDialogState.value.provider = value;
    targetDialogState.value.config = normalizeTargetConfigValues(
      extractCommonTargetConfig(targetDialogState.value.config),
    );
  };

  const validateTargetDialogConfig = () => {
    const provider = targetDialogState.value.provider.trim();
    const issue = validateDDNSTargetConfig({
      config: targetDialogState.value.config,
      ipv4Options: targetDialogIPv4Options.value,
      ipv6Options: targetDialogIPv6Options.value,
      provider,
      providerDef: targetDialogProviderDef.value,
      providers: providers.value,
      updateScope: targetDialogUpdateScope.value,
    });

    return !showValidationIssue(issue);
  };

  const saveTargetDialog = async () => {
    normalizeDomainForSubmit();
    if (!validateTargetDialogConfig()) {
      return;
    }

    const payload = {
      name: targetDialogState.value.name.trim() || undefined,
      provider: targetDialogState.value.provider,
      enabled: targetDialogState.value.enabled,
      config: { ...targetDialogState.value.config },
    };

    await runSaveTarget(
      async () => {
        if (targetDialogMode.value === "edit" && targetDialogState.value.id) {
          await api.updateTarget(targetDialogState.value.id, payload);
          return;
        }

        await api.createTarget(payload);
      },
      {
        onSuccess: async () => {
          showTargetDialog.value = false;
          toast.success(
            targetDialogMode.value === "create"
              ? translate("admin.ddns.targetCreated")
              : translate("admin.ddns.targetUpdated"),
          );
          await loadStatus();
          refreshPolling();
        },
      },
    );
  };

  const onTestExtraTarget = async (target: DDNSTargetSummaryPayload) => {
    testingTargetId.value = target.id;
    await runTestTarget(
      async () => {
        const result = await api.testTarget(target.id);
        if (result.success) {
          toast.success(translate("admin.ddns.targetUpdateSuccess"));
        } else {
          toast.error(translate("admin.ddns.testTargetFailed"), {
            description: result.message,
          });
        }
      },
      {
        onFinally: async () => {
          testingTargetId.value = "";
          await loadStatus();
        },
      },
    );
  };

  const onToggleExtraTarget = async (
    target: DDNSTargetSummaryPayload,
    enabled: boolean,
  ) => {
    togglingTargetId.value = target.id;
    await runToggleTarget(
      async () => {
        await api.setTargetEnabled(target.id, enabled);
      },
      {
        onSuccess: async () => {
          toast.success(
            enabled
              ? translate("admin.ddns.targetEnabled")
              : translate("admin.ddns.targetDisabled"),
          );
          await loadStatus();
        },
        onFinally: () => {
          togglingTargetId.value = "";
        },
      },
    );
  };

  const onDeleteExtraTarget = async (target: DDNSTargetSummaryPayload) => {
    deletingTargetId.value = target.id;
    await runDeleteTarget(
      async () => {
        await api.deleteTarget(target.id);
      },
      {
        onSuccess: async () => {
          toast.success(translate("admin.ddns.targetDeleted"));
          await loadStatus();
        },
        onFinally: () => {
          deletingTargetId.value = "";
        },
      },
    );
  };

  return {
    handleTargetDialogProviderChange,
    onDeleteExtraTarget,
    onTestExtraTarget,
    onToggleExtraTarget,
    openCreateTargetDialog,
    openEditTargetDialog,
    saveTargetDialog,
    updateTargetDialogNetworkInterface,
  };
};
