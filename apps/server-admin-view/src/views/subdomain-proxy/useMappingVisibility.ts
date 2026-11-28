import { computed, ref, watch, type ComputedRef, type Ref } from "vue";
import { parseCidrTextarea } from "@admin-shared/utils/cidr";
import { useGatewayVisibilityStatus } from "@/composables/useGatewayVisibilityStatus";
import { getMappingVisibilityValidationIssue } from "@/lib/mapping-visibility";
import type { GatewayVisibilitySelection, HostMapping } from "@/types";
import type { TranslationParams } from "./model";

export { getMappingVisibilityValidationIssue } from "@/lib/mapping-visibility";

export type MappingDialogView =
  "basic" | "icon" | "path-browser" | "visibility";
export type MappingDialogMotionDirection = "forward" | "back";

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
  const {
    globalVisibilityEnabled,
    isGlobalVisibilityLoading,
    globalVisibilityLoadError,
    loadGlobalVisibilityStatus: loadGlobalVisibility,
    resetGlobalVisibilityStatus,
  } = useGatewayVisibilityStatus(() =>
    translate("admin.subdomainProxy.visibilityLoadFailed"),
  );
  const customCidrsText = ref("");

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
    resetGlobalVisibilityStatus();
    customCidrsText.value = (mappingForm.visibility?.custom_cidrs ?? []).join(
      "\n",
    );
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

  const openPathBrowserView = () => {
    motionDirection.value = "forward";
    mappingDialogView.value = "path-browser";
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
    if (!open) resetGlobalVisibilityStatus();
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
    customCidrsText,
    globalVisibilityLoadError,
    isGlobalVisibilityLoading,
    loadGlobalVisibility,
    mappingDialogView,
    openIconView,
    openPathBrowserView,
    openVisibilityView,
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
