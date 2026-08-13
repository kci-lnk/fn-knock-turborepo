<script setup lang="ts">
import WebTerminalDialogs from "./web-terminal/WebTerminalDialogs.vue";
import WebTerminalWorkspace from "./web-terminal/WebTerminalWorkspace.vue";
import { useWebTerminalPage } from "./web-terminal/useWebTerminalPage";

const controller = useWebTerminalPage();
</script>

<template>
  <div class="flex h-full min-w-0 flex-col gap-3 sm:gap-4">
    <WebTerminalWorkspace :controller="controller" />
    <WebTerminalDialogs :controller="controller" />
  </div>
</template>
