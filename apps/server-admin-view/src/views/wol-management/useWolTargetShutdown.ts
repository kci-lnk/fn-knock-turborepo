import { ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { WOLAPI, type WOLTarget } from "@/lib/api/wol";
import { updatePendingIds } from "./wol-management-model";
import type { WolTranslate } from "./wol-management-types";

export const useWolTargetShutdown = ({
  refreshTargetById,
  t,
}: {
  refreshTargetById: (id: string, signal?: AbortSignal) => Promise<void>;
  t: WolTranslate;
}) => {
  const shuttingDownTargetIds = ref(new Set<string>());
  const shutdownDialogOpen = ref(false);
  const shutdownDialogTarget = ref<WOLTarget | null>(null);
  const refreshTimers = new Set<number>();

  const openShutdownDialog = (target: WOLTarget) => {
    shutdownDialogTarget.value = target;
    shutdownDialogOpen.value = true;
  };

  const setShutdownDialogOpen = (open: boolean) => {
    shutdownDialogOpen.value = open;
    if (
      !open &&
      shutdownDialogTarget.value &&
      !shuttingDownTargetIds.value.has(shutdownDialogTarget.value.id)
    ) {
      shutdownDialogTarget.value = null;
    }
  };

  const scheduleRefreshes = (id: string) => {
    for (const seconds of [5, 20, 35]) {
      const timer = globalThis.setTimeout(() => {
        refreshTimers.delete(timer);
        void refreshTargetById(id).catch(() => undefined);
      }, seconds * 1_000);
      refreshTimers.add(timer);
    }
  };

  const shutdownTarget = async () => {
    const target = shutdownDialogTarget.value;
    if (!target || shuttingDownTargetIds.value.has(target.id)) return;
    shuttingDownTargetIds.value = updatePendingIds(
      shuttingDownTargetIds.value,
      target.id,
      true,
    );
    try {
      const result = await WOLAPI.shutdownTarget(target.id);
      toast.success(t("admin.wol.ssh.shutdownAccepted"), {
        description: t("admin.wol.ssh.shutdownAcceptedDescription", {
          latency: result.latencyMs,
        }),
      });
      scheduleRefreshes(target.id);
      shutdownDialogOpen.value = false;
    } catch (error) {
      const status = (error as { response?: { status?: number } })?.response
        ?.status;
      const description = extractErrorMessage(
        error,
        t("admin.wol.ssh.shutdownFailed"),
      );
      if (status === 504) {
        toast.warning(t("admin.wol.ssh.shutdownUnknown"), { description });
        scheduleRefreshes(target.id);
        shutdownDialogOpen.value = false;
      } else {
        toast.error(t("admin.wol.ssh.shutdownFailed"), { description });
      }
    } finally {
      shuttingDownTargetIds.value = updatePendingIds(
        shuttingDownTargetIds.value,
        target.id,
        false,
      );
      if (!shutdownDialogOpen.value) shutdownDialogTarget.value = null;
    }
  };

  const stopShutdownRefreshes = () => {
    for (const timer of refreshTimers) globalThis.clearTimeout(timer);
    refreshTimers.clear();
  };

  return {
    openShutdownDialog,
    setShutdownDialogOpen,
    shutdownDialogOpen,
    shutdownDialogTarget,
    shutdownTarget,
    shuttingDownTargetIds,
    stopShutdownRefreshes,
  };
};
