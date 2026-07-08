<script setup lang="ts">
import { ConfigAPI } from "../../lib/api";
import GatewayHostToggleSettings from "./GatewayHostToggleSettings.vue";
</script>

<template>
  <GatewayHostToggleSettings
    config-store-key="gateway_host_response"
    description-code="Host"
    message-key-prefix="admin.gatewayHostResponseSettings"
    toggle-column-label-key="host"
    toggle-field="preserve_host"
    :fetch-details="ConfigAPI.getGatewayHostResponse"
    :save-details="ConfigAPI.updateGatewayHostResponse"
  />
</template>
