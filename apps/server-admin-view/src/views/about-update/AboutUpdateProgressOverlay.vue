<script setup lang="ts">
import { MonitorUp, RefreshCw } from "lucide-vue-next";
import { Progress } from "@/components/ui/progress";
import type { AboutUpdatePageController } from "./useAboutUpdatePage";

const props = defineProps<{ controller: AboutUpdatePageController }>();
const {
  isDownloadingOrVerifying,
  isUpdateModalVisible,
  modalDescription,
  modalTitle,
  progressText,
  progressValue,
} = props.controller;
</script>

<template>
  <div
    v-if="isUpdateModalVisible"
    class="fixed inset-0 z-[120] flex items-center justify-center bg-background/80 px-4 backdrop-blur-md transition-all duration-300"
  >
    <div
      class="flex w-full max-w-sm flex-col items-center space-y-6 rounded-2xl border border-border/50 bg-background/95 p-8 text-center shadow-2xl"
    >
      <div
        class="flex h-16 w-16 items-center justify-center rounded-full bg-primary/10"
      >
        <RefreshCw
          v-if="isDownloadingOrVerifying"
          class="h-8 w-8 animate-spin text-primary"
        />
        <MonitorUp v-else class="h-8 w-8 animate-pulse text-primary" />
      </div>

      <div class="space-y-2">
        <h3 class="text-lg font-semibold tracking-tight">{{ modalTitle }}</h3>
        <p
          class="text-sm leading-relaxed text-muted-foreground"
          v-html="modalDescription"
        />
      </div>

      <div v-if="isDownloadingOrVerifying" class="w-full space-y-3">
        <Progress :model-value="progressValue" class="h-2 w-full" />
        <div
          class="flex justify-between font-mono text-xs text-muted-foreground"
        >
          <span>{{ progressText }}</span>
          <span class="font-bold text-primary">{{ progressValue }}%</span>
        </div>
      </div>
    </div>
  </div>
</template>
