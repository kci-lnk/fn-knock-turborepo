<script setup lang="ts">
import AcmeCertificateApplicationsTable from "./AcmeCertificateApplicationsTable.vue";
import AcmeCertificateHeader from "./AcmeCertificateHeader.vue";
import AcmeCertificateWorkflowPanels from "./AcmeCertificateWorkflowPanels.vue";
import { useAcmeCertificateController } from "./useAcmeCertificateController";

const controller = useAcmeCertificateController();
</script>

<template>
  <div class="grid gap-4">
    <AcmeCertificateHeader :controller="controller" />
    <AcmeCertificateApplicationsTable :controller="controller" />
    <AcmeCertificateWorkflowPanels :controller="controller" />
  </div>
</template>
