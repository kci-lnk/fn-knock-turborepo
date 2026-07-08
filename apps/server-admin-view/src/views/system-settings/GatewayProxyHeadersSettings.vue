<script setup lang="ts">
import { ConfigAPI } from "../../lib/api";
import GatewayHostToggleSettings from "./GatewayHostToggleSettings.vue";
</script>

<template>
  <GatewayHostToggleSettings
    config-store-key="gateway_proxy_headers"
    description-code="X-Forwarded-*"
    message-key-prefix="admin.gatewayProxyHeadersSettings"
    toggle-column-label-key="proxyHeaders"
    toggle-field="send_proxy_headers"
    :fetch-details="ConfigAPI.getGatewayProxyHeaders"
    :save-details="ConfigAPI.updateGatewayProxyHeaders"
  />
</template>
