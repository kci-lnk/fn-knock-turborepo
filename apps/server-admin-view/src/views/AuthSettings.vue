<script setup lang="ts">
import AuthSettingsDialogs from "./auth-settings/AuthSettingsDialogs.vue";
import AuthSettingsHeader from "./auth-settings/AuthSettingsHeader.vue";
import AuthSettingsTables from "./auth-settings/AuthSettingsTables.vue";
import { useAuthSettingsPage } from "./auth-settings/useAuthSettingsPage";

const controller = useAuthSettingsPage();
</script>

<template>
  <div
    class="dynamic-white-page-card dynamic-white-settings-surface flex h-full flex-col gap-4"
  >
    <AuthSettingsHeader :controller="controller" />
    <AuthSettingsTables :controller="controller" />
  </div>
  <AuthSettingsDialogs :controller="controller" />
</template>
