import { computed, ref, type Ref } from "vue";
import {
  CloudflaredAPI,
  type CloudflaredConfig,
  type CloudflareManagedState,
  type CloudflareReconcileJob,
  type CloudflareReconcilePlan,
} from "@/lib/api/tunnel";
import { createVisibilityPoller } from "@/composables/useVisibilityPolling";
import {
  extractErrorMessage,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import type { CloudflareTranslate } from "./cloudflareTunnelTypes";

export const useCloudflareManagedTunnel = ({
  reloadConfig,
  t,
  tunnelTokenConfigured,
}: {
  reloadConfig: () => Promise<void>;
  t: CloudflareTranslate;
  tunnelTokenConfigured: Ref<boolean>;
}) => {
  const showApiToken = ref(false);
  const apiToken = ref("");
  const apiTokenConfigured = ref(false);
  const managedState = ref<CloudflareManagedState | null>(null);
  const tunnelMode = ref<"dedicated" | "existing">("dedicated");
  const selectedTunnelId = ref("");
  const optimizationEnabled = ref(false);
  const reconcilePlan = ref<CloudflareReconcilePlan | null>(null);
  const reconcileJob = ref<CloudflareReconcileJob | null>(null);
  const takeoverResourceIds = ref<string[]>([]);
  const reconcileAttentionToken = ref(0);
  const isConnectingCloudflare = ref(false);
  const isLoadingManagedState = ref(false);
  const isPreviewingReconcile = ref(false);
  const isApplyingReconcile = ref(false);
  const deleteDedicatedTunnel = ref(false);
  let reconcilePollTimer: number | undefined;
  let reconcilePollSequence = 0;

  const applyConfig = (config: CloudflaredConfig) => {
    apiTokenConfigured.value = config.apiTokenConfigured;
    optimizationEnabled.value = config.optimizationEnabled;
    if (config.tunnel?.ownership === "adopted") {
      tunnelMode.value = "existing";
      selectedTunnelId.value = config.tunnel.id;
    }
  };

  const reconcileHasUnconfirmedConflicts = computed(() => {
    const plan = reconcilePlan.value;
    if (!plan) return false;
    return plan.conflicts.some(
      (conflict) =>
        !conflict.takeoverAllowed ||
        !takeoverResourceIds.value.includes(conflict.id),
    );
  });

  const loadManagedState = async (options?: { silent?: boolean }) => {
    if (isLoadingManagedState.value) return;
    isLoadingManagedState.value = true;
    try {
      const next = await CloudflaredAPI.getCloudflareState();
      managedState.value = next;
      apiTokenConfigured.value = next.apiTokenConfigured;
      tunnelTokenConfigured.value = next.tunnelTokenConfigured;
      optimizationEnabled.value = next.optimization.enabled;
      const managedTunnel = next.managed.tunnel;
      if (managedTunnel?.ownership === "adopted") {
        tunnelMode.value = "existing";
        selectedTunnelId.value = managedTunnel.id;
      } else if (managedTunnel?.ownership === "dedicated") {
        tunnelMode.value = "dedicated";
      }
    } catch (error) {
      if (!options?.silent) {
        toast.error(t("admin.cloudflareTunnel.managed.loadFailed"), {
          description: extractErrorMessage(
            error,
            t("admin.cloudflareTunnel.managed.loadFailed"),
          ),
        });
      }
    } finally {
      isLoadingManagedState.value = false;
    }
  };

  const connectCloudflare = async () => {
    const value = apiToken.value.trim();
    if (!value) return;
    isConnectingCloudflare.value = true;
    try {
      managedState.value = await CloudflaredAPI.saveCloudflareCredential(value);
      apiToken.value = "";
      apiTokenConfigured.value = true;
      toast.success(t("admin.cloudflareTunnel.managed.connected"));
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.managed.connectFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.managed.connectFailed"),
        ),
      });
    } finally {
      isConnectingCloudflare.value = false;
    }
  };

  const disconnectCloudflare = async () => {
    isConnectingCloudflare.value = true;
    try {
      await CloudflaredAPI.deleteCloudflareCredential();
      apiTokenConfigured.value = false;
      apiToken.value = "";
      if (managedState.value) managedState.value.apiTokenConfigured = false;
      toast.success(t("admin.cloudflareTunnel.managed.disconnected"));
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.managed.disconnectFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.managed.disconnectFailed"),
        ),
      });
    } finally {
      isConnectingCloudflare.value = false;
    }
  };

  const previewReconcile = async () => {
    isPreviewingReconcile.value = true;
    reconcilePlan.value = null;
    takeoverResourceIds.value = [];
    try {
      reconcilePlan.value = await CloudflaredAPI.previewReconcile({
        tunnelMode: tunnelMode.value,
        tunnelId:
          tunnelMode.value === "existing"
            ? selectedTunnelId.value || undefined
            : undefined,
        optimizationEnabled: optimizationEnabled.value,
      });
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.managed.previewFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.managed.previewFailed"),
        ),
      });
    } finally {
      isPreviewingReconcile.value = false;
    }
  };

  const prepareOptimizationConflictResolution = async () => {
    optimizationEnabled.value = true;
    reconcileAttentionToken.value += 1;
    await previewReconcile();
  };

  const previewCleanup = async () => {
    isPreviewingReconcile.value = true;
    reconcilePlan.value = null;
    takeoverResourceIds.value = [];
    try {
      reconcilePlan.value = await CloudflaredAPI.previewReconcile({
        action: "cleanup",
        tunnelMode: tunnelMode.value,
        tunnelId: selectedTunnelId.value || undefined,
        optimizationEnabled: optimizationEnabled.value,
        deleteDedicatedTunnel: deleteDedicatedTunnel.value,
      });
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.managed.previewFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.managed.previewFailed"),
        ),
      });
    } finally {
      isPreviewingReconcile.value = false;
    }
  };

  const completeReconcileJob = async (job: CloudflareReconcileJob) => {
    reconcileJob.value = job;
    isApplyingReconcile.value = false;
    reconcilePlan.value = null;
    takeoverResourceIds.value = [];
    await Promise.all([loadManagedState({ silent: true }), reloadConfig()]);
    if (job.status === "succeeded") {
      toast.success(t("admin.cloudflareTunnel.managed.applied"));
      return;
    }
    toast.error(t("admin.cloudflareTunnel.managed.applyFailed"), {
      description: job.error || t("admin.cloudflareTunnel.managed.applyFailed"),
    });
  };

  const pollReconcileJob = async (
    id: string,
    failures: number,
    sequence: number,
  ) => {
    if (sequence !== reconcilePollSequence) return;
    if (reconcilePollTimer) window.clearTimeout(reconcilePollTimer);
    try {
      const job = await CloudflaredAPI.getReconcileJob(id);
      if (sequence !== reconcilePollSequence) return;
      reconcileJob.value = job;
      if (["queued", "running"].includes(job.status)) {
        isApplyingReconcile.value = true;
        reconcilePollTimer = window.setTimeout(
          () => void pollReconcileJob(id, 0, sequence),
          1500,
        );
        return;
      }
      await completeReconcileJob(job);
    } catch (error) {
      if (sequence !== reconcilePollSequence) return;
      const status = (error as { response?: { status?: number } }).response
        ?.status;
      if (status !== 404 && failures < 30) {
        isApplyingReconcile.value = true;
        reconcilePollTimer = window.setTimeout(
          () => void pollReconcileJob(id, failures + 1, sequence),
          2000,
        );
        return;
      }
      isApplyingReconcile.value = false;
      if (status === 404) {
        reconcileJob.value = null;
        reconcilePlan.value = null;
        takeoverResourceIds.value = [];
        await Promise.all([loadManagedState({ silent: true }), reloadConfig()]);
      }
      toast.error(t("admin.cloudflareTunnel.managed.applyFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.managed.applyFailed"),
        ),
      });
    }
  };

  const followReconcileJob = async (job: CloudflareReconcileJob) => {
    reconcileJob.value = job;
    isApplyingReconcile.value = ["queued", "running"].includes(job.status);
    const sequence = ++reconcilePollSequence;
    await pollReconcileJob(job.id, 0, sequence);
  };

  const recoverActiveReconcileJob = async () => {
    try {
      await followReconcileJob(await CloudflaredAPI.getActiveReconcileJob());
    } catch (error) {
      const status = (error as { response?: { status?: number } }).response
        ?.status;
      if (status !== 404) {
        console.warn("recover active Cloudflare reconcile job failed:", error);
      }
    }
  };

  const applyReconcile = async () => {
    if (!reconcilePlan.value || reconcileHasUnconfirmedConflicts.value) return;
    isApplyingReconcile.value = true;
    const planId = reconcilePlan.value.planId;
    try {
      const job = await CloudflaredAPI.applyReconcile({
        planId,
        takeoverResourceIds: takeoverResourceIds.value,
      });
      await followReconcileJob(job);
    } catch (error) {
      const existing = await CloudflaredAPI.getReconcileJobByPlan(planId).catch(
        () => null,
      );
      if (existing) {
        await followReconcileJob(existing);
        return;
      }
      isApplyingReconcile.value = false;
      toast.error(t("admin.cloudflareTunnel.managed.applyFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.managed.applyFailed"),
        ),
      });
    }
  };

  const managedStatePoller = createVisibilityPoller({
    intervalMs: 60_000,
    immediate: false,
    enabled: () => apiTokenConfigured.value,
    task: () => loadManagedState({ silent: true }),
  });
  const stop = () => {
    reconcilePollSequence += 1;
    if (reconcilePollTimer) window.clearTimeout(reconcilePollTimer);
    managedStatePoller.stop();
  };

  return {
    apiToken,
    apiTokenConfigured,
    applyConfig,
    applyReconcile,
    connectCloudflare,
    deleteDedicatedTunnel,
    disconnectCloudflare,
    isApplyingReconcile,
    isConnectingCloudflare,
    isLoadingManagedState,
    isPreviewingReconcile,
    loadManagedState,
    managedState,
    optimizationEnabled,
    prepareOptimizationConflictResolution,
    previewCleanup,
    previewReconcile,
    reconcileAttentionToken,
    reconcileHasUnconfirmedConflicts,
    reconcileJob,
    reconcilePlan,
    recoverActiveReconcileJob,
    selectedTunnelId,
    showApiToken,
    startPolling: managedStatePoller.start,
    stop,
    takeoverResourceIds,
    tunnelMode,
  };
};
