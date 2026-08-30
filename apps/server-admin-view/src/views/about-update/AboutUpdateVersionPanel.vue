<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  AlertCircle,
  CheckCircle2,
  RefreshCw,
  Rocket,
  Sparkles,
} from "lucide-vue-next";
import type { AboutUpdatePageController } from "./useAboutUpdatePage";

const props = defineProps<{ controller: AboutUpdatePageController }>();
const { t } = useI18n();
const {
  canInstall,
  canTriggerOneClick,
  checkNow,
  isChecking,
  oneClickLabel,
  oneClickUpdate,
  showOneClickUpdateButton,
  status,
  versionStatusHint,
  versionStatusMessage,
} = props.controller;
</script>

<template>
  <div
    class="flex items-center justify-center rounded-2xl border border-border/50 bg-muted/[0.16] px-4 py-6"
  >
    <div class="flex flex-1 flex-col items-center space-y-1">
      <span class="text-sm font-medium text-muted-foreground">
        {{ t("admin.aboutUpdate.currentVersion") }}
      </span>
      <span class="font-mono text-2xl font-bold tracking-tight text-foreground">
        {{ status?.localVersion || "..." }}
      </span>
    </div>

    <div class="px-4 md:px-8">
      <div class="version-flow-arrow" aria-hidden="true">
        <span class="version-flow-arrow__glyph" />
        <span class="version-flow-arrow__flow" />
      </div>
    </div>

    <div class="flex flex-1 flex-col items-center space-y-1">
      <span class="text-sm font-medium text-muted-foreground">
        {{ t("admin.aboutUpdate.latestVersion") }}
      </span>
      <span
        class="font-mono text-2xl font-bold tracking-tight"
        :class="status?.hasUpdate ? 'text-primary' : 'text-foreground'"
      >
        {{ status?.latest?.version || "..." }}
      </span>
    </div>
  </div>

  <div
    class="flex flex-col items-stretch justify-between gap-6 rounded-xl border border-border/50 bg-muted/[0.14] p-4 sm:flex-row sm:items-center"
  >
    <div class="flex w-full items-center gap-3 sm:w-auto">
      <div
        class="flex h-10 w-10 items-center justify-center rounded-full"
        :class="
          status?.hasUpdate
            ? 'bg-primary/10 text-primary'
            : 'bg-muted text-muted-foreground'
        "
      >
        <Sparkles v-if="status?.hasUpdate" class="h-5 w-5" />
        <CheckCircle2 v-else-if="status?.updateEnabled" class="h-5 w-5" />
        <AlertCircle v-else class="h-5 w-5" />
      </div>
      <div class="space-y-0.5">
        <p class="text-sm font-medium">{{ versionStatusMessage }}</p>
        <p class="text-xs text-muted-foreground">{{ versionStatusHint }}</p>
      </div>
    </div>

    <div class="flex w-full items-center gap-2 sm:w-auto sm:gap-3">
      <Button
        variant="outline"
        class="min-w-0 flex-1 border-border/70 bg-card shadow-none hover:bg-muted/60 sm:w-auto sm:flex-none dark:bg-muted/20 dark:hover:bg-muted/35"
        :disabled="isChecking"
        @click="checkNow"
      >
        <RefreshCw
          class="mr-2 h-4 w-4"
          :class="isChecking ? 'animate-spin' : ''"
        />
        {{ t("admin.aboutUpdate.checkUpdate") }}
      </Button>
      <Button
        v-if="showOneClickUpdateButton"
        class="min-w-0 flex-1 shadow-sm sm:w-auto sm:flex-none"
        :disabled="!canTriggerOneClick"
        :variant="canInstall ? 'destructive' : 'default'"
        @click="oneClickUpdate"
      >
        <Rocket class="mr-2 h-4 w-4" />
        <span class="sm:hidden">
          {{ canInstall ? t("admin.aboutUpdate.installRestart") : oneClickLabel }}
        </span>
        <span class="hidden sm:inline">{{ oneClickLabel }}</span>
      </Button>
    </div>
  </div>

  <Alert v-if="status?.check.error" variant="destructive" class="rounded-xl">
    <AlertCircle class="h-4 w-4" />
    <AlertTitle>{{ t("admin.aboutUpdate.checkFailed") }}</AlertTitle>
    <AlertDescription>{{ status.check.error }}</AlertDescription>
  </Alert>

  <Alert
    v-if="status?.download.error"
    variant="destructive"
    class="rounded-xl"
  >
    <AlertCircle class="h-4 w-4" />
    <AlertTitle>{{ t("admin.aboutUpdate.updateFailed") }}</AlertTitle>
    <AlertDescription>{{ status.download.error }}</AlertDescription>
  </Alert>
