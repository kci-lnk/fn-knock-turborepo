import { computed, ref, shallowRef, type ComputedRef, type Ref } from "vue";
import { toast } from "@admin-shared/utils/toast";
import type { HostMapping, HostMappingAvailability } from "@/types";
import { getAvailabilityWindowValidationError } from "./model";
import { batchMappingSignature } from "./subdomain-batch-edit-model";

type BatchMutation =
  "enable" | "disable" | "delete" | "require-auth" | "public-access";
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
  saveHostMappings: (
    mappings: HostMapping[],
    beforeSave: () => void,
  ) => Promise<unknown>;
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
        hosts.has(mapping.host) &&
        mapping.service_role !== "auth" &&
        !isAuthServiceTarget(mapping.target),
    );
  });
  const selectedCount = computed(() => selectedMappings.value.length);
  const isBatchMutationOpen = computed(
    () => batchMutation.value !== null && selectedCount.value > 0,
  );
  const mutationMessages = {
    enable: {
      title: "admin.subdomainProxy.batchEnableTitle",
      description: "admin.subdomainProxy.batchEnableDescription",
      confirm: "admin.subdomainProxy.confirmBatchEnable",
      success: "admin.subdomainProxy.batchMappingsEnabled",
    },
    disable: {
      title: "admin.subdomainProxy.batchDisableTitle",
      description: "admin.subdomainProxy.batchDisableDescription",
      confirm: "admin.subdomainProxy.confirmBatchDisable",
      success: "admin.subdomainProxy.batchMappingsDisabled",
    },
    delete: {
      title: "admin.subdomainProxy.batchDeleteTitle",
      description: "admin.subdomainProxy.batchDeleteDescription",
      confirm: "admin.subdomainProxy.confirmBatchDelete",
      success: "admin.subdomainProxy.batchMappingsDeleted",
    },
    "require-auth": {
      title: "admin.subdomainProxy.batchRequireAuthTitle",
      description: "admin.subdomainProxy.batchRequireAuthDescription",
      confirm: "admin.subdomainProxy.confirmBatchRequireAuth",
      success: "admin.subdomainProxy.batchMappingsAuthRequired",
    },
    "public-access": {
      title: "admin.subdomainProxy.batchPublicAccessTitle",
      description: "admin.subdomainProxy.batchPublicAccessDescription",
      confirm: "admin.subdomainProxy.confirmBatchPublicAccess",
      success: "admin.subdomainProxy.batchMappingsPublic",
    },
  } satisfies Record<
    BatchMutation,
    Record<"title" | "description" | "confirm" | "success", string>
  >;
  const mutationMessage = (field: "title" | "description" | "confirm") =>
    batchMutation.value
      ? translate(mutationMessages[batchMutation.value][field], {
          count: selectedCount.value,
        })
      : "";
  const batchMutationTitle = computed(() => mutationMessage("title"));
  const batchMutationDescription = computed(() =>
    mutationMessage("description"),
  );
  const batchMutationConfirmLabel = computed(() => mutationMessage("confirm"));
  const batchMutationConfirmVariant = computed<"default" | "destructive">(() =>
    batchMutation.value === "disable" ||
    batchMutation.value === "delete" ||
    batchMutation.value === "public-access"
      ? "destructive"
      : "default",
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
    if (isSavingMappings.value) return;
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
  const saveBatchMappings = (mappings: HostMapping[]) => {
    const baseline = batchMappingSignature(allMappings.value);
    return saveHostMappings(mappings, () => {
      if (batchMappingSignature(allMappings.value) !== baseline) {
        throw new Error(
          translate("admin.subdomainProxy.batchConfigurationChanged"),
        );
      }
    });
  };
  const confirmBatchMutation = async () => {
    const mutation = batchMutation.value;
    const hosts = new Set(
      selectedMappings.value.map((mapping) => mapping.host),
    );
    if (!mutation || hosts.size === 0 || isSavingMappings.value) return;

    const saved = await runSaveMappings(async () => {
      const nextMappings =
        mutation === "delete"
          ? allMappings.value.filter((mapping) => !hosts.has(mapping.host))
          : allMappings.value.map((mapping) => {
              if (!hosts.has(mapping.host)) return mapping;
              if (mutation === "require-auth" || mutation === "public-access") {
                return { ...mapping, use_auth: mutation === "require-auth" };
              }
              return { ...mapping, disabled: mutation === "disable" };
            });
      await saveBatchMappings(nextMappings);
      toast.success(
        translate(mutationMessages[mutation].success, { count: hosts.size }),
      );
      return true;
    });
    if (saved) finishSuccessfulAction();
  };
  const saveBatchAvailability = async () => {
    const hosts = new Set(
      selectedMappings.value.map((mapping) => mapping.host),
    );
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
      await saveBatchMappings(
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
    batchAvailabilityFormEnabled: availabilityFormEnabled,
    batchAvailabilityFormEndTime: availabilityFormEndTime,
    batchAvailabilityFormStartTime: availabilityFormStartTime,
    batchAvailabilityValidationMessage: availabilityValidationMessage,
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
    batchSelectedCount: selectedCount,
  };
};
