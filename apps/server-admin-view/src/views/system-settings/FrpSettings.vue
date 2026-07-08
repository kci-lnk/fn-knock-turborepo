<script setup lang="ts">
import { SystemAPI } from "../../lib/api";
import BinaryResourceSettings from "./BinaryResourceSettings.vue";
</script>

<template>
  <BinaryResourceSettings
    message-key-prefix="admin.frpSettings"
    :fetch-status="SystemAPI.getFrpStatus"
    :start-download="SystemAPI.startFrpDownload"
    :cancel-download="SystemAPI.cancelFrpDownload"
    :delete-resource="SystemAPI.deleteFrp"
  />
</template>
