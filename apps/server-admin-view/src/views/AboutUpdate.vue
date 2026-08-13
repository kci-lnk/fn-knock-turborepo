<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { BookOpen, Github, Globe2, Terminal } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import ReleaseNotesMarkdown from "../components/ReleaseNotesMarkdown.vue";
import {
  OFFICIAL_DOCUMENTATION_URL,
  OFFICIAL_WEBSITE_URL,
} from "../lib/update-presentation";
import AboutUpdateDeploymentNotices from "./about-update/AboutUpdateDeploymentNotices.vue";
import AboutUpdateProgressOverlay from "./about-update/AboutUpdateProgressOverlay.vue";
import AboutUpdateVersionPanel from "./about-update/AboutUpdateVersionPanel.vue";
import { useAboutUpdatePage } from "./about-update/useAboutUpdatePage";

const { t } = useI18n();
const controller = useAboutUpdatePage();
const { openGithub, status, updateSubtitleKey } = controller;
</script>

<template>
  <div class="mx-auto space-y-6">
    <Card class="overflow-hidden border-border/50 shadow-sm">
      <CardContent class="space-y-8">
        <div
          class="flex flex-col gap-4 px-1 sm:flex-row sm:items-start sm:justify-between"
        >
          <div>
            <h2 class="text-2xl font-semibold tracking-tight">
              {{ t("admin.aboutUpdate.title") }}
            </h2>
            <p class="mt-1 text-sm text-muted-foreground">
              {{ t(updateSubtitleKey) }}
            </p>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <Button
              as-child
              variant="outline"
              size="sm"
              class="border-border/70 bg-card shadow-none hover:bg-muted/60 dark:bg-muted/20 dark:hover:bg-muted/35"
            >
              <a
                :href="OFFICIAL_WEBSITE_URL"
                target="_blank"
                rel="noopener noreferrer"
              >
                <Globe2 class="h-4 w-4" />
                {{ t("admin.aboutUpdate.officialWebsite") }}
              </a>
            </Button>
            <Button
              as-child
              variant="outline"
              size="sm"
              class="border-border/70 bg-card shadow-none hover:bg-muted/60 dark:bg-muted/20 dark:hover:bg-muted/35"
            >
              <a
                :href="OFFICIAL_DOCUMENTATION_URL"
                target="_blank"
                rel="noopener noreferrer"
              >
                <BookOpen class="h-4 w-4" />
                {{ t("admin.aboutUpdate.officialDocumentation") }}
              </a>
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              class="rounded-full hover:bg-muted"
              :disabled="!status?.githubUrl"
              :title="t('admin.aboutUpdate.openGithub')"
              :aria-label="t('admin.aboutUpdate.openGithub')"
              @click="openGithub"
            >
              <Github class="h-5 w-5" />
            </Button>
          </div>
        </div>

        <AboutUpdateDeploymentNotices :controller="controller" />
        <AboutUpdateVersionPanel :controller="controller" />

        <div
          v-if="status?.latest?.release_notes"
          class="border-t border-border/40 pt-4"
        >
          <h3
            class="mb-4 flex items-center gap-2 text-sm font-medium text-foreground"
          >
            <Terminal class="h-4 w-4 text-muted-foreground" />
            {{ t("admin.aboutUpdate.releaseNotes") }}
          </h3>
          <div class="rounded-2xl border border-border/40 bg-muted/30 p-5">
            <ReleaseNotesMarkdown
              :source="status.latest.release_notes"
              :fallback="t('admin.aboutUpdate.noReleaseNotes')"
            />
          </div>
        </div>
      </CardContent>
    </Card>

    <AboutUpdateProgressOverlay :controller="controller" />
  </div>
</template>
