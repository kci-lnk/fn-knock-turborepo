<script setup lang="ts">
import { SystemAPI } from "@/lib/api/system";
import BinaryResourceSettings from "./BinaryResourceSettings.vue";
</script>

<template>
  <BinaryResourceSettings
    message-key-prefix="admin.cloudflaredSettings"
    :fetch-status="SystemAPI.getCloudflaredStatus"
    :start-download="SystemAPI.startCloudflaredDownload"
    :cancel-download="SystemAPI.cancelCloudflaredDownload"
    :delete-resource="SystemAPI.deleteCloudflared"
    version-aware
    :allow-manage-platforms="[
      'darwin-amd64',
      'darwin-arm64',
      'linux-386',
      'linux-amd64',
      'linux-arm',
      'linux-armhf',
      'linux-arm64',
      'windows-386',
      'windows-amd64',
    ]"
  />
</template>
