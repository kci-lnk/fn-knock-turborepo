import { computed, ref, watch } from "vue";
import { TerminalAPI } from "@/lib/api/terminal";
import {
  useTerminalResource,
  type TerminalResourceContext,
} from "./useTerminalResource";

/** Disk enumeration is demand-driven and independent of overview sampling. */
export const useTerminalDisks = (context: TerminalResourceContext) => {
  const diskDetailsOpen = ref(false);
  watch(
    [context.sessionId, () => context.attachment.value?.id, context.connected],
    () => {
      diskDetailsOpen.value = false;
    },
    { flush: "sync" },
  );
  const resource = useTerminalResource(
    {
      ...context,
      connected: computed(
        () => context.connected.value && diskDetailsOpen.value,
      ),
    },
    TerminalAPI.getAttachmentDisks,
  );
  return {
    diskDetailsOpen,
    disks: resource.metrics,
    disksLoading: resource.metricsLoading,
    disksFailed: resource.metricsFailed,
    disksStale: resource.metricsStale,
  };
};
