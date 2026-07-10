import { computed, ref, type ComputedRef } from "vue";
import { CloudflaredAPI, FrpcAPI } from "@/lib/api";
import { isCloudflaredTunnelAvailable } from "@/lib/reverse-proxy-submode";
import type { AppConfig } from "@/types";
import { useAsyncAction } from "@admin-shared/composables/useAsyncAction";

type TunnelStatus = {
  running: boolean;
  pid: number | null;
  initialized: boolean;
};

export const useDashboardTunnelStatus = ({
  getConfig,
  loadConfig,
  showTunnelSection,
}: {
  getConfig: () => AppConfig | null;
  loadConfig: () => Promise<AppConfig | null>;
  showTunnelSection: ComputedRef<boolean>;
}) => {
  const frpStatus = ref<TunnelStatus | null>(null);
  const cfStatus = ref<TunnelStatus | null>(null);
  const defaultTunnel = ref<"frp" | "cloudflared">("frp");
  const isInitializing = ref(true);
  const { isPending, run: runLoad } = useAsyncAction();
  const isLoading = computed(() => isInitializing.value || isPending.value);
  let timer: number | null = null;
  let inFlight: Promise<void> | null = null;

  const reset = () => {
    frpStatus.value = null;
    cfStatus.value = null;
    defaultTunnel.value = "frp";
    isInitializing.value = false;
  };

  const load = async () => {
    if (inFlight) return inFlight;

    inFlight = runLoad(async () => {
      if (!showTunnelSection.value) {
        reset();
        return;
      }

      const currentConfig = getConfig();
      const [frp, cloudflared, nextConfig] = await Promise.all([
        FrpcAPI.getStatus().catch(() => null),
        CloudflaredAPI.getStatus().catch(() => null),
        (currentConfig ? Promise.resolve(currentConfig) : loadConfig()).catch(
          () => null,
        ),
      ]);

      if (frp) {
        frpStatus.value = {
          running: frp.running,
          pid: frp.pid,
          initialized: frp.initialized,
        };
      }
      if (cloudflared) {
        cfStatus.value = {
          running: cloudflared.running,
          pid: cloudflared.pid,
          initialized: cloudflared.initialized,
        };
      }
      if (nextConfig) {
        defaultTunnel.value =
          nextConfig.default_tunnel === "cloudflared" &&
          !isCloudflaredTunnelAvailable(nextConfig)
            ? "frp"
            : nextConfig.default_tunnel || "frp";
      }
    })
      .then(() => undefined)
      .finally(() => {
        inFlight = null;
        isInitializing.value = false;
      });

    return inFlight;
  };

  const scheduleLoad = () => {
    if (timer !== null) {
      window.clearTimeout(timer);
      timer = null;
    }

    if (!showTunnelSection.value) {
      reset();
      return;
    }

    if (inFlight) return;
    isInitializing.value = true;
    timer = window.setTimeout(() => {
      timer = null;
      void load();
    }, 0);
  };

  const dispose = () => {
    if (timer !== null) {
      window.clearTimeout(timer);
      timer = null;
    }
  };

  return {
    cfStatus,
    defaultTunnel,
    dispose,
    frpStatus,
    isLoading,
    reset,
    scheduleLoad,
  };
};
