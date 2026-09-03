import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { toast } from "@admin-shared/utils/toast";
import { isAnySubdomainRoutingMode } from "../../../lib/reverse-proxy-submode";
import { useConfigStore } from "../../../store/config";
import type { HostLocation, HostMapping } from "../../../types";
import { isProxyHostMapping } from "../../../lib/host-mapping-target";
import {
  cloneLocation,
  DEFAULT_RESPONSE_CONTENT_TYPE,
  snapshotLocations,
} from "./gatewayLocationModel";
import { useGatewayLocationEditor } from "./useGatewayLocationEditor";

type HostMappingTitleInfo = Pick<HostMapping, "title" | "title_override">;

export function useGatewayLocationsPage() {
  const route = useRoute();
  const router = useRouter();
  const configStore = useConfigStore();
  const { t } = useI18n();
  const selectedHost = ref("");
  const isHostPickerOpen = ref(false);
  const draftLocations = ref<HostLocation[]>([]);

  const { isPending: isLoading, run: runLoad } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.gatewayLocationsSettings.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.gatewayLocationsSettings.loadDescription"),
        ),
      });
    },
  });
  const showLoadingSkeleton = useDelayedLoading(isLoading);
  const { isPending: isSaving, run: runSave } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.gatewayLocationsSettings.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.gatewayLocationsSettings.saveDescription"),
        ),
      });
    },
  });

  const availableMappings = computed(() =>
    (configStore.config?.host_mappings ?? []).filter(
      (mapping) =>
        mapping.service_role !== "auth" && isProxyHostMapping(mapping),
    ),
  );
  const selectedMapping = computed(
    () =>
      availableMappings.value.find(
        (mapping) => mapping.host === selectedHost.value,
      ) ?? null,
  );
  const isAvailable = computed(() =>
    isAnySubdomainRoutingMode(configStore.config),
  );
  const isMissingHost = computed(
    () => Boolean(selectedHost.value) && selectedMapping.value === null,
  );
  const isDirty = computed(() => {
    const saved = selectedMapping.value?.locations ?? [];
    return snapshotLocations(saved) !== snapshotLocations(draftLocations.value);
  });
  const indexedDraftLocations = computed(() =>
    draftLocations.value.map((location, index) => ({ location, index })),
  );
  const canSave = computed(
    () => Boolean(selectedMapping.value) && isDirty.value && !isSaving.value,
  );

  const getMappingDisplayTitle = (mapping?: HostMappingTitleInfo | null) =>
    mapping?.title_override.trim() || mapping?.title.trim() || "";
  const getMappingTitleForDisplay = (mapping?: HostMappingTitleInfo | null) =>
    getMappingDisplayTitle(mapping) || "-";

  const resetDraftFromSelected = () => {
    draftLocations.value = (selectedMapping.value?.locations ?? []).map(
      cloneLocation,
    );
  };

  const selectHost = (host: string) => {
    selectedHost.value = host;
    if (host) {
      void router.push(`/subdomains/${encodeURIComponent(host)}/paths`);
    } else {
      void router.push({ path: "/mappings", query: { tab: "subdomain" } });
    }
    resetDraftFromSelected();
  };

  const openHostPicker = () => {
    if (!isAvailable.value || availableMappings.value.length === 0) return;
    isHostPickerOpen.value = true;
  };
  const selectHostFromDialog = (host: string) => {
    selectHost(host);
    isHostPickerOpen.value = false;
  };
  const handleHostPickerOpenChange = (open: boolean) => {
    isHostPickerOpen.value = open;
  };

  const ensureSelectedHost = () => {
    const requestedHost =
      typeof route.params.host === "string" ? route.params.host.trim() : "";
    selectedHost.value = requestedHost;
    resetDraftFromSelected();
  };

  const persistLocations = async (locations: HostLocation[]) => {
    const host = selectedHost.value;
    if (!host || !selectedMapping.value) return false;
    const result = await runSave(
      () =>
        configStore.saveHostMappings(
          (configStore.config?.host_mappings ?? []).map((item) =>
            item.host === host
              ? { ...item, locations: locations.map(cloneLocation) }
              : item,
          ),
        ),
      {
        onSuccess: () => {
          resetDraftFromSelected();
          toast.success(t("admin.gatewayLocationsSettings.saved"));
        },
      },
    );
    return result !== undefined;
  };

  const editor = useGatewayLocationEditor({
    draftLocations,
    persistLocations,
  });
  const saveLocations = async () => {
    await persistLocations(draftLocations.value);
  };
  const formatAction = (location: HostLocation) =>
    location.action === "response"
      ? t("admin.gatewayLocationsSettings.fixedResponse")
      : t("admin.gatewayLocationsSettings.proxyAction");
  const formatTarget = (location: HostLocation) => {
    if (location.action === "response") {
      return `${location.response.status || 200} ${location.response.content_type || DEFAULT_RESPONSE_CONTENT_TYPE}`;
    }
    return location.target;
  };
  const formatAuthMode = (location: HostLocation) => {
    if (location.auth_mode === "public") {
      return t("admin.gatewayLocationsSettings.authPublic");
    }
    return selectedMapping.value?.use_auth
      ? t("admin.gatewayLocationsSettings.authInheritProtected")
      : t("admin.gatewayLocationsSettings.authInheritPublic");
  };
  const backToSubdomains = () =>
    void router.push({ path: "/mappings", query: { tab: "subdomain" } });

  watch(() => route.params.host, ensureSelectedHost);
  onMounted(async () => {
    if (!configStore.config) {
      await runLoad(() => configStore.loadConfig());
    }
    ensureSelectedHost();
  });

  return {
    ...editor,
    availableMappings,
    canSave,
    draftLocations,
    formatAction,
    formatAuthMode,
    formatTarget,
    getMappingTitleForDisplay,
    handleHostPickerOpenChange,
    indexedDraftLocations,
    isAvailable,
    isDirty,
    isHostPickerOpen,
    isLoading,
    isMissingHost,
    isSaving,
    openHostPicker,
    resetDraftFromSelected,
    saveLocations,
    backToSubdomains,
    selectedHost,
    selectedMapping,
    selectHostFromDialog,
    showLoadingSkeleton,
  };
}

export type GatewayLocationsPageController = ReturnType<
  typeof useGatewayLocationsPage
>;
