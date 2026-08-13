import { ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  WOLAPI,
  type WOLDiscoveredDevice,
  type WOLDiscoveryPollEvent,
  type WOLDiscoveryProgress,
  type WOLDiscoveryResult,
} from "@/lib/api/wol";
import { reduceWolDiscoveryEvent } from "./wol-management-model";
import type { WolTranslate } from "./wol-management-types";

export const useWolDiscovery = ({
  reload,
  t,
}: {
  reload: () => Promise<void>;
  t: WolTranslate;
}) => {
  const discoveryOpen = ref(false);
  const discoveryResult = ref<WOLDiscoveryResult | null>(null);
  const discoveryProgress = ref<WOLDiscoveryProgress | null>(null);
  const discovering = ref(false);
  const addingDiscovered = ref(false);
  let discoveryAbortController: AbortController | null = null;

  const applyDiscoveryEvent = (event: WOLDiscoveryPollEvent) => {
    const next = reduceWolDiscoveryEvent(
      { progress: discoveryProgress.value, result: discoveryResult.value },
      event,
    );
    discoveryProgress.value = next.progress;
    discoveryResult.value = next.result;
  };

  const discoverDevices = async (targetCidrs: string[] = []) => {
    discoveryAbortController?.abort();
    const abortController = new AbortController();
    discoveryAbortController = abortController;
    discovering.value = true;
    discoveryProgress.value = null;
    discoveryResult.value = null;
    try {
      discoveryResult.value = await WOLAPI.discoverLocalDevices(targetCidrs, {
        signal: abortController.signal,
        onEvent: applyDiscoveryEvent,
      });
    } catch (error) {
      if ((error as Error)?.name === "AbortError") return;
      toast.error(t("admin.wol.discovery.failed"), {
        description: extractErrorMessage(
          error,
          t("admin.wol.discovery.failed"),
        ),
      });
    } finally {
      if (discoveryAbortController === abortController) {
        discoveryAbortController = null;
        discovering.value = false;
      }
    }
  };

  const openDiscovery = async () => {
    discoveryOpen.value = true;
    await discoverDevices();
  };

  const setDiscoveryOpen = (open: boolean) => {
    discoveryOpen.value = open;
    if (!open) {
      discoveryAbortController?.abort();
      discoveryAbortController = null;
      discovering.value = false;
    }
  };

  const addDiscoveredDevices = async (
    devices: Array<WOLDiscoveredDevice & { name: string }>,
  ) => {
    addingDiscovered.value = true;
    let added = 0;
    try {
      for (const device of devices) {
        await WOLAPI.createTarget({
          name: device.name,
          mac: device.mac,
          relayId: null,
          broadcastAddress: device.broadcastAddress,
          ipAddress: device.ip,
          enabled: true,
        });
        added += 1;
      }
      toast.success(t("admin.wol.discovery.addedCount", { count: added }));
      discoveryOpen.value = false;
      await reload();
    } catch (error) {
      toast.error(t("admin.wol.discovery.addFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wol.discovery.addFailed"),
        ),
      });
      if (added) await reload();
    } finally {
      addingDiscovered.value = false;
    }
  };

  const stop = () => {
    discoveryAbortController?.abort();
    discoveryAbortController = null;
  };

  return {
    addDiscoveredDevices,
    addingDiscovered,
    discoverDevices,
    discovering,
    discoveryOpen,
    discoveryProgress,
    discoveryResult,
    openDiscovery,
    setDiscoveryOpen,
    stop,
  };
};
