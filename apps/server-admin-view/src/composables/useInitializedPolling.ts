import { onActivated, onBeforeUnmount, onDeactivated, onMounted } from "vue";

// KeepAlive can activate before asynchronous initialization finishes. Start only
// after initialization, and never restart a hidden or disposed polling resource.
export const useInitializedPolling = (options: {
  poller: { start: () => void; stop: () => void };
  initialize: () => Promise<void>;
  dispose: () => void;
}) => {
  let active = true;
  let initialized = false;
  let disposed = false;
  onMounted(async () => {
    await options.initialize();
    initialized = true;
    if (active && !disposed) options.poller.start();
  });
  onActivated(() => {
    active = true;
    if (initialized && !disposed) options.poller.start();
  });
  onDeactivated(() => {
    active = false;
    options.poller.stop();
  });
  onBeforeUnmount(() => {
    disposed = true;
    options.poller.stop();
    options.dispose();
  });
};
