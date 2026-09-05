<script setup lang="ts">
import WebTerminalDialogs from "./WebTerminalDialogs.vue";
import WebTerminalWorkspace from "./WebTerminalWorkspace.vue";
import { useWebTerminalPage } from "./useWebTerminalPage";

const controller = useWebTerminalPage();
</script>

<template>
  <div class="flex h-full min-w-0 flex-col gap-3 sm:gap-4">
    <WebTerminalWorkspace :controller="controller" />
    <WebTerminalDialogs :controller="controller" />
  </div>
</template>
