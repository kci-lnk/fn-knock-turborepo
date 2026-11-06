<script setup lang="ts">
import AcmeCertificateApplicationsTable from "./AcmeCertificateApplicationsTable.vue";
import AcmeDnsReview from "./AcmeDnsReview.vue";
import AcmeCertificateHeader from "./AcmeCertificateHeader.vue";
import AcmeCertificateWorkflowPanels from "./AcmeCertificateWorkflowPanels.vue";
import { useAcmeCertificateController } from "./useAcmeCertificateController";

const controller = useAcmeCertificateController();
</script>

<template>
  <div class="grid gap-4">
    <p v-if="controller.checkingDns.value" role="status">
      {{ controller.t("admin.acmeDns.checking") }}
    </p>
    <AcmeDnsReview :controller="controller" />
    <AcmeCertificateHeader :controller="controller" />
    <AcmeCertificateApplicationsTable :controller="controller" />
    <AcmeCertificateWorkflowPanels :controller="controller" />
  </div>
</template>
