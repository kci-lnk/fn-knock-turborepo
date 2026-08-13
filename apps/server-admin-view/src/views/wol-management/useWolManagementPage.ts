import { computed, onBeforeUnmount, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useWolDiscovery } from "./useWolDiscovery";
import { useWolLocalRelay } from "./useWolLocalRelay";
import { useWolPortalSettings } from "./useWolPortalSettings";
import { useWolRelayManagement } from "./useWolRelayManagement";
import { useWolResources } from "./useWolResources";
import { useWolTargetManagement } from "./useWolTargetManagement";

export function useWolManagementPage() {
  const { t } = useI18n();
  const resources = useWolResources(t);
  const localRelayController = useWolLocalRelay({
    applyLocalRelay: resources.applyLocalRelay,
    localRelayForm: resources.localRelayForm,
    t,
  });
  const relayController = useWolRelayManagement({
    reload: resources.load,
    t,
  });
  const targetController = useWolTargetManagement({
    reload: resources.load,
    t,
    targets: resources.targets,
  });
  const discoveryController = useWolDiscovery({
    reload: resources.load,
    t,
  });
  const portalController = useWolPortalSettings(t);

  const saving = computed(
    () => relayController.savingRelay.value || targetController.savingTarget.value,
  );
  const existingLocalMacs = computed(() =>
    resources.targets.value
      .filter((target) => target.deliveryMode === "local")
      .map((target) => target.mac),
  );
  const editingTarget = computed(() => {
    if (targetController.targetMode.value !== "edit") return null;
    return (
      resources.targets.value.find(
        (target) => target.id === targetController.editingTargetId.value,
      ) ?? null
    );
  });

  onMounted(() => {
    targetController.startPolling();
    void resources.load();
  });
  onBeforeUnmount(() => {
    discoveryController.stop();
    targetController.stopPolling();
  });

  return {
    ...resources,
    ...localRelayController,
    ...relayController,
    ...targetController,
    ...discoveryController,
    ...portalController,
    editingTarget,
    existingLocalMacs,
    saving,
  };
}

export type WolManagementPageController = ReturnType<
  typeof useWolManagementPage
>;
