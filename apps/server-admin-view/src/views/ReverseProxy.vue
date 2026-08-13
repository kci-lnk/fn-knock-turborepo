<script setup lang="ts">
import ScanDiscoveryIntensityDialog from "@/components/ScanDiscoveryIntensityDialog.vue";
import ReverseProxyDialogs from "./reverse-proxy/ReverseProxyDialogs.vue";
import ReverseProxyMappingsCard from "./reverse-proxy/ReverseProxyMappingsCard.vue";
import { useReverseProxyPage } from "./reverse-proxy/useReverseProxyPage";

const page = useReverseProxyPage();
</script>

<template>
  <ReverseProxyMappingsCard :model="page" />
  <ScanDiscoveryIntensityDialog
    v-model:open="page.isScanIntensityDialogOpen"
    :disabled="page.isDiscovering"
  />
  <ReverseProxyDialogs :model="page" />
</template>
