import { onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import type { CloudflaredConfig } from "@/lib/api/tunnel";
import { useCloudflareManagedTunnel } from "./useCloudflareManagedTunnel";
import { useCloudflareOptimization } from "./useCloudflareOptimization";
import { useCloudflaredRuntime } from "./useCloudflaredRuntime";

export const useCloudflareTunnelController = () => {
  const { locale, t } = useI18n();
  let isDisposed = false;
  let applyManagedConfig: (config: CloudflaredConfig) => void = () => undefined;

  const {
    loadAccessEntryPort,
    loadConfig,
    loadEnvironmentConfig,
    loadStatus,
    startPolling: startRuntimePolling,
    stopPolling: stopRuntimePolling,
    ...runtime
  } = useCloudflaredRuntime({
    t,
    onConfigLoaded: (config) => applyManagedConfig(config),
  });

  const {
    applyConfig,
    loadManagedState,
    recoverActiveReconcileJob,
    startPolling: startManagedStatePolling,
    stop: stopManagedTunnel,
    ...managedTunnel
  } = useCloudflareManagedTunnel({
    t,
    tunnelTokenConfigured: runtime.tunnelTokenConfigured,
    reloadConfig: loadConfig,
  });
  applyManagedConfig = applyConfig;

  const {
    stop: stopOptimization,
    ...optimization
  } = useCloudflareOptimization({
    t,
    loadManagedState,
    managedState: managedTunnel.managedState,
    prepareOptimizationConflictResolution:
      managedTunnel.prepareOptimizationConflictResolution,
    previewReconcile: managedTunnel.previewReconcile,
    reconcilePlan: managedTunnel.reconcilePlan,
  });

  onMounted(async () => {
    await Promise.all([
      recoverActiveReconcileJob(),
      loadStatus(),
      loadConfig(),
      loadAccessEntryPort(),
      loadEnvironmentConfig(),
      loadManagedState({ silent: true }),
    ]);
    if (isDisposed) return;

    startRuntimePolling();
    startManagedStatePolling();
  });

  onUnmounted(() => {
    isDisposed = true;
    stopRuntimePolling();
    stopManagedTunnel();
    stopOptimization();
  });

  return {
    ...runtime,
    ...managedTunnel,
    ...optimization,
    locale,
    t,
  };
};

export type CloudflareTunnelController = ReturnType<
  typeof useCloudflareTunnelController
>;
