import { computed, reactive } from "vue";
import {
  capabilityStatusKeys,
  cloudflareResourceConflictErrorCode,
  cloudflareSaasValidationPendingErrorCode,
  formatOptimizationDate,
  optimizationSwitchReasonLabel,
  requiresCloudflareSaasSetup,
} from "./cloudflareOptimizationPresentation";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

export const useCloudflareOptimizationCardPresentation = (
  controller: CloudflareTunnelController,
) => {
  const capabilityRequiresCloudflareSaas = computed(() => {
    const probe = controller.optimization.value?.capabilityProbe;
    return requiresCloudflareSaasSetup(probe?.reasonCode, probe?.message);
  });
  const capabilityValidationPending = computed(
    () =>
      controller.optimizationReadinessErrorCode.value ===
      cloudflareSaasValidationPendingErrorCode,
  );
  const optimizationResourceConflict = computed(
    () =>
      controller.optimizationReadinessErrorCode.value ===
      cloudflareResourceConflictErrorCode,
  );
  const optimizedDomainCount = computed(
    () =>
      controller.optimization.value?.domains.filter((item) => item.optimized)
        .length || 0,
  );
  const optimizationManagedDomainCount = computed(
    () =>
      controller.optimization.value?.domains.filter(
        (item) => item.managementMode !== "external",
      ).length || 0,
  );
  const capabilityProbeMessage = computed(() => {
    const probe = controller.optimization.value?.capabilityProbe;
    if (!probe) return "";
    if (capabilityRequiresCloudflareSaas.value) {
      return controller.t(
        "admin.cloudflareTunnel.optimization.cloudflareSaasRequiredDescription",
      );
    }
    if (probe.status === "pending") {
      return controller.t(
        "admin.cloudflareTunnel.optimization.cloudflareSaasValidationPendingDescription",
      );
    }
    const key = capabilityStatusKeys[probe.status];
    return key
      ? controller.t(`admin.cloudflareTunnel.optimization.capability.${key}`)
      : probe.message || probe.status;
  });

  const formatDate = (value?: string | null) =>
    formatOptimizationDate(value, controller.locale.value);
  const switchReasonLabel = (reason: string) =>
    optimizationSwitchReasonLabel(reason, controller.t);

  return reactive({
    capabilityProbeMessage,
    capabilityRequiresCloudflareSaas,
    capabilityValidationPending,
    formatDate,
    optimizationManagedDomainCount,
    optimizationResourceConflict,
    optimizedDomainCount,
    switchReasonLabel,
  });
};

export type CloudflareOptimizationCardPresentation = ReturnType<
  typeof useCloudflareOptimizationCardPresentation
>;
