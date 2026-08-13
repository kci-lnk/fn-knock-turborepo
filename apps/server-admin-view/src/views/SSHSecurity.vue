<script setup lang="ts">
import SSHSecurityActivityTabs from "./ssh-security/SSHSecurityActivityTabs.vue";
import SSHSecurityClearFirewallDialog from "./ssh-security/SSHSecurityClearFirewallDialog.vue";
import SSHSecurityConfigurationCard from "./ssh-security/SSHSecurityConfigurationCard.vue";
import { useSSHSecurityConfig } from "./ssh-security/useSSHSecurityConfig";

const controller = useSSHSecurityConfig();
</script>

<template>
  <div class="space-y-4">
    <SSHSecurityConfigurationCard :controller="controller" />
    <SSHSecurityActivityTabs :controller="controller" />
    <SSHSecurityClearFirewallDialog :controller="controller" />
  </div>
</template>