</template>

<style scoped>
.version-flow-arrow {
  --version-arrow-mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'%3E%3Cpath d='M5 12h14M12 5l7 7-7 7' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");

  position: relative;
  display: grid;
  width: 2.75rem;
  height: 2.75rem;
  color: var(--foreground);
  place-items: center;
  isolation: isolate;
}

.version-flow-arrow::before {
  z-index: -1;
  width: 1.75rem;
  height: 1.1rem;
  grid-area: 1 / 1;
  border-radius: 999px;
  background: conic-gradient(
    from 90deg,
    #ff4f9a,
    #ffe66d,
    #4fffc1,
    #45caff,
    #8b7cff,
    #ff5fd2,
    #ff4f9a
  );
  content: "";
  opacity: 0;
  filter: blur(0.5rem) saturate(1.25);
  transform: scale(0.72);
  transition:
    opacity 600ms cubic-bezier(0.22, 1, 0.36, 1),
    transform 700ms cubic-bezier(0.16, 1, 0.3, 1);
  animation: version-arrow-aura 6s linear infinite paused;
}

.version-flow-arrow__flow,
.version-flow-arrow__glyph {
  width: 1.5rem;
  height: 1.5rem;
  grid-area: 1 / 1;
  -webkit-mask: var(--version-arrow-mask) center / contain no-repeat;
  mask: var(--version-arrow-mask) center / contain no-repeat;
}

.version-flow-arrow__glyph {
  background: currentColor;
  transition: opacity 450ms cubic-bezier(0.22, 1, 0.36, 1);
}

.version-flow-arrow__flow {
  background-image: linear-gradient(
    110deg,
    #ff4f9a 0%,
    #ff9f43 16%,
    #ffe66d 31%,
    #4fffc1 48%,
    #45caff 65%,
    #8b7cff 82%,
    #ff5fd2 100%
  );
  background-size: 175% 100%;
  opacity: 0;
  filter: saturate(1.05) drop-shadow(0 0 0 rgb(110 196 255 / 0%));
  transform: scale(0.92);
  transition:
    opacity 500ms cubic-bezier(0.22, 1, 0.36, 1),
    filter 600ms ease,
    transform 650ms cubic-bezier(0.16, 1, 0.3, 1);
  animation: version-arrow-flow 3.8s linear infinite paused;
}

.version-flow-arrow:hover .version-flow-arrow__glyph {
  opacity: 0;
}

.version-flow-arrow:hover::before {
  opacity: 0.26;
  transform: scale(1.08);
  animation-play-state: running;
}

.version-flow-arrow:hover .version-flow-arrow__flow {
  opacity: 1;
  filter: saturate(1.15) drop-shadow(0 0 0.22rem rgb(125 183 255 / 48%));
  transform: scale(1);
  animation-play-state: running;
}

@keyframes version-arrow-flow {
  from {
    background-position: 0% 50%;
  }

  to {
    background-position: 150% 50%;
  }
}

@keyframes version-arrow-aura {
  to {
    rotate: 1turn;
  }
}

@media (prefers-reduced-motion: reduce) {
  .version-flow-arrow::before,
  .version-flow-arrow__flow,
  .version-flow-arrow__glyph {
    animation: none;
    background-position: 50% 50%;
  }

  .version-flow-arrow__flow,
  .version-flow-arrow__glyph,
  .version-flow-arrow::before {
    transition-duration: 0.01ms;
  }
}
</style>
