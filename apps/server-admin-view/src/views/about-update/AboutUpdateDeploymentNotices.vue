<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { AlertCircle, ExternalLink, MonitorUp } from "lucide-vue-next";
import type { AboutUpdatePageController } from "./useAboutUpdatePage";

const props = defineProps<{ controller: AboutUpdatePageController }>();
const { t } = useI18n();
const {
  canSelfUpdate,
  desktopUpdateManaged,
  isFpkLiteDeployment,
  isSynologyDeployment,
  openOfficialWebsite,
  openSynologyWebsite,
  unsupportedDescriptionKey,
} = props.controller;
</script>

<template>
  <Alert
    v-if="desktopUpdateManaged"
    class="rounded-xl border-primary/20 bg-primary/5 text-foreground shadow-none"
  >
    <MonitorUp class="h-4 w-4" />
    <AlertTitle>{{ t("admin.aboutUpdate.desktopManagedTitle") }}</AlertTitle>
    <AlertDescription>
      {{ t("admin.aboutUpdate.desktopManagedDescription") }}
    </AlertDescription>
  </Alert>

  <div
    v-if="isSynologyDeployment"
    class="flex flex-col gap-3 rounded-xl border border-border/50 bg-muted/[0.14] px-4 py-3.5 sm:flex-row sm:items-center sm:justify-between"
  >
    <div class="min-w-0 space-y-0.5">
      <p class="text-sm font-medium text-foreground">
        {{ t("admin.aboutUpdate.synologyUpdateTitle") }}
      </p>
      <p class="text-xs leading-5 text-muted-foreground">
        {{ t("admin.aboutUpdate.synologyUpdateDescription") }}
      </p>
    </div>
    <Button
      variant="outline"
      size="sm"
      class="shrink-0 border-border/70 bg-card shadow-none hover:bg-muted/60 dark:bg-muted/20 dark:hover:bg-muted/35"
      @click="openSynologyWebsite"
    >
      {{ t("admin.aboutUpdate.synologyWebsite") }}
      <ExternalLink class="ml-2 h-3.5 w-3.5" />
    </Button>
  </div>

  <div
    v-if="isFpkLiteDeployment"
    class="flex flex-col gap-4 rounded-xl border border-primary/20 bg-primary/5 px-4 py-4 sm:flex-row sm:items-center sm:justify-between"
  >
    <div class="min-w-0 space-y-1">
      <p class="text-sm font-semibold text-foreground">
        {{ t("admin.aboutUpdate.liteUpdateTitle") }}
      </p>
      <p class="text-xs leading-5 text-muted-foreground">
        {{ t("admin.aboutUpdate.liteUpdateDescription") }}
      </p>
      <p class="text-xs leading-5 text-muted-foreground">
        {{ t("admin.aboutUpdate.liteMigrationSteps") }}
      </p>
    </div>
    <Button size="sm" class="shrink-0 shadow-sm" @click="openOfficialWebsite">
      {{ t("admin.aboutUpdate.liteDownloadFull") }}
      <ExternalLink class="ml-2 h-3.5 w-3.5" />
    </Button>
  </div>

  <Alert
    v-if="
      !canSelfUpdate &&
      !desktopUpdateManaged &&
      !isSynologyDeployment &&
      !isFpkLiteDeployment
    "
    class="rounded-xl border-border/70 bg-muted/30 text-foreground shadow-none"
  >
    <AlertCircle class="h-4 w-4" />
    <AlertTitle>
      {{ t("admin.aboutUpdate.selfUpdateUnsupportedTitle") }}
    </AlertTitle>
    <AlertDescription>{{ t(unsupportedDescriptionKey) }}</AlertDescription>
  </Alert>
</template>
