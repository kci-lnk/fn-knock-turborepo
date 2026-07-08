<script setup lang="ts">
import { SystemAPI } from "../../lib/api";
import BinaryResourceSettings from "./BinaryResourceSettings.vue";
</script>

<template>
  <BinaryResourceSettings
    message-key-prefix="admin.cloudflaredSettings"
    :fetch-status="SystemAPI.getCloudflaredStatus"
    :start-download="SystemAPI.startCloudflaredDownload"
    :cancel-download="SystemAPI.cancelCloudflaredDownload"
    :delete-resource="SystemAPI.deleteCloudflared"
    :allow-manage-platforms="['linux-amd64', 'linux-arm64', 'linux-arm']"
  />
</template>
