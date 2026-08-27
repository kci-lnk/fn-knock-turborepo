<script setup lang="ts">
import { Card } from "@/components/ui/card";
import TerminalTargetsNavigation from "./TerminalTargetsNavigation.vue";
import TerminalWorkspacePanel from "./TerminalWorkspacePanel.vue";
import type { WebTerminalPageController } from "./useWebTerminalPage";

const props = defineProps<{ controller: WebTerminalPageController }>();
const { setTerminalShellRef } = props.controller;
</script>

<template>
  <div
    :ref="setTerminalShellRef"
    class="min-h-0 min-w-0 flex-1 md:min-h-[80vh]"
  >
    <Card class="h-full min-h-0 min-w-0 overflow-hidden py-0">
      <div class="flex h-full min-h-0 min-w-0">
        <TerminalTargetsNavigation :controller="controller" />
        <TerminalWorkspacePanel :controller="controller" />
      </div>
    </Card>
  </div>
</template>
