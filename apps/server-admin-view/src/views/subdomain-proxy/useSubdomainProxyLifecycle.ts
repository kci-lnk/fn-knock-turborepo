import { onMounted, onUnmounted, watch, type WatchSource } from "vue";

type SubdomainProxyLifecycleOptions = {
  clearMappingDialogKeyboardScrollTimer: () => void;
  clearProtocolHeadersWarningCloseTimer: () => void;
  filteredMappings: WatchSource<unknown>;
  handleMappingDialogViewportResize: () => void;
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
    window.visualViewport?.addEventListener(
      "resize",
      options.handleMappingDialogViewportResize,
    );
    window.visualViewport?.addEventListener(
      "scroll",
      options.handleMappingDialogViewportResize,
    );
    if (!options.isConfigLoaded()) await options.loadConfig();
    if (disposed) return;
    void options.loadGlobalVisibilityStatus();
    void options.loadAccessEntryPort();
    options.startTrafficRealtimePolling();
  });

  onUnmounted(() => {
    disposed = true;
    options.stopAvailabilityClock();
    window.visualViewport?.removeEventListener(
      "resize",
      options.handleMappingDialogViewportResize,
    );
    window.visualViewport?.removeEventListener(
      "scroll",
      options.handleMappingDialogViewportResize,
    );
    options.clearMappingDialogKeyboardScrollTimer();
    options.clearProtocolHeadersWarningCloseTimer();
    options.stopTrafficRealtimePolling();
    options.stopDiscoverScan();
  });
};
