<script setup lang="ts">
import ReverseProxyDefaultRouteDialog from "./ReverseProxyDefaultRouteDialog.vue";
import ReverseProxyDiscoverDialog from "./ReverseProxyDiscoverDialog.vue";
import ReverseProxyMappingDialog from "./ReverseProxyMappingDialog.vue";
import type { ReverseProxyPageModel } from "./useReverseProxyPage";

defineProps<{ model: ReverseProxyPageModel }>();
</script>

<template>
  <ReverseProxyMappingDialog
    :open="model.isMappingDialogOpen"
    :form="model.newMapping"
    :is-editing="model.isEditing"
    :is-saving="model.isSaving"
    :is-valid="model.isValid"
    :is-web-socket-target="model.isNewMappingWebSocketTarget"
    @update:open="model.handleMappingDialogOpenChange"
    @update-form="model.updateMappingDraft"
    @close="model.closeMappingDialog(true)"
    @save="model.saveMapping"
  />

  <ReverseProxyDiscoverDialog
    :ref="model.setDiscoverTargetsSettingsRef"
    :selected-services="model.selectedServices"
    :discovered-data="model.discoveredData"
    :is-all-selected="model.isAllSelected"
    :is-discovering="model.isDiscovering"
    :is-saving="model.isSaving"
    :is-selection-valid="model.isDiscoverSelectionValid"
    :is-settings-open="model.isDiscoverSettingsOpen"
    :open="model.isDiscoverDialogOpen"
    :resolve-service-host="model.resolveDiscoveredServiceHost"
    :show-host-column="model.showDiscoverHostColumn"
    @cancel="model.dismissDiscoverDialog"
    @save="model.saveDiscoveredServices"
    @scan="model.triggerScan"
    @stop-scan="model.stopDiscoverScan"
    @toggle-all="model.onToggleAllDiscoverSelect"
    @toggle-settings="model.toggleDiscoverSettings"
    @update:open="model.handleDiscoverDialogOpenChange"
    @update:selected-services="model.selectedServices = $event"
  />

  <ReverseProxyDefaultRouteDialog
    :open="model.isDefaultRouteConfirmOpen"
    :title="model.defaultRouteDialogTitle"
    :description="model.defaultRouteDialogDescription"
    :show-fnos-hint="model.showDefaultRouteFnosHint"
    :saving="model.isSavingDefaultRoute"
    @update:open="model.handleDefaultRouteConfirmOpenChange"
    @cancel="model.closeDefaultRouteConfirm"
    @confirm="model.confirmDefaultRouteChange"
  />
</template>
