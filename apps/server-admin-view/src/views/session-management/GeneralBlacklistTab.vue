<script setup lang="ts">
import GeneralBlacklistAddDialog from "./GeneralBlacklistAddDialog.vue";
import GeneralBlacklistRecordsPanel from "./GeneralBlacklistRecordsPanel.vue";
import { useGeneralBlacklistPage } from "./useGeneralBlacklistPage";

const controller = useGeneralBlacklistPage();
</script>

<template>
  <div class="flex h-full flex-col gap-4">
    <GeneralBlacklistRecordsPanel :controller="controller" />
    <GeneralBlacklistAddDialog :controller="controller" />
  </div>
</template>
