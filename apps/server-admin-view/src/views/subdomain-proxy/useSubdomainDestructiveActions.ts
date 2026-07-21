import type { ComputedRef, Ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import type { HostMapping, SubdomainModeConfig } from "@/types";
import {
  composeHostFromSubdomain,
  createDisabledMappingBasicAuth,
  createDefaultMappingVisibility,
  DEFAULT_ACCESS_MODE,
  DEFAULT_AUTH_SUBDOMAIN,
  DEFAULT_PROTOCOL_MODE,
  resolveDefaultAuthServiceTarget,
  type DeleteDialogState,
} from "./model";

type RunAsyncAction = <T>(action: () => Promise<T>) => Promise<T | undefined>;
type Translate = (
  key: string,
  params?: Record<string, string | number>,
) => string;

export const useSubdomainDestructiveActions = ({
  advanceClearAllConfirmation,
  allMappings,
  authServiceMapping,
  canManageNewMappings,
  closeDeleteDialog,
  currentModeConfig,
  deleteDialogState,
  isAuthServiceTarget,
  modeForm,
  openClearAllConfigDialogState,
  runSaveMappings,
  savedRootDomain,
  saveHostMappings,
  translate,
}: {
  advanceClearAllConfirmation: () => boolean;
  allMappings: ComputedRef<HostMapping[]>;
  authServiceMapping: ComputedRef<HostMapping | null>;
  canManageNewMappings: ComputedRef<boolean>;
  closeDeleteDialog: () => void;
  currentModeConfig: ComputedRef<SubdomainModeConfig>;
  deleteDialogState: Ref<DeleteDialogState | null>;
  isAuthServiceTarget: (target: string) => boolean;
  modeForm: SubdomainModeConfig;
  openClearAllConfigDialogState: () => void;
  runSaveMappings: RunAsyncAction;
  savedRootDomain: ComputedRef<string>;
  saveHostMappings: (mappings: HostMapping[]) => Promise<unknown>;
  translate: Translate;
}) => {
  const {
    isPending: isClearingAllSubdomainConfig,
    run: runClearAllSubdomainConfig,
  } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.subdomainProxy.clearFailed"), {
        description: extractErrorMessage(
          error,
          translate("admin.subdomainProxy.clearConfigFailed"),
        ),
      });
    },
  });

  const addAuthService = async () => {
    if (!canManageNewMappings.value) {
      toast.error(translate("admin.subdomainProxy.cannotAddAuthService"), {
        description: !savedRootDomain.value
          ? translate("admin.subdomainProxy.saveRootFirst")
          : translate("admin.subdomainProxy.rootDirtyAddAuth"),
      });
      return;
    }
    if (authServiceMapping.value) {
      toast.error(translate("admin.subdomainProxy.authServiceExists"), {
        description: translate(
          "admin.subdomainProxy.authServiceExistsDescription",
          { host: authServiceMapping.value.host },
        ),
      });
      return;
    }

    const host = composeHostFromSubdomain(
      DEFAULT_AUTH_SUBDOMAIN,
      savedRootDomain.value,
    );
    const target = resolveDefaultAuthServiceTarget(
      modeForm.auth_target,
      currentModeConfig.value.auth_target,
    );
    if (!host) {
      toast.error(translate("admin.subdomainProxy.defaultAuthGenerateFailed"), {
        description: translate("admin.subdomainProxy.confirmRootSaved"),
      });
      return;
    }
    if (allMappings.value.some((item) => item.host === host)) {
      toast.error(
        translate("admin.subdomainProxy.defaultAuthSubdomainExists"),
        {
          description: translate(
            "admin.subdomainProxy.defaultAuthSubdomainExistsDescription",
            { host },
          ),
        },
      );
      return;
    }

    await runSaveMappings(async () => {
      await saveHostMappings([
        ...allMappings.value,
        {
          host,
          target,
          waf_enabled: true,
          use_auth: false,
          access_mode: DEFAULT_ACCESS_MODE,
          suppress_toolbar: false,
          preserve_host: true,
          is_default: false,
          disabled: false,
          availability: null,
          protocol_mode: DEFAULT_PROTOCOL_MODE,
          basic_auth: createDisabledMappingBasicAuth(),
          visibility: createDefaultMappingVisibility(),
          locations: [],
          service_role: "auth",
          title: "",
          title_override: "",
          favicon: "",
          favicon_override: "",
        },
      ]);
      toast.success(translate("admin.subdomainProxy.authServiceAdded"), {
        description: `${host} -> ${target}`,
      });
    });
  };

  const openClearAllConfigDialog = () => {
    if (allMappings.value.length === 0) {
      toast.error(translate("admin.subdomainProxy.noClearableMappings"));
      return;
    }
    openClearAllConfigDialogState();
  };

  const removeAuthService = async (): Promise<boolean> => {
    if (!authServiceMapping.value) {
      toast.error(translate("admin.subdomainProxy.noCurrentAuthService"));
      return false;
    }
    const authHost = authServiceMapping.value.host;
    const removed = await runSaveMappings(async () => {
      await saveHostMappings(
        allMappings.value.filter((item) => !isAuthServiceTarget(item.target)),
      );
      toast.success(translate("admin.subdomainProxy.authServiceDeleted"), {
        description: authHost,
      });
      return true;
    });
    return removed === true;
  };

  const clearAllSubdomainConfig = async (): Promise<boolean> => {
    const mappingsCount = allMappings.value.length;
    const cleared = await runClearAllSubdomainConfig(async () => {
      await saveHostMappings([]);
      toast.success(translate("admin.subdomainProxy.allCleared"), {
        description:
          mappingsCount > 0
            ? translate("admin.subdomainProxy.clearedMappingsDescription", {
                count: mappingsCount,
              })
            : translate("admin.subdomainProxy.modeConfigKept"),
      });
      return true;
    });
    return cleared === true;
  };

  const removeMapping = async (host: string): Promise<boolean> => {
    if (!allMappings.value.some((item) => item.host === host)) return false;
    const removed = await runSaveMappings(async () => {
      await saveHostMappings(
        allMappings.value.filter((item) => item.host !== host),
      );
      toast.success(translate("admin.subdomainProxy.mappingDeleted"));
      return true;
    });
    return removed === true;
  };

  const confirmDelete = async () => {
    const target = deleteDialogState.value;
    if (!target) return;
    if (target.kind === "clear_all") {
      if (advanceClearAllConfirmation()) return;
      if (await clearAllSubdomainConfig()) closeDeleteDialog();
      return;
    }
    if (await removeMapping(target.host)) closeDeleteDialog();
  };

  return {
    addAuthService,
    confirmDelete,
    isClearingAllSubdomainConfig,
    openClearAllConfigDialog,
    removeAuthService,
  };
};
