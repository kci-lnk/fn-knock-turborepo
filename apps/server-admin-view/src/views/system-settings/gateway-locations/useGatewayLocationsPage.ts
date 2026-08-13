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
import {
  cloneLocation,
  DEFAULT_RESPONSE_CONTENT_TYPE,
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
      (mapping) => mapping.service_role !== "auth",
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
  const isDirty = computed(() => {
    const saved = selectedMapping.value?.locations ?? [];
    return JSON.stringify(saved) !== JSON.stringify(draftLocations.value);
  });
  const indexedDraftLocations = computed(() =>
    draftLocations.value.map((location, index) => ({ location, index })),
  );
  const canSave = computed(
    () => Boolean(selectedMapping.value) && isDirty.value && !isSaving.value,
  );

  const getMappingDisplayTitle = (
    mapping?: HostMappingTitleInfo | null,
  ) => mapping?.title_override.trim() || mapping?.title.trim() || "";
  const getMappingTitleForDisplay = (
    mapping?: HostMappingTitleInfo | null,
  ) => getMappingDisplayTitle(mapping) || "-";

  const resetDraftFromSelected = () => {
    draftLocations.value = (selectedMapping.value?.locations ?? []).map(
      cloneLocation,
    );
  };

  const selectHost = (host: string) => {
    selectedHost.value = host;
    void router.replace({
      path: "/system/gateway-locations",
      query: host ? { host } : {},
    });
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
      typeof route.query.host === "string" ? route.query.host.trim() : "";
    const hostExists = availableMappings.value.some(
      (mapping) => mapping.host === requestedHost,
    );
    selectedHost.value = hostExists
      ? requestedHost
      : (availableMappings.value[0]?.host ?? "");
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

  watch(() => route.query.host, ensureSelectedHost);
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
    formatTarget,
    getMappingTitleForDisplay,
    handleHostPickerOpenChange,
    indexedDraftLocations,
    isAvailable,
    isDirty,
    isHostPickerOpen,
    isLoading,
    isSaving,
    openHostPicker,
    resetDraftFromSelected,
    saveLocations,
    selectedHost,
    selectedMapping,
    selectHostFromDialog,
    showLoadingSkeleton,
  };
}

export type GatewayLocationsPageController = ReturnType<
  typeof useGatewayLocationsPage
>;
