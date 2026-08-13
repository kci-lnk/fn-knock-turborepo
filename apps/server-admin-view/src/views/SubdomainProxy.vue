<script setup lang="ts">
import SubdomainProxyDialogs from "./subdomain-proxy/SubdomainProxyDialogs.vue";
import SubdomainProxyOverview from "./subdomain-proxy/SubdomainProxyOverview.vue";
import { useSubdomainProxyPage } from "./subdomain-proxy/useSubdomainProxyPage";

const controller = useSubdomainProxyPage();
</script>

<template>
  <div class="space-y-6">
    <SubdomainProxyOverview :controller="controller.overview" />
    <SubdomainProxyDialogs :controller="controller.dialogs" />
  </div>
</template>
