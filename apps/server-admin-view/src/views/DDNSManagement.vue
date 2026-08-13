<script setup lang="ts">
import DDNSManagementContent from "./ddns-management/DDNSManagementContent.vue";
import { useDDNSManagementPage } from "./ddns-management/useDDNSManagementPage";

const controller = useDDNSManagementPage();
</script>

<template>
  <DDNSManagementContent :controller="controller" />
</template>
