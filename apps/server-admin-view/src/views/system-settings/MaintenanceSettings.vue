<script setup lang="ts">
import AutomaticBackupSettings from "./AutomaticBackupSettings.vue";
import MaintenanceBackupDialogs from "./MaintenanceBackupDialogs.vue";
import MaintenanceBackupPanel from "./MaintenanceBackupPanel.vue";
import MaintenanceDangerZone from "./MaintenanceDangerZone.vue";
import { useMaintenanceBackupWorkflow } from "./useMaintenanceBackupWorkflow";
import { useMaintenanceClearData } from "./useMaintenanceClearData";

const backup = useMaintenanceBackupWorkflow();
const clearData = useMaintenanceClearData();
</script>

<template>
  <div class="w-full">
    <MaintenanceBackupPanel :controller="backup" />

    <section class="mt-6 overflow-hidden rounded-2xl border bg-background">
      <AutomaticBackupSettings
        @files-changed="backup.refreshAutomaticBackupFiles"
      />
    </section>

    <MaintenanceDangerZone :controller="clearData" />
    <MaintenanceBackupDialogs :controller="backup" />
  </div>
</template>
