import { onMounted, onUnmounted, watch, type WatchSource } from "vue";

type SubdomainProxyLifecycleOptions = {
  clearProtocolHeadersWarningCloseTimer: () => void;
  filteredMappings: WatchSource<unknown>;
  isConfigLoaded: () => boolean;
  loadAccessEntryPort: () => unknown;
  loadConfig: () => Promise<unknown>;
  loadGlobalVisibilityStatus: () => unknown;
  startAvailabilityClock: () => void;
  startTrafficRealtimePolling: () => void;
  stopAvailabilityClock: () => void;
  stopDiscoverScan: () => void;
  stopTrafficRealtimePolling: () => void;
  syncDraggableVisibleMappings: () => void;
};

export const useSubdomainProxyLifecycle = (
  options: SubdomainProxyLifecycleOptions,
) => {
  watch(options.filteredMappings, options.syncDraggableVisibleMappings, {
    immediate: true,
  });

  let disposed = false;
  onMounted(async () => {
    options.startAvailabilityClock();
    if (!options.isConfigLoaded()) await options.loadConfig();
    if (disposed) return;
    void options.loadGlobalVisibilityStatus();
    void options.loadAccessEntryPort();
    options.startTrafficRealtimePolling();
  });

  onUnmounted(() => {
    disposed = true;
    options.stopAvailabilityClock();
    options.clearProtocolHeadersWarningCloseTimer();
    options.stopTrafficRealtimePolling();
    options.stopDiscoverScan();
  });
};
