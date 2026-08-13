import { reactive, ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import {
  WOLAPI,
  type WOLLocalRelay,
  type WOLRelay,
  type WOLTarget,
} from "@/lib/api/wol";
import {
  createWolLocalRelayInput,
  wolLocalRelayToInput,
} from "./wol-management-model";
import type { WolTranslate } from "./wol-management-types";

export const useWolResources = (t: WolTranslate) => {
  const relays = ref<WOLRelay[]>([]);
  const targets = ref<WOLTarget[]>([]);
  const localRelay = ref<WOLLocalRelay | null>(null);
  const localRelayForm = reactive(createWolLocalRelayInput());
  const loading = ref(true);
  const loadError = ref("");

  const applyLocalRelay = (result: WOLLocalRelay) => {
    localRelay.value = result;
    Object.assign(localRelayForm, wolLocalRelayToInput(result));
  };

  const load = async () => {
    loading.value = true;
    loadError.value = "";
    try {
      const [relayResult, targetResult, localRelayResult] = await Promise.all([
        WOLAPI.listRelays(),
        WOLAPI.listTargets(),
        WOLAPI.getLocalRelay(),
      ]);
      relays.value = relayResult.items;
      targets.value = targetResult.items;
      applyLocalRelay(localRelayResult);
    } catch (error) {
      loadError.value = extractErrorMessage(error, t("admin.wol.loadFailed"));
    } finally {
      loading.value = false;
    }
  };

  const statusLabel = (target: WOLTarget) =>
    t(`admin.wol.status.${target.status.state}`);
  const checkedAtLabel = (target: WOLTarget) => {
    if (!target.status.checkedAt) return t("admin.wol.status.notChecked");
    return t("admin.wol.status.checkedAt", {
      time: new Date(target.status.checkedAt).toLocaleString(),
    });
  };

  return {
    applyLocalRelay,
    checkedAtLabel,
    load,
    loadError,
    loading,
    localRelay,
    localRelayForm,
    relays,
    statusLabel,
    targets,
  };
};
