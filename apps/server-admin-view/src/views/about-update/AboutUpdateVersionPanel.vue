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
import AboutUpdateVersionFlowArrow from "./AboutUpdateVersionFlowArrow.vue";
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
      <AboutUpdateVersionFlowArrow />
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
          {{
            canInstall ? t("admin.aboutUpdate.installRestart") : oneClickLabel
          }}
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

  <Alert v-if="status?.download.error" variant="destructive" class="rounded-xl">
    <AlertCircle class="h-4 w-4" />
    <AlertTitle>{{ t("admin.aboutUpdate.updateFailed") }}</AlertTitle>
    <AlertDescription>{{ status.download.error }}</AlertDescription>
  </Alert>
</template>
