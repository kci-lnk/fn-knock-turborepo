import { reactive, ref, watch, type Ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  WOLAPI,
  type WOLTarget,
  type WOLTargetInput,
} from "@/lib/api/wol";
import { createVisibilityPoller } from "@/composables/useVisibilityPolling";
import { createRandomTargetName } from "@/lib/wolTargetName";
import {
  createWolTargetInput,
  updatePendingIds,
  wolTargetToEditInput,
} from "./wol-management-model";
import type { WolTranslate } from "./wol-management-types";

export const useWolTargetManagement = ({
  reload,
  t,
  targets,
}: {
  reload: () => Promise<void>;
  t: WolTranslate;
  targets: Ref<WOLTarget[]>;
}) => {
  const targetDialogOpen = ref(false);
  const targetDialogError = ref("");
  const targetMode = ref<"create" | "edit">("create");
  const editingTargetId = ref("");
  const targetForm = reactive<WOLTargetInput>(createWolTargetInput());
  const savingTarget = ref(false);
  const wakingTargetIds = ref(new Set<string>());
  const deletingTargetIds = ref(new Set<string>());

  const setPending = (id: string, value: boolean, deleting = false) => {
    const target = deleting ? deletingTargetIds : wakingTargetIds;
    target.value = updatePendingIds(target.value, id, value);
  };

  const openCreateTarget = () => {
    targetDialogError.value = "";
    targetMode.value = "create";
    editingTargetId.value = "";
    Object.assign(
      targetForm,
      createWolTargetInput(
        createRandomTargetName(t("admin.wol.targetDialog.generatedNamePrefix")),
      ),
    );
    targetDialogOpen.value = true;
  };

  const openEditTarget = (target: WOLTarget) => {
    targetDialogError.value = "";
    targetMode.value = "edit";
    editingTargetId.value = target.id;
    Object.assign(targetForm, wolTargetToEditInput(target));
    targetDialogOpen.value = true;
  };

  const refreshEditingTargetRuntime = async (signal: AbortSignal) => {
    const id = editingTargetId.value;
    if (!targetDialogOpen.value || targetMode.value !== "edit" || !id) return;
    try {
      const refreshed = await WOLAPI.getTarget(id, signal);
      if (!targetDialogOpen.value || editingTargetId.value !== id) return;
      const index = targets.value.findIndex((target) => target.id === id);
      if (index >= 0) targets.value.splice(index, 1, refreshed);
    } catch {
      // Runtime polling must not replace a save error or close the editor.
    }
  };

  const targetRuntimePoller = createVisibilityPoller({
    intervalMs: 2_000,
    enabled: () =>
      targetDialogOpen.value &&
      targetMode.value === "edit" &&
      Boolean(editingTargetId.value),
    task: refreshEditingTargetRuntime,
  });

  watch([targetDialogOpen, targetMode, editingTargetId], () => {
    targetRuntimePoller.sync();
  });

  const saveTarget = async () => {
    savingTarget.value = true;
    targetDialogError.value = "";
    try {
      if (targetMode.value === "create") {
        const { integrations: _integrations, ...createPayload } = targetForm;
        await WOLAPI.createTarget({ ...createPayload });
        toast.success(t("admin.wol.targetCreated"));
      } else {
        await WOLAPI.updateTarget(editingTargetId.value, { ...targetForm });
        toast.success(t("admin.wol.targetUpdated"));
      }
      targetDialogOpen.value = false;
      await reload();
    } catch (error) {
      targetDialogError.value = extractErrorMessage(
        error,
        t("admin.wol.saveFailed"),
      );
      toast.error(t("admin.wol.saveFailed"), {
        description: targetDialogError.value,
      });
    } finally {
      savingTarget.value = false;
    }
  };

  const wakeTarget = async (target: WOLTarget) => {
    setPending(target.id, true);
    try {
      const result = await WOLAPI.wakeTarget(target.id);
      const local = result.deliveryMode === "local";
      toast.success(
        t(local ? "admin.wol.localWakeSent" : "admin.wol.wakeAccepted"),
        {
          description: t(
            local
              ? "admin.wol.localWakeSentDescription"
              : "admin.wol.wakeAcceptedDescription",
            { latency: result.latencyMs },
          ),
        },
      );
    } catch (error) {
      const status = (error as { response?: { status?: number } })?.response
        ?.status;
      const description = extractErrorMessage(error, t("admin.wol.wakeFailed"));
      if (status === 504) {
        toast.warning(t("admin.wol.wakeUnknown"), { description });
      } else {
        toast.error(t("admin.wol.wakeFailed"), { description });
      }
    } finally {
      setPending(target.id, false);
    }
  };

  const deleteTarget = async (target: WOLTarget) => {
    setPending(target.id, true, true);
    try {
      await WOLAPI.deleteTarget(target.id);
      toast.success(t("admin.wol.targetDeleted"));
      await reload();
    } catch (error) {
      toast.error(t("admin.wol.deleteFailed"), {
        description: extractErrorMessage(error, t("admin.wol.deleteFailed")),
      });
    } finally {
      setPending(target.id, false, true);
    }
  };

  return {
    deleteTarget,
    deletingTargetIds,
    editingTargetId,
    openCreateTarget,
    openEditTarget,
    saveTarget,
    savingTarget,
    startPolling: targetRuntimePoller.start,
    stopPolling: targetRuntimePoller.stop,
    targetDialogError,
    targetDialogOpen,
    targetForm,
    targetMode,
    wakeTarget,
    wakingTargetIds,
  };
};
