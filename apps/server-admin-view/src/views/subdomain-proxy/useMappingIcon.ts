import { computed, ref, watch, type ComputedRef, type Ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "@/lib/api";
import type { HostMapping } from "@/types";
import {
  getMappingFaviconSource,
  getMappingFaviconSrc,
  type TranslationParams,
} from "./model";
import {
  MappingIconProcessingError,
  processMappingIconFile,
} from "./mapping-icon";

export const useMappingIcon = ({
  canRefreshMetadata,
  getMetadataBasicAuth,
  isDialogOpen,
  mappingForm,
  metadataTarget,
  resetFaviconErrors,
  translate,
}: {
  canRefreshMetadata: ComputedRef<boolean>;
  getMetadataBasicAuth: () => HostMapping["basic_auth"] | null;
  isDialogOpen: Ref<boolean>;
  mappingForm: HostMapping;
  metadataTarget: Ref<string>;
  resetFaviconErrors: () => void;
  translate: (key: string, params?: TranslationParams) => string;
}) => {
  const isRefreshingFavicon = ref(false);
  const isProcessingFavicon = ref(false);
  const faviconRefreshDirty = ref(false);
  const iconErrorMessage = ref("");
  let refreshRequestId = 0;
  let uploadRequestId = 0;

  const hasFreshAutoFavicon = computed(
    () => metadataTarget.value === mappingForm.target.trim(),
  );
  const iconPresentationMapping = computed<HostMapping>(() => ({
    ...mappingForm,
    favicon: hasFreshAutoFavicon.value ? mappingForm.favicon : "",
  }));
  const faviconSource = computed(() =>
    getMappingFaviconSource(iconPresentationMapping.value),
  );
  const effectiveFaviconSrc = computed(() =>
    getMappingFaviconSrc(iconPresentationMapping.value),
  );
  const faviconSummary = computed(() =>
    translate(`admin.subdomainProxy.iconSource.${faviconSource.value}`),
  );
  const isIconBusy = computed(
    () => isRefreshingFavicon.value || isProcessingFavicon.value,
  );

  const refreshAutomaticFavicon = async (): Promise<boolean> => {
    if (!canRefreshMetadata.value || isRefreshingFavicon.value) return false;
    const requestId = ++refreshRequestId;
    const target = mappingForm.target.trim();
    isRefreshingFavicon.value = true;
    iconErrorMessage.value = "";
    try {
      const metadata = await ConfigAPI.fetchHostMappingMetadata(
        target,
        getMetadataBasicAuth(),
      );
      if (requestId !== refreshRequestId) return false;
      const favicon = metadata.favicon.trim();
      if (favicon) {
        mappingForm.favicon = favicon;
        metadataTarget.value = target;
        faviconRefreshDirty.value = true;
      } else if (metadataTarget.value !== target) {
        mappingForm.favicon = "";
        metadataTarget.value = target;
        faviconRefreshDirty.value = true;
      }
      resetFaviconErrors();
      if (favicon) {
        toast.success(translate("admin.subdomainProxy.iconCollected"));
      } else {
        iconErrorMessage.value = translate("admin.subdomainProxy.iconNotFound");
      }
      return true;
    } catch (error) {
      if (requestId !== refreshRequestId) return false;
      iconErrorMessage.value = extractErrorMessage(
        error,
        translate("admin.subdomainProxy.iconCollectFailed"),
      );
      return false;
    } finally {
      if (requestId === refreshRequestId) {
        isRefreshingFavicon.value = false;
      }
    }
  };

  const restoreAutomaticFavicon = async () => {
    mappingForm.favicon_override = "";
    resetFaviconErrors();
    await refreshAutomaticFavicon();
  };

  const uploadCustomFavicon = async (file: File) => {
    if (isIconBusy.value) return;
    const requestId = ++uploadRequestId;
    isProcessingFavicon.value = true;
    iconErrorMessage.value = "";
    try {
      const favicon = await processMappingIconFile(file);
      if (requestId !== uploadRequestId) return;
      mappingForm.favicon_override = favicon;
      resetFaviconErrors();
    } catch (error) {
      if (requestId !== uploadRequestId) return;
      const kind =
        error instanceof MappingIconProcessingError
          ? error.kind
          : "decode_failed";
      iconErrorMessage.value = translate(
        `admin.subdomainProxy.iconUploadErrors.${kind}`,
      );
    } finally {
      if (requestId === uploadRequestId) {
        isProcessingFavicon.value = false;
      }
    }
  };

  const resetIconEditor = () => {
    refreshRequestId += 1;
    uploadRequestId += 1;
    isRefreshingFavicon.value = false;
    isProcessingFavicon.value = false;
    faviconRefreshDirty.value = false;
    iconErrorMessage.value = "";
  };

  watch(isDialogOpen, (open) => {
    if (!open) resetIconEditor();
  });

  return {
    canRefreshMetadata,
    effectiveFaviconSrc,
    faviconRefreshDirty,
    faviconSource,
    faviconSummary,
    iconErrorMessage,
    isIconBusy,
    isProcessingFavicon,
    isRefreshingFavicon,
    refreshAutomaticFavicon,
    resetIconEditor,
    restoreAutomaticFavicon,
    uploadCustomFavicon,
  };
};
