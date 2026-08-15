import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI, type StreamServiceCatalog } from "@/lib/api/config";
import { useConfigStore } from "../../store/config";
import type { StreamMapping } from "../../types";
import { getMappingKey } from "./streamMappingModel";

export function useStreamMappingSecurity() {
  const configStore = useConfigStore();
  const { t } = useI18n();
  const probingMappingKey = ref<string | null>(null);
  const serviceCatalog = ref<StreamServiceCatalog | null>(null);
  const serviceProfileMapping = ref<StreamMapping | null>(null);
  const serviceProfileInitialServiceId = ref("");
  const isServiceProfileOpen = ref(false);
  const isSavingServiceProfile = ref(false);
  let probeSequence = 0;
  let serviceLoadSequence = 0;

  async function probeMapping(mapping: StreamMapping) {
    const request = ++probeSequence;
    probingMappingKey.value = getMappingKey(mapping);
    try {
      const result = await ConfigAPI.probeStreamMapping(mapping);
      const mappings = await configStore.refreshStreamMappingsOnly();
      if (request !== probeSequence) return;
      const refreshedMapping =
        mappings.find(
          (candidate) => getMappingKey(candidate) === getMappingKey(mapping),
        ) ?? mapping;
      if (
        ["http1", "tls"].includes(result.profile?.service_id ?? "") &&
        (result.profile?.metadata?.auth_probe_status ??
          result.profile?.metadata?.http_status) === "401" &&
        Boolean(result.profile?.metadata?.auth_scheme)
      ) {
        toast.warning(t("admin.streamMappings.probeAuthenticatedHttp"), {
          description: t(
            "admin.streamMappings.probeAuthenticatedHttpDescription",
          ),
        });
        await openServiceProfile(refreshedMapping, { preselectCurrent: false });
        return;
      }
      if (result.status === "verified") {
        toast.success(t("admin.streamMappings.probeVerified"), {
          description: result.profile?.service_id || undefined,
        });
      } else {
        toast.error(t("admin.streamMappings.probeUnverified"), {
          description: result.message || result.status,
        });
      }
    } catch (error: any) {
      if (request !== probeSequence) return;
      toast.error(t("admin.streamMappings.probeFailed"), {
        description: extractErrorMessage(error, t("common.tryLater")),
      });
    } finally {
      if (request === probeSequence) probingMappingKey.value = null;
    }
  }

  async function openServiceProfile(
    mapping: StreamMapping,
    options: { preselectCurrent?: boolean } = {},
  ) {
    const request = ++serviceLoadSequence;
    try {
      serviceCatalog.value ??= await ConfigAPI.getStreamServiceCatalog();
      if (request !== serviceLoadSequence) return;
      serviceProfileMapping.value = mapping;
      serviceProfileInitialServiceId.value =
        options.preselectCurrent === false
          ? ""
          : (mapping.service_profile?.service_id ?? "");
      isServiceProfileOpen.value = true;
    } catch (error: any) {
      if (request !== serviceLoadSequence) return;
      toast.error(t("admin.streamMappings.serviceCatalogFailed"), {
        description: extractErrorMessage(error, t("common.tryLater")),
      });
    }
  }

  async function confirmServiceProfile(serviceId: string) {
    const mapping = serviceProfileMapping.value;
    if (!mapping || isSavingServiceProfile.value) return;
    isSavingServiceProfile.value = true;
    try {
      await ConfigAPI.confirmStreamServiceProfile(mapping, serviceId);
      await configStore.refreshStreamMappingsOnly();
      isServiceProfileOpen.value = false;
      toast.success(t("admin.streamMappings.serviceConfirmed"));
    } catch (error: any) {
      toast.error(t("admin.streamMappings.serviceConfirmFailed"), {
        description: extractErrorMessage(error, t("common.tryLater")),
      });
    } finally {
      isSavingServiceProfile.value = false;
    }
  }

  async function clearServiceProfile() {
    const mapping = serviceProfileMapping.value;
    if (!mapping || isSavingServiceProfile.value) return;
    isSavingServiceProfile.value = true;
    try {
      await ConfigAPI.clearStreamServiceProfile(mapping);
      await configStore.refreshStreamMappingsOnly();
      isServiceProfileOpen.value = false;
      toast.success(t("admin.streamMappings.serviceCleared"));
    } catch (error: any) {
      toast.error(t("admin.streamMappings.serviceClearFailed"), {
        description: extractErrorMessage(error, t("common.tryLater")),
      });
    } finally {
      isSavingServiceProfile.value = false;
    }
  }

  function setServiceProfileOpen(open: boolean) {
    isServiceProfileOpen.value = open;
    if (!open) {
      serviceLoadSequence += 1;
      serviceProfileMapping.value = null;
      serviceProfileInitialServiceId.value = "";
    }
  }

  return {
    clearServiceProfile,
    confirmServiceProfile,
    isSavingServiceProfile,
    isServiceProfileOpen,
    openServiceProfile,
    probeMapping,
    probingMappingKey,
    setServiceProfileOpen,
    serviceCatalog,
    serviceProfileInitialServiceId,
    serviceProfileMapping,
  };
}
