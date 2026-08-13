<script setup lang="ts">
import IpBlacklistDetailDialog from "./IpBlacklistDetailDialog.vue";
import IpBlacklistOverview from "./IpBlacklistOverview.vue";
import IpBlacklistRecordsPanel from "./IpBlacklistRecordsPanel.vue";
import { useIpBlacklistPage } from "./useIpBlacklistPage";

const controller = useIpBlacklistPage();
</script>

<template>
  <div class="flex h-full flex-col gap-4">
    <IpBlacklistOverview :controller="controller" />
    <IpBlacklistRecordsPanel :controller="controller" />
    <IpBlacklistDetailDialog :controller="controller" />
  </div>
</template>
