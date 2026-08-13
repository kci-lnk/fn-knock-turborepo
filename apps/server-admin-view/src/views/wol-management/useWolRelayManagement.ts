import { reactive, ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  WOLAPI,
  type WOLRelay,
  type WOLRelayCredentialResult,
  type WOLRelayInput,
} from "@/lib/api/wol";
import {
  createWolRelayInput,
  updatePendingIds,
} from "./wol-management-model";
import type { WolTranslate } from "./wol-management-types";

export const useWolRelayManagement = ({
  reload,
  t,
}: {
  reload: () => Promise<void>;
  t: WolTranslate;
}) => {
  const relayDialogOpen = ref(false);
  const relayMode = ref<"create" | "edit">("create");
  const editingRelayId = ref("");
  const relayForm = reactive<WOLRelayInput>(createWolRelayInput());
  const savingRelay = ref(false);
  const probingRelayIds = ref(new Set<string>());
  const deletingRelayIds = ref(new Set<string>());
  const rotatingRelayIds = ref(new Set<string>());
  const bootstrapOpen = ref(false);
  const bootstrapCredential = ref<WOLRelayCredentialResult | null>(null);

  const setPending = (
    target: typeof probingRelayIds,
    id: string,
    value: boolean,
  ) => {
    target.value = updatePendingIds(target.value, id, value);
  };

  const openCreateRelay = () => {
    relayMode.value = "create";
    editingRelayId.value = "";
    Object.assign(relayForm, createWolRelayInput());
    relayDialogOpen.value = true;
  };

  const openEditRelay = (relay: WOLRelay) => {
    relayMode.value = "edit";
    editingRelayId.value = relay.id;
    Object.assign(relayForm, {
      name: relay.name,
      address: relay.address,
      port: relay.port,
      enabled: relay.enabled,
    });
    relayDialogOpen.value = true;
  };

  const saveRelay = async () => {
    savingRelay.value = true;
    try {
      if (relayMode.value === "create") {
        const result = await WOLAPI.createRelay({ ...relayForm });
        bootstrapCredential.value = result;
        bootstrapOpen.value = true;
        toast.success(t("admin.wol.relayCreated"));
      } else {
        await WOLAPI.updateRelay(editingRelayId.value, { ...relayForm });
        toast.success(t("admin.wol.relayUpdated"));
      }
      relayDialogOpen.value = false;
      await reload();
    } catch (error) {
      toast.error(t("admin.wol.saveFailed"), {
        description: extractErrorMessage(error, t("admin.wol.saveFailed")),
      });
    } finally {
      savingRelay.value = false;
    }
  };

  const probeRelay = async (relay: WOLRelay) => {
    setPending(probingRelayIds, relay.id, true);
    try {
      const result = await WOLAPI.probeRelay(relay.id);
      toast.success(t("admin.wol.probeSuccess"), {
        description: t("admin.wol.probeSuccessDescription", {
          latency: result.latencyMs,
        }),
      });
    } catch (error) {
      toast.error(t("admin.wol.probeFailed"), {
        description: extractErrorMessage(error, t("admin.wol.probeFailed")),
      });
    } finally {
      setPending(probingRelayIds, relay.id, false);
    }
  };

  const rotateRelay = async (relay: WOLRelay) => {
    setPending(rotatingRelayIds, relay.id, true);
    try {
      const result = await WOLAPI.rotateRelayPsk(relay.id);
      bootstrapCredential.value = result;
      bootstrapOpen.value = true;
      toast.success(t("admin.wol.pskRotated"));
      await reload();
    } catch (error) {
      toast.error(t("admin.wol.rotateFailed"), {
        description: extractErrorMessage(error, t("admin.wol.rotateFailed")),
      });
    } finally {
      setPending(rotatingRelayIds, relay.id, false);
    }
  };

  const deleteRelay = async (relay: WOLRelay) => {
    setPending(deletingRelayIds, relay.id, true);
    try {
      await WOLAPI.deleteRelay(relay.id);
      toast.success(t("admin.wol.relayDeleted"));
      await reload();
    } catch (error) {
      toast.error(t("admin.wol.deleteFailed"), {
        description: extractErrorMessage(error, t("admin.wol.deleteFailed")),
      });
    } finally {
      setPending(deletingRelayIds, relay.id, false);
    }
  };

  const closeBootstrap = (open: boolean) => {
    bootstrapOpen.value = open;
    if (!open) bootstrapCredential.value = null;
  };

  const copyBootstrap = async (value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      toast.success(t("admin.wol.bootstrap.codeCopied"));
    } catch {
      toast.error(t("admin.wol.copyFailed"));
    }
  };

  return {
    bootstrapCredential,
    bootstrapOpen,
    closeBootstrap,
    copyBootstrap,
    deleteRelay,
    deletingRelayIds,
    openCreateRelay,
    openEditRelay,
    probeRelay,
    probingRelayIds,
    relayDialogOpen,
    relayForm,
    relayMode,
    rotateRelay,
    rotatingRelayIds,
    saveRelay,
    savingRelay,
  };
};
