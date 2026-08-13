import { computed, ref, watch, type Ref } from "vue";
import {
  CloudflaredAPI,
  type CloudflareManagedState,
  type CloudflareOptimizationScan,
  type CloudflareReconcilePlan,
} from "@/lib/api/tunnel";
import {
  extractErrorMessage,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { optimizationPreferredIpErrorLabel } from "./cloudflareOptimizationPresentation";
import type { CloudflareTranslate } from "./cloudflareTunnelTypes";

export const useCloudflareOptimization = ({
  loadManagedState,
  managedState,
  prepareOptimizationConflictResolution,
  previewReconcile,
  reconcilePlan,
  t,
}: {
  loadManagedState: (options?: { silent?: boolean }) => Promise<void>;
  managedState: Ref<CloudflareManagedState | null>;
  prepareOptimizationConflictResolution: () => Promise<void>;
  previewReconcile: () => Promise<void>;
  reconcilePlan: Ref<CloudflareReconcilePlan | null>;
  t: CloudflareTranslate;
}) => {
  const optimizationScan = ref<CloudflareOptimizationScan | null>(null);
  const selectedCandidateIp = ref("");
  const preferredCandidateIp = ref("");
  const optimizationOfficialRanges = ref(true);
  const optimizationBuiltinIds = ref<string[]>([]);
  const optimizationCustomHostnames = ref("");
  const isScanningOptimization = ref(false);
  const isApplyingOptimization = ref(false);
  const isFallingBackOptimization = ref(false);
  const isSavingOptimizationSources = ref(false);
  const updatingOptimizationDomainHostname = ref("");
  let optimizationSourcesLoaded = false;
  let scanPollTimer: number | undefined;

  const optimization = computed(() => managedState.value?.optimization ?? null);
  const optimizationApplied = computed(
    () => optimization.value?.enabled === true,
  );
  const optimizationScanReady = computed(
    () => optimization.value?.scanReady === true,
  );
  const optimizationReadinessErrorCode = computed(
    () => optimization.value?.scanReadinessErrorCode ?? null,
  );
  const optimizationActionRequiredDomains = computed(
    () =>
      optimization.value?.domains.filter((domain) => domain.actionRequired) ??
      [],
  );

  watch(
    managedState,
    (next) => {
      if (!next) return;
      if (!optimizationSourcesLoaded) {
        const sources = next.optimization.candidateSources;
        optimizationOfficialRanges.value = sources.officialRanges;
        optimizationBuiltinIds.value = sources.builtins
          .filter((source) => source.enabled)
          .map((source) => source.id);
        optimizationCustomHostnames.value = sources.customHostnames.join("\n");
        optimizationSourcesLoaded = true;
      }
      const latestScan = next.optimization.scans[0];
      if (latestScan && !optimizationScan.value) {
        optimizationScan.value = latestScan;
        preferredCandidateIp.value = latestScan.preferredIp || "";
      }
    },
    { immediate: true },
  );

  const setOptimizationDomainMode = async (
    hostname: string,
    mode: "optimize" | "external",
  ) => {
    if (updatingOptimizationDomainHostname.value) return;
    updatingOptimizationDomainHostname.value = hostname;
    try {
      const result = await CloudflaredAPI.setOptimizationDomainMode(
        hostname,
        mode,
      );
      await loadManagedState({ silent: true });
      if (mode === "external") {
        if (reconcilePlan.value) await previewReconcile();
        if (result.cleanupPending) {
          toast.warning(
            t(
              "admin.cloudflareTunnel.optimization.domainActions.externalCleanupPending",
            ),
          );
        } else {
          toast.success(
            t(
              "admin.cloudflareTunnel.optimization.domainActions.externalSaved",
            ),
          );
        }
      } else {
        await prepareOptimizationConflictResolution();
      }
    } catch (error) {
      toast.error(
        t("admin.cloudflareTunnel.optimization.domainActions.updateFailed"),
        {
          description: extractErrorMessage(
            error,
            t("admin.cloudflareTunnel.optimization.domainActions.updateFailed"),
          ),
        },
      );
    } finally {
      updatingOptimizationDomainHostname.value = "";
    }
  };

  const pollOptimizationScan = async (id: string) => {
    if (scanPollTimer) window.clearTimeout(scanPollTimer);
    try {
      const scan = await CloudflaredAPI.getOptimizationScan(id);
      optimizationScan.value = scan;
      isScanningOptimization.value = ["queued", "running"].includes(
        scan.status,
      );
      if (scan.status === "completed") {
        selectedCandidateIp.value = scan.recommendedIp || "";
        await loadManagedState({ silent: true });
        return;
      }
      if (["failed", "cancelled"].includes(scan.status)) return;
      scanPollTimer = window.setTimeout(
        () => void pollOptimizationScan(id),
        1500,
      );
    } catch (error) {
      isScanningOptimization.value = false;
      toast.error(t("admin.cloudflareTunnel.optimization.scanFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.optimization.scanFailed"),
        ),
      });
    }
  };

  const startOptimizationScan = async () => {
    if (!optimizationApplied.value) {
      toast.warning(
        t("admin.cloudflareTunnel.optimization.reconcileRequiredTitle"),
        {
          description: t(
            "admin.cloudflareTunnel.optimization.reconcileRequiredDescription",
          ),
        },
      );
      return;
    }
    if (!optimizationScanReady.value) {
      const resourceConflict =
        optimizationReadinessErrorCode.value === "cloudflare-resource-conflict";
      const validationPending =
        optimizationReadinessErrorCode.value ===
        "cloudflare-saas-validation-pending";
      toast.warning(
        t(
          resourceConflict
            ? "admin.cloudflareTunnel.optimization.resourceConflictTitle"
            : validationPending
              ? "admin.cloudflareTunnel.optimization.cloudflareSaasValidationPendingTitle"
              : "admin.cloudflareTunnel.optimization.notReadyTitle",
        ),
        {
          description: t(
            resourceConflict
              ? "admin.cloudflareTunnel.optimization.resourceConflictDescription"
              : validationPending
                ? "admin.cloudflareTunnel.optimization.cloudflareSaasValidationPendingDescription"
                : "admin.cloudflareTunnel.optimization.notReadyDescription",
          ),
        },
      );
      return;
    }
    isScanningOptimization.value = true;
    try {
      const preferredIp = preferredCandidateIp.value.trim();
      const scan = await CloudflaredAPI.startOptimizationScan(
        preferredIp ? { preferredIp } : {},
      );
      optimizationScan.value = scan;
      await pollOptimizationScan(scan.id);
    } catch (error) {
      isScanningOptimization.value = false;
      const message = extractErrorMessage(
        error,
        t("admin.cloudflareTunnel.optimization.scanFailed"),
      );
      toast.error(t("admin.cloudflareTunnel.optimization.scanFailed"), {
        description: optimizationPreferredIpErrorLabel(message, t),
      });
    }
  };

  const toggleOptimizationBuiltin = (id: string, enabled: boolean) => {
    const current = new Set(optimizationBuiltinIds.value);
    if (enabled) current.add(id);
    else current.delete(id);
    optimizationBuiltinIds.value = [...current];
  };

  const saveOptimizationSources = async () => {
    isSavingOptimizationSources.value = true;
    try {
      const sources = await CloudflaredAPI.saveOptimizationSourceSettings({
        officialRanges: optimizationOfficialRanges.value,
        builtinIds: optimizationBuiltinIds.value,
        customHostnames: optimizationCustomHostnames.value
          .split(/[\n,]+/u)
          .map((value) => value.trim())
          .filter(Boolean),
      });
      optimizationOfficialRanges.value = sources.officialRanges;
      optimizationBuiltinIds.value = sources.builtins
        .filter((source) => source.enabled)
        .map((source) => source.id);
      optimizationCustomHostnames.value = sources.customHostnames.join("\n");
      if (managedState.value) {
        managedState.value.optimization.candidateSources = sources;
      }
      toast.success(t("admin.cloudflareTunnel.optimization.sources.saved"));
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.optimization.sources.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.optimization.sources.saveFailed"),
        ),
      });
    } finally {
      isSavingOptimizationSources.value = false;
    }
  };

  const cancelOptimizationScan = async () => {
    const scan = optimizationScan.value;
    if (!scan) return;
    try {
      await CloudflaredAPI.cancelOptimizationScan(scan.id);
      if (scanPollTimer) window.clearTimeout(scanPollTimer);
      isScanningOptimization.value = false;
      optimizationScan.value = {
        ...scan,
        status: "cancelled",
        phase: "cancelled",
      };
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.optimization.cancelFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.optimization.cancelFailed"),
        ),
      });
    }
  };

  const applyOptimization = async () => {
    const scan = optimizationScan.value;
    if (!scan || scan.status !== "completed" || !optimizationApplied.value) {
      if (!optimizationApplied.value) {
        toast.warning(
          t("admin.cloudflareTunnel.optimization.reconcileRequiredTitle"),
          {
            description: t(
              "admin.cloudflareTunnel.optimization.reconcileRequiredDescription",
            ),
          },
        );
      }
      return;
    }
    isApplyingOptimization.value = true;
    try {
      await CloudflaredAPI.applyOptimization({
        scanId: scan.id,
        candidateIp: selectedCandidateIp.value || undefined,
      });
      await loadManagedState({ silent: true });
      const conflictCount = optimizationActionRequiredDomains.value.length;
      if (conflictCount > 0) {
        await prepareOptimizationConflictResolution();
        toast.warning(
          t("admin.cloudflareTunnel.optimization.appliedWithConflictsTitle"),
          {
            description: t(
              "admin.cloudflareTunnel.optimization.appliedWithConflictsDescription",
              { count: conflictCount },
            ),
          },
        );
      } else {
        toast.success(t("admin.cloudflareTunnel.optimization.applied"));
      }
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.optimization.applyFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.optimization.applyFailed"),
        ),
      });
    } finally {
      isApplyingOptimization.value = false;
    }
  };

  const fallbackOptimization = async () => {
    isFallingBackOptimization.value = true;
    try {
      await CloudflaredAPI.fallbackOptimization();
      await loadManagedState({ silent: true });
      toast.success(t("admin.cloudflareTunnel.optimization.fallbackApplied"));
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.optimization.fallbackFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.optimization.fallbackFailed"),
        ),
      });
    } finally {
      isFallingBackOptimization.value = false;
    }
  };

  const stop = () => {
    if (scanPollTimer) window.clearTimeout(scanPollTimer);
  };

  return {
    applyOptimization,
    cancelOptimizationScan,
    fallbackOptimization,
    isApplyingOptimization,
    isFallingBackOptimization,
    isSavingOptimizationSources,
    isScanningOptimization,
    optimization,
    optimizationActionRequiredDomains,
    optimizationApplied,
    optimizationBuiltinIds,
    optimizationCustomHostnames,
    optimizationOfficialRanges,
    optimizationReadinessErrorCode,
    optimizationScan,
    optimizationScanReady,
    preferredCandidateIp,
    saveOptimizationSources,
    selectedCandidateIp,
    setOptimizationDomainMode,
    startOptimizationScan,
    stop,
    toggleOptimizationBuiltin,
    updatingOptimizationDomainHostname,
  };
};
