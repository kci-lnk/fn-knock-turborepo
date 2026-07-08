import { computed, ref, type ComputedRef, type Ref } from "vue";
import { toast } from "@admin-shared/utils/toast";
import type { ButtonVariants } from "@/components/ui/button";
import type { HostMapping, HostMappingAvailability } from "@/types";
import {
  getAvailabilityWindowValidationError,
  type TranslationParams,
} from "./model";

type AsyncActionRun = <T>(action: () => Promise<T>) => Promise<T | undefined>;
type Translate = (key: string, params?: TranslationParams) => string;

const DEFAULT_START_TIME = "09:00";
const DEFAULT_END_TIME = "18:00";

export const useSubdomainAvailabilityActions = ({
  allMappings,
  formatHostWithAccessEntryPort,
  isAuthServiceTarget,
  isSavingMappings,
  runSaveMappings,
  saveHostMappings,
  translate,
}: {
  allMappings: ComputedRef<HostMapping[]>;
  formatHostWithAccessEntryPort: (host: string) => string;
  isAuthServiceTarget: (target: string) => boolean;
  isSavingMappings: Ref<boolean>;
  runSaveMappings: AsyncActionRun;
  saveHostMappings: (mappings: HostMapping[]) => Promise<unknown>;
  translate: Translate;
}) => {
  const toggleDialogMappingHost = ref<string | null>(null);
  const availabilityDialogMappingHost = ref<string | null>(null);
  const availabilityFormEnabled = ref(false);
  const availabilityFormStartTime = ref(DEFAULT_START_TIME);
  const availabilityFormEndTime = ref(DEFAULT_END_TIME);

  const findMappingByHost = (host: string | null) =>
    host
      ? (allMappings.value.find((mapping) => mapping.host === host) ?? null)
      : null;

  const toggleDialogMapping = computed(() =>
    findMappingByHost(toggleDialogMappingHost.value),
  );
  const availabilityDialogMapping = computed(() =>
    findMappingByHost(availabilityDialogMappingHost.value),
  );

  const isToggleDialogOpen = computed(() => toggleDialogMapping.value !== null);
  const isToggleEnabling = computed(
    () => toggleDialogMapping.value?.disabled === true,
  );
  const toggleDialogTitle = computed(() =>
    isToggleEnabling.value
      ? translate("admin.subdomainProxy.enableMappingTitle")
      : translate("admin.subdomainProxy.disableMappingTitle"),
  );
  const toggleDialogDescription = computed(() => {
    const mapping = toggleDialogMapping.value;
    const host = mapping ? formatHostWithAccessEntryPort(mapping.host) : "";
    return isToggleEnabling.value
      ? translate("admin.subdomainProxy.enableMappingDescription", { host })
      : translate("admin.subdomainProxy.disableMappingDescription", { host });
  });
  const toggleDialogConfirmLabel = computed(() =>
    isToggleEnabling.value
      ? translate("admin.subdomainProxy.confirmEnable")
      : translate("admin.subdomainProxy.confirmDisable"),
  );
  const toggleDialogConfirmVariant = computed<ButtonVariants["variant"]>(() =>
    isToggleEnabling.value ? "default" : "destructive",
  );

  const isAvailabilityDialogOpen = computed(
    () => availabilityDialogMapping.value !== null,
  );
  const availabilityDialogHostLabel = computed(() =>
    availabilityDialogMapping.value
      ? formatHostWithAccessEntryPort(availabilityDialogMapping.value.host)
      : "",
  );
  const availabilityValidationMessage = computed(() => {
    if (!availabilityFormEnabled.value) return "";
    const error = getAvailabilityWindowValidationError(
      availabilityFormStartTime.value.trim(),
      availabilityFormEndTime.value.trim(),
    );
    if (error === "invalid_time") {
      return translate("admin.subdomainProxy.availabilityInvalidTime");
    }
    if (error === "same_time") {
      return translate("admin.subdomainProxy.availabilitySameTimeInvalid");
    }
    return "";
  });

  const openToggleMappingDialog = (mapping: HostMapping) => {
    if (isSavingMappings.value || isAuthServiceTarget(mapping.target)) return;
    toggleDialogMappingHost.value = mapping.host;
  };

  const closeToggleDialog = () => {
    toggleDialogMappingHost.value = null;
  };

  const handleToggleDialogOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      closeToggleDialog();
    }
  };

  const confirmToggleMapping = async () => {
    const mapping = toggleDialogMapping.value;
    if (
      !mapping ||
      isSavingMappings.value ||
      isAuthServiceTarget(mapping.target)
    ) {
      return;
    }

    const nextDisabled = mapping.disabled !== true;
    await runSaveMappings(async () => {
      await saveHostMappings(
        allMappings.value.map((item) =>
          item.host === mapping.host
            ? { ...item, disabled: nextDisabled }
            : item,
        ),
      );
      toast.success(
        nextDisabled
          ? translate("admin.subdomainProxy.mappingDisabled")
          : translate("admin.subdomainProxy.mappingEnabled"),
        {
          description: formatHostWithAccessEntryPort(mapping.host),
        },
      );
      closeToggleDialog();
    });
  };

  const openAvailabilityDialog = (mapping: HostMapping) => {
    if (isSavingMappings.value || isAuthServiceTarget(mapping.target)) return;
    availabilityDialogMappingHost.value = mapping.host;
    availabilityFormEnabled.value = mapping.availability?.enabled === true;
    availabilityFormStartTime.value =
      mapping.availability?.start_time?.trim() || DEFAULT_START_TIME;
    availabilityFormEndTime.value =
      mapping.availability?.end_time?.trim() || DEFAULT_END_TIME;
  };

  const closeAvailabilityDialog = () => {
    availabilityDialogMappingHost.value = null;
    availabilityFormEnabled.value = false;
    availabilityFormStartTime.value = DEFAULT_START_TIME;
    availabilityFormEndTime.value = DEFAULT_END_TIME;
  };

  const handleAvailabilityDialogOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      closeAvailabilityDialog();
    }
  };

  const saveAvailabilityDialog = async () => {
    const mapping = availabilityDialogMapping.value;
    if (
      !mapping ||
      isSavingMappings.value ||
      isAuthServiceTarget(mapping.target) ||
      availabilityValidationMessage.value
    ) {
      return;
    }

    const nextAvailability: HostMappingAvailability | null =
      availabilityFormEnabled.value
        ? {
            enabled: true,
            start_time: availabilityFormStartTime.value.trim(),
            end_time: availabilityFormEndTime.value.trim(),
          }
        : null;

    await runSaveMappings(async () => {
      await saveHostMappings(
        allMappings.value.map((item) =>
          item.host === mapping.host
            ? { ...item, availability: nextAvailability }
            : item,
        ),
      );
      toast.success(
        nextAvailability
          ? translate("admin.subdomainProxy.availabilitySaved")
          : translate("admin.subdomainProxy.availabilityCleared"),
        {
          description: formatHostWithAccessEntryPort(mapping.host),
        },
      );
      closeAvailabilityDialog();
    });
  };

  return {
    availabilityDialogHostLabel,
    availabilityFormEnabled,
    availabilityFormEndTime,
    availabilityFormStartTime,
    availabilityValidationMessage,
    closeAvailabilityDialog,
    closeToggleDialog,
    confirmToggleMapping,
    handleAvailabilityDialogOpenChange,
    handleToggleDialogOpenChange,
    isAvailabilityDialogOpen,
    isToggleDialogOpen,
    openAvailabilityDialog,
    openToggleMappingDialog,
    saveAvailabilityDialog,
    toggleDialogConfirmLabel,
    toggleDialogConfirmVariant,
    toggleDialogDescription,
    toggleDialogTitle,
  };
};
