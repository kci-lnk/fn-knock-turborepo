import { computed, ref, shallowRef, type ComputedRef, type Ref } from "vue";
import { toast } from "@admin-shared/utils/toast";
import type { HostMapping, HostMappingAvailability } from "@/types";
import { getAvailabilityWindowValidationError } from "./model";

type BatchMutation = "enable" | "disable" | "delete";
type AsyncActionRun = <T>(action: () => Promise<T>) => Promise<T | undefined>;
type Translate = (key: string, params?: Record<string, number>) => string;

const DEFAULT_START_TIME = "09:00";
const DEFAULT_END_TIME = "18:00";

export const useSubdomainBatchActions = ({
  allMappings,
  isAuthServiceTarget,
  isSavingMappings,
  runSaveMappings,
  saveHostMappings,
  translate,
}: {
  allMappings: ComputedRef<HostMapping[]>;
  isAuthServiceTarget: (target: string) => boolean;
  isSavingMappings: Ref<boolean>;
  runSaveMappings: AsyncActionRun;
  saveHostMappings: (mappings: HostMapping[]) => Promise<unknown>;
  translate: Translate;
}) => {
  const batchMutation = ref<BatchMutation | null>(null);
  const batchHosts = ref<string[]>([]);
  const batchAvailabilityOpen = ref(false);
  const availabilityFormEnabled = ref(true);
  const availabilityFormStartTime = ref(DEFAULT_START_TIME);
  const availabilityFormEndTime = ref(DEFAULT_END_TIME);
  const onComplete = shallowRef<(() => void) | null>(null);

  const selectedMappings = computed(() => {
    const hosts = new Set(batchHosts.value);
    return allMappings.value.filter(
      (mapping) =>
        hosts.has(mapping.host) && !isAuthServiceTarget(mapping.target),
    );
  });
  const selectedCount = computed(() => selectedMappings.value.length);
  const isBatchMutationOpen = computed(
    () => batchMutation.value !== null && selectedCount.value > 0,
  );
  const batchMutationTitle = computed(() => {
    switch (batchMutation.value) {
      case "enable":
        return translate("admin.subdomainProxy.batchEnableTitle", {
          count: selectedCount.value,
        });
      case "disable":
        return translate("admin.subdomainProxy.batchDisableTitle", {
          count: selectedCount.value,
        });
      default:
        return translate("admin.subdomainProxy.batchDeleteTitle", {
          count: selectedCount.value,
        });
    }
  });
  const batchMutationDescription = computed(() => {
    const key =
      batchMutation.value === "enable"
        ? "admin.subdomainProxy.batchEnableDescription"
        : batchMutation.value === "disable"
          ? "admin.subdomainProxy.batchDisableDescription"
          : "admin.subdomainProxy.batchDeleteDescription";
    return translate(key, { count: selectedCount.value });
  });
  const batchMutationConfirmLabel = computed(() => {
    const key =
      batchMutation.value === "enable"
        ? "admin.subdomainProxy.confirmBatchEnable"
        : batchMutation.value === "disable"
          ? "admin.subdomainProxy.confirmBatchDisable"
          : "admin.subdomainProxy.confirmBatchDelete";
    return translate(key, { count: selectedCount.value });
  });
  const batchMutationConfirmVariant = computed<"default" | "destructive">(
    () => (batchMutation.value === "enable" ? "default" : "destructive"),
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

  const setTargets = (hosts: string[], complete: () => void) => {
    batchHosts.value = [...new Set(hosts)];
    onComplete.value = complete;
  };
  const closeBatchMutation = () => {
    batchMutation.value = null;
    batchHosts.value = [];
    onComplete.value = null;
  };
  const closeBatchAvailability = () => {
    batchAvailabilityOpen.value = false;
    batchHosts.value = [];
    onComplete.value = null;
    availabilityFormEnabled.value = true;
    availabilityFormStartTime.value = DEFAULT_START_TIME;
    availabilityFormEndTime.value = DEFAULT_END_TIME;
  };
  const openBatchMutation = (
    hosts: string[],
    mutation: BatchMutation,
    complete: () => void,
  ) => {
    if (isSavingMappings.value || hosts.length === 0) return;
    setTargets(hosts, complete);
    if (selectedCount.value === 0) {
      closeBatchMutation();
      return;
    }
    batchMutation.value = mutation;
  };
  const openBatchAvailability = (hosts: string[], complete: () => void) => {
    if (isSavingMappings.value || hosts.length === 0) return;
    setTargets(hosts, complete);
    if (selectedCount.value === 0) {
      closeBatchAvailability();
      return;
    }
    availabilityFormEnabled.value = true;
    availabilityFormStartTime.value = DEFAULT_START_TIME;
    availabilityFormEndTime.value = DEFAULT_END_TIME;
    batchAvailabilityOpen.value = true;
  };
  const finishSuccessfulAction = () => {
    onComplete.value?.();
    closeBatchMutation();
    closeBatchAvailability();
  };
  const confirmBatchMutation = async () => {
    const mutation = batchMutation.value;
    const hosts = new Set(selectedMappings.value.map((mapping) => mapping.host));
    if (!mutation || hosts.size === 0 || isSavingMappings.value) return;

    const saved = await runSaveMappings(async () => {
      const nextMappings =
        mutation === "delete"
          ? allMappings.value.filter((mapping) => !hosts.has(mapping.host))
          : allMappings.value.map((mapping) =>
              hosts.has(mapping.host)
                ? { ...mapping, disabled: mutation === "disable" }
                : mapping,
            );
      await saveHostMappings(nextMappings);
      toast.success(
        translate(
          mutation === "delete"
            ? "admin.subdomainProxy.batchMappingsDeleted"
            : mutation === "disable"
              ? "admin.subdomainProxy.batchMappingsDisabled"
              : "admin.subdomainProxy.batchMappingsEnabled",
          { count: hosts.size },
        ),
      );
      return true;
    });
    if (saved) finishSuccessfulAction();
  };
  const saveBatchAvailability = async () => {
    const hosts = new Set(selectedMappings.value.map((mapping) => mapping.host));
    if (
      hosts.size === 0 ||
      isSavingMappings.value ||
      availabilityValidationMessage.value
    ) {
      return;
    }
    const availability: HostMappingAvailability | null =
      availabilityFormEnabled.value
        ? {
            enabled: true,
            start_time: availabilityFormStartTime.value.trim(),
            end_time: availabilityFormEndTime.value.trim(),
          }
        : null;
    const saved = await runSaveMappings(async () => {
      await saveHostMappings(
        allMappings.value.map((mapping) =>
          hosts.has(mapping.host) ? { ...mapping, availability } : mapping,
        ),
      );
      toast.success(
        translate(
          availability
            ? "admin.subdomainProxy.batchAvailabilitySaved"
            : "admin.subdomainProxy.batchAvailabilityCleared",
          { count: hosts.size },
        ),
      );
      return true;
    });
    if (saved) finishSuccessfulAction();
  };

  return {
    availabilityFormEnabled,
    availabilityFormEndTime,
    availabilityFormStartTime,
    availabilityValidationMessage,
    batchAvailabilityOpen,
    batchMutationConfirmLabel,
    batchMutationConfirmVariant,
    batchMutationDescription,
    batchMutationTitle,
    closeBatchAvailability,
    closeBatchMutation,
    confirmBatchMutation,
    isBatchMutationOpen,
    openBatchAvailability,
    openBatchMutation,
    saveBatchAvailability,
    selectedCount,
  };
};
