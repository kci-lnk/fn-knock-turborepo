import { ref, type Reactive } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  WOLAPI,
  type WOLLocalRelay,
  type WOLLocalRelayInput,
} from "@/lib/api/wol";
import type { WolTranslate } from "./wol-management-types";

export const useWolLocalRelay = ({
  applyLocalRelay,
  localRelayForm,
  t,
}: {
  applyLocalRelay: (result: WOLLocalRelay) => void;
  localRelayForm: Reactive<WOLLocalRelayInput>;
  t: WolTranslate;
}) => {
  const savingLocalRelay = ref(false);

  const refreshLocalRelayRuntime = async () => {
    for (const delay of [100, 250, 500]) {
      await new Promise<void>((resolve) => globalThis.setTimeout(resolve, delay));
      try {
        const result = await WOLAPI.getLocalRelay();
        applyLocalRelay(result);
        if (
          result.runtime.lastError ||
          result.runtime.active === result.config.enabled
        ) {
          break;
        }
      } catch {
        break;
      }
    }
  };

  const saveLocalRelay = async () => {
    savingLocalRelay.value = true;
    try {
      const psk = localRelayForm.psk?.trim();
      const result = await WOLAPI.updateLocalRelay({
        ...localRelayForm,
        broadcastDestinations: [...localRelayForm.broadcastDestinations],
        allowedSources: [...localRelayForm.allowedSources],
        psk: psk || undefined,
      });
      applyLocalRelay(result);
      await refreshLocalRelayRuntime();
      toast.success(t("admin.wol.localRelay.saved"));
    } catch (error) {
      toast.error(t("admin.wol.localRelay.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wol.localRelay.saveFailed"),
        ),
      });
    } finally {
      savingLocalRelay.value = false;
    }
  };

  const pairLocalRelay = async (pairingCode: string) => {
    savingLocalRelay.value = true;
    try {
      const result = await WOLAPI.pairLocalRelay(pairingCode);
      applyLocalRelay(result);
      await refreshLocalRelayRuntime();
      toast.success(t("admin.wol.localRelay.paired"));
    } catch (error) {
      toast.error(t("admin.wol.localRelay.pairFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wol.localRelay.pairFailed"),
        ),
      });
    } finally {
      savingLocalRelay.value = false;
    }
  };

  return { pairLocalRelay, saveLocalRelay, savingLocalRelay };
};
