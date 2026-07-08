import { computed, ref, type ComputedRef } from "vue";
import {
  composeHostFromSubdomain,
  extractSubdomainFromHost,
  normalizeHostLike,
  resolveMappingEditorState,
  type MappingInputMode,
  type TranslationParams,
} from "./model";

export const useSubdomainMappingDraft = ({
  canUseRootDomainSuffix,
  onSubdomainExtractionMiss,
  savedRootDomain,
  translate,
}: {
  canUseRootDomainSuffix: ComputedRef<boolean>;
  onSubdomainExtractionMiss: (domain: string) => void;
  savedRootDomain: ComputedRef<string>;
  translate: (key: string, params?: TranslationParams) => string;
}) => {
  const mappingInputMode = ref<MappingInputMode>("subdomain");
  const mappingSubdomain = ref("");

  const mappingModeDescription = computed(() => {
    if (mappingInputMode.value === "subdomain" && canUseRootDomainSuffix.value) {
      return translate("admin.subdomainProxy.subdomainModeDescription", {
        domain: savedRootDomain.value,
      });
    }

    if (canUseRootDomainSuffix.value) {
      return translate("admin.subdomainProxy.fullHostModeDescription", {
        domain: savedRootDomain.value,
      });
    }

    if (!savedRootDomain.value) {
      return translate("admin.subdomainProxy.suffixAfterSavingRoot");
    }

    return translate("admin.subdomainProxy.suffixAfterSavingChanges");
  });

  const mappingInputLabel = computed(() =>
    mappingInputMode.value === "subdomain"
      ? translate("admin.subdomainProxy.subdomainPrefix")
      : translate("admin.subdomainProxy.fullHost"),
  );

  const fullHostInputHint = computed(() => {
    if (canUseRootDomainSuffix.value) {
      return translate("admin.subdomainProxy.fullHostInputHintWithRoot", {
        domain: savedRootDomain.value,
      });
    }

    return translate("admin.subdomainProxy.fullHostInputHint");
  });

  const composedPreviewHost = computed(() => {
    if (mappingInputMode.value === "full_host") {
      return normalizeHostLike(mappingSubdomain.value) || "";
    }
    return composeHostFromSubdomain(
      mappingSubdomain.value,
      savedRootDomain.value,
    );
  });

  const mappingDraftHost = computed(() => composedPreviewHost.value);

  const setMappingSubdomain = (value: string) => {
    mappingSubdomain.value = value;
  };

  const setMappingInputMode = (nextMode: MappingInputMode) => {
    if (nextMode === "subdomain" && !canUseRootDomainSuffix.value) {
      mappingInputMode.value = "full_host";
      return;
    }

    if (nextMode === mappingInputMode.value) return;

    const currentValue = mappingSubdomain.value;
    if (nextMode === "full_host") {
      mappingSubdomain.value =
        mappingInputMode.value === "subdomain"
          ? composeHostFromSubdomain(currentValue, savedRootDomain.value) ||
            normalizeHostLike(currentValue)
          : normalizeHostLike(currentValue);
      mappingInputMode.value = "full_host";
      return;
    }

    const extractedSubdomain = extractSubdomainFromHost(
      currentValue,
      savedRootDomain.value,
    );

    mappingInputMode.value = "subdomain";
    mappingSubdomain.value = extractedSubdomain ?? "";

    if (currentValue.trim() && !extractedSubdomain) {
      onSubdomainExtractionMiss(savedRootDomain.value);
    }
  };

  const handleMappingInputModeChange = (nextMode: MappingInputMode) => {
    setMappingInputMode(nextMode);
  };

  const resetMappingDraftInput = () => {
    mappingInputMode.value = canUseRootDomainSuffix.value
      ? "subdomain"
      : "full_host";
    mappingSubdomain.value = "";
  };

  const setMappingDraftInputFromHost = (host: string) => {
    const editorState = resolveMappingEditorState(
      host,
      canUseRootDomainSuffix.value ? savedRootDomain.value : "",
    );
    mappingInputMode.value = editorState.mode;
    mappingSubdomain.value = editorState.value;
  };

  return {
    composedPreviewHost,
    fullHostInputHint,
    handleMappingInputModeChange,
    mappingDraftHost,
    mappingInputLabel,
    mappingInputMode,
    mappingModeDescription,
    mappingSubdomain,
    resetMappingDraftInput,
    setMappingDraftInputFromHost,
    setMappingSubdomain,
  };
};
