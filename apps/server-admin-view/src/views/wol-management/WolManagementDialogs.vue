<script setup lang="ts">
import WOLBootstrapDialog from "./WOLBootstrapDialog.vue";
import WOLDiscoveryDialog from "./WOLDiscoveryDialog.vue";
import WOLPortalSettingsDialog from "./WOLPortalSettingsDialog.vue";
import WOLRelayDialog from "./WOLRelayDialog.vue";
import WOLShutdownDialog from "./WOLShutdownDialog.vue";
import WOLTargetDialog from "./WOLTargetDialog.vue";
import type { WolManagementPageController } from "./useWolManagementPage";

const props = defineProps<{ controller: WolManagementPageController }>();
const {
  addDiscoveredDevices,
  addingDiscovered,
  bootstrapCredential,
  bootstrapOpen,
  closeBootstrap,
  copyBootstrap,
  discoverDevices,
  discovering,
  discoveryOpen,
  discoveryProgress,
  discoveryResult,
  editingTarget,
  existingLocalMacs,
  relayDialogOpen,
  relayForm,
  relayMode,
  relays,
  savePortalSetting,
  saveRelay,
  saveTarget,
  saving,
  savingPortalSetting,
  setDiscoveryOpen,
  settingsOpen,
  showWolInPortal,
  targetDialogError,
  targetDialogOpen,
  targetForm,
  targetMode,
  setShutdownDialogOpen,
  shutdownDialogOpen,
  shutdownDialogTarget,
  shutdownTarget,
  shuttingDownTargetIds,
  testSsh,
  testingSsh,
} = props.controller;
</script>

<template>
  <WOLRelayDialog
    v-model:open="relayDialogOpen"
    :mode="relayMode"
    :model="relayForm"
    :saving="saving"
    @confirm="saveRelay"
  />
  <WOLDiscoveryDialog
    :open="discoveryOpen"
    :result="discoveryResult"
    :progress="discoveryProgress"
    :existing-macs="existingLocalMacs"
    :scanning="discovering"
    :adding="addingDiscovered"
    @update:open="setDiscoveryOpen"
    @scan="discoverDevices"
    @add="addDiscoveredDevices"
  />
  <WOLTargetDialog
    v-model:open="targetDialogOpen"
    :mode="targetMode"
    :model="targetForm"
    :relays="relays"
    :saving="saving"
    :error="targetDialogError"
    :target="editingTarget"
    :testing-ssh="testingSsh"
    @confirm="saveTarget"
    @test-ssh="testSsh"
  />
  <WOLShutdownDialog
    :open="shutdownDialogOpen"
    :target="shutdownDialogTarget"
    :loading="
      Boolean(
        shutdownDialogTarget &&
        shuttingDownTargetIds.has(shutdownDialogTarget.id),
      )
    "
    @update:open="setShutdownDialogOpen"
    @confirm="shutdownTarget"
  />
  <WOLBootstrapDialog
    :open="bootstrapOpen"
    :credential="bootstrapCredential"
    @update:open="closeBootstrap"
    @copy="copyBootstrap"
  />
  <WOLPortalSettingsDialog
    v-model:open="settingsOpen"
    v-model:show-wol="showWolInPortal"
    :saving="savingPortalSetting"
    @save="savePortalSetting"
  />
</template>
