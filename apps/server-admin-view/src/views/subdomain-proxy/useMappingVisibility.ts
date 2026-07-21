import { computed, ref, watch, type ComputedRef, type Ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { parseCidrTextarea } from "@admin-shared/utils/cidr";
import { ConfigAPI } from "@/lib/api";
import type { GatewayVisibilitySelection, HostMapping } from "@/types";
import type { TranslationParams } from "./model";

export type MappingDialogView = "basic" | "icon" | "visibility";
export type MappingDialogMotionDirection = "forward" | "back";

export type MappingVisibilityValidationIssue =
  | { kind: "invalid_cidrs"; invalid: string[] }
  | { kind: "empty" }
  | null;

export const shouldBlockMappingSaveForVisibility = ({
  isAuthService,
  isLoading,
  loadError,
  validationMessage,
}: {
  isAuthService: boolean;
  isLoading: boolean;
  loadError: string;
  validationMessage: string;
}): boolean =>
  !isAuthService &&
  (isLoading || Boolean(loadError) || Boolean(validationMessage));

export const shouldReturnToVisibilityAfterSaveError = (
  mode: HostMapping["visibility"]["mode"],
): boolean => mode !== "inherit";

export const getMappingVisibilityValidationIssue = ({
  available,
  mode,
  selectionsCount,
  customCidrsText,
}: {
  available: boolean;
  mode: HostMapping["visibility"]["mode"];
  selectionsCount: number;
  customCidrsText: string;
}): MappingVisibilityValidationIssue => {
  if (!available || mode !== "custom") return null;
  const parsed = parseCidrTextarea(customCidrsText);
  if (parsed.invalid.length > 0) {
    return { kind: "invalid_cidrs", invalid: parsed.invalid };
  }
  if (selectionsCount === 0 && parsed.cidrs.length === 0) {
    return { kind: "empty" };
  }
  return null;
};

export const useMappingVisibility = ({
  isDialogOpen,
  isMappingAuthService,
  mappingForm,
  translate,
}: {
  isDialogOpen: Ref<boolean>;
  isMappingAuthService: ComputedRef<boolean>;
  mappingForm: HostMapping;
  translate: (key: string, params?: TranslationParams) => string;
}) => {
  const mappingDialogView = ref<MappingDialogView>("basic");
  const motionDirection = ref<MappingDialogMotionDirection>("forward");
  const globalVisibilityEnabled = ref(false);
  const isGlobalVisibilityLoading = ref(false);
  const globalVisibilityLoadError = ref("");
  const customCidrsText = ref("");
  let globalRequestId = 0;

  const visibilityMode = computed({
    get: () => {
      const mode = mappingForm.visibility?.mode;
      return mode === "custom" || mode === "disabled" ? mode : "inherit";
    },
    set: (mode: HostMapping["visibility"]["mode"]) => {
      mappingForm.visibility.mode = mode;
    },
  });
  const selections = computed<GatewayVisibilitySelection[]>({
    get: () => mappingForm.visibility.selections,
    set: (value) => {
      mappingForm.visibility.selections = value;
    },
  });
  const customCidrsState = computed(() =>
    parseCidrTextarea(customCidrsText.value),
  );
  const visibilityAvailable = computed(
    () => globalVisibilityEnabled.value && !isMappingAuthService.value,
  );
  const customVisibilityEnabled = computed(
    () => visibilityMode.value === "custom",
  );
  const regionInputsDisabled = computed(() => !customVisibilityEnabled.value);
  const visibilityValidationMessage = computed(() => {
    const issue = getMappingVisibilityValidationIssue({
      available: visibilityAvailable.value,
      mode: visibilityMode.value,
      selectionsCount: selections.value.length,
      customCidrsText: customCidrsText.value,
    });
    if (issue?.kind === "invalid_cidrs") {
      return translate("admin.subdomainProxy.visibilityInvalidCidrs", {
        items: issue.invalid.join("、"),
      });
    }
    if (issue?.kind === "empty") {
      return translate("admin.subdomainProxy.visibilityRuleRequired");
    }
    return "";
  });
  const visibilitySummary = computed(() => {
    if (visibilityMode.value === "disabled") {
      return translate("admin.subdomainProxy.visibilityDisabledSummary");
    }
    if (visibilityMode.value === "inherit") {
      return translate("admin.subdomainProxy.visibilityInherit");
    }
    return translate("admin.subdomainProxy.visibilityCustomSummary", {
      regions: selections.value.length,
      cidrs: customCidrsState.value.cidrs.length,
    });
  });

  const syncCustomCidrsToForm = () => {
    mappingForm.visibility.custom_cidrs = customCidrsText.value
      .split(/\r?\n/u)
      .map((value) => value.trim())
      .filter(Boolean);
  };

  const resetVisibilityEditor = () => {
    mappingDialogView.value = "basic";
    motionDirection.value = "forward";
    globalVisibilityEnabled.value = false;
    customCidrsText.value = (mappingForm.visibility?.custom_cidrs ?? []).join(
      "\n",
    );
    globalVisibilityLoadError.value = "";
  };

  const loadGlobalVisibility = async () => {
    const requestId = ++globalRequestId;
    globalVisibilityEnabled.value = false;
    isGlobalVisibilityLoading.value = true;
    globalVisibilityLoadError.value = "";
    try {
      const details = await ConfigAPI.getGatewayVisibility();
      if (requestId !== globalRequestId) return;
      globalVisibilityEnabled.value = details.config.enabled;
    } catch (error) {
      if (requestId !== globalRequestId) return;
      globalVisibilityEnabled.value = false;
      globalVisibilityLoadError.value = extractErrorMessage(
        error,
        translate("admin.subdomainProxy.visibilityLoadFailed"),
      );
    } finally {
      if (requestId === globalRequestId) {
        isGlobalVisibilityLoading.value = false;
      }
    }
  };

  const openVisibilityView = () => {
    if (!visibilityAvailable.value) return;
    motionDirection.value = "forward";
    mappingDialogView.value = "visibility";
  };

  const openIconView = () => {
    motionDirection.value = "forward";
    mappingDialogView.value = "icon";
  };

  const returnBasicView = () => {
    motionDirection.value = "back";
    mappingDialogView.value = "basic";
  };

  watch(customCidrsText, syncCustomCidrsToForm);
  watch(isMappingAuthService, (isAuth) => {
    if (isAuth && mappingDialogView.value === "visibility") {
      returnBasicView();
    }
  });
  watch(isDialogOpen, (open) => {
    if (!open) {
      globalRequestId += 1;
      isGlobalVisibilityLoading.value = false;
    }
  });

  const transitionEnterFromClass = computed(() =>
    motionDirection.value === "forward"
      ? "opacity-0 motion-safe:translate-x-6"
      : "opacity-0 motion-safe:-translate-x-6",
  );
  const transitionLeaveToClass = computed(() =>
    motionDirection.value === "forward"
      ? "opacity-0 motion-safe:-translate-x-6"
      : "opacity-0 motion-safe:translate-x-6",
  );

  return {
    customCidrsState,
    customCidrsText,
    globalVisibilityLoadError,
    isGlobalVisibilityLoading,
    loadGlobalVisibility,
    mappingDialogView,
    openIconView,
    openVisibilityView,
    regionInputsDisabled,
    resetVisibilityEditor,
    returnBasicView,
    transitionEnterFromClass,
    transitionLeaveToClass,
    visibilityAvailable,
    visibilityMode,
    visibilitySummary,
    visibilityValidationMessage,
  };
};
