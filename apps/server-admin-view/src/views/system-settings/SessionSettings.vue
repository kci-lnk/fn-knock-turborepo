<script setup lang="ts">
import { useId } from "vue";
import { useI18n } from "vue-i18n";
import { Info } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import SessionDurationFieldRow from "./SessionDurationFieldRow.vue";
import SessionAccessPolicyPanel from "./session-settings/SessionAccessPolicyPanel.vue";
import { useSessionSettingsController } from "./session-settings/useSessionSettingsController";

const a11yId = useId();
const { t } = useI18n();
const controller = useSessionSettingsController();
const {
  durationUnits,
  form,
  formatDuration,
  isDirty,
  isLoading,
  isSaving,
  rememberMeTtlSeconds,
  resetForm,
  saveSettings,
  sessionTtlSeconds,
  showLoadingSkeleton,
} = controller;
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle class="text-md">
        {{ t("admin.sessionSettings.title") }}
      </CardTitle>
      <CardDescription class="mt-1.5">
        {{ t("admin.sessionSettings.description") }}
      </CardDescription>
    </CardHeader>

    <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="divide-y border-t p-0">
      <div class="border-b border-border bg-muted/20 px-6 py-5">
        <Alert
          class="items-start rounded-xl border-border/70 bg-muted/30 text-foreground shadow-none"
        >
          <Info class="mt-0.5 h-4 w-4 shrink-0" />
          <AlertTitle>
            {{ t("admin.sessionSettings.newSessionsOnlyTitle") }}
          </AlertTitle>
          <AlertDescription class="text-sm leading-6">
            {{ t("admin.sessionSettings.newSessionsOnlyDescription") }}
          </AlertDescription>
        </Alert>
      </div>

      <SessionDurationFieldRow
        v-model="form.session"
        :title="t('admin.sessionSettings.sessionTtl')"
        :description="t('admin.sessionSettings.sessionTtlDescription')"
        :units="durationUnits"
        :disabled="isSaving"
        :summary="
          t('admin.sessionSettings.willSaveAs', {
            duration: formatDuration(sessionTtlSeconds),
          })
        "
      />

      <SessionDurationFieldRow
        v-model="form.rememberMe"
        :title="t('admin.sessionSettings.rememberMeTtl')"
        :description="t('admin.sessionSettings.rememberMeTtlDescription')"
        :units="durationUnits"
        :disabled="isSaving"
        :summary="
          t('admin.sessionSettings.willSaveAs', {
            duration: formatDuration(rememberMeTtlSeconds),
          })
        "
      />

      <SessionAccessPolicyPanel
        :controller="controller"
        :mobility-switch-id="`${a11yId}-sessionsettings-1`"
      />
    </CardContent>

    <CardContent v-else class="min-h-[200px]" aria-hidden="true" />

    <FloatingActionDock
      :active="isDirty"
      inline-class="flex items-center justify-between rounded-b-xl border-t bg-muted/20 p-6"
    >
      <template #inline>
        <div class="text-sm text-muted-foreground">
          <span v-if="isDirty">
            {{ t("admin.sessionSettings.unsavedChanges") }}
          </span>
          <span v-else>{{ t("admin.sessionSettings.upToDate") }}</span>
        </div>
        <div class="flex gap-3">
          <Button
            variant="outline"
            :disabled="!isDirty || isSaving"
            @click="resetForm"
          >
            {{ t("admin.sessionSettings.discard") }}
          </Button>
          <Button :disabled="!isDirty || isSaving" @click="saveSettings">
            <span
              v-if="isSaving"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            />
            {{ t("admin.sessionSettings.saveChanges") }}
          </Button>
        </div>
      </template>

      <template #floating>
        <Button
          variant="outline"
          :disabled="!isDirty || isSaving"
          @click="resetForm"
        >
          {{ t("admin.sessionSettings.discard") }}
        </Button>
        <Button :disabled="!isDirty || isSaving" @click="saveSettings">
          <span
            v-if="isSaving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          />
          {{ t("admin.sessionSettings.saveChanges") }}
        </Button>
      </template>
    </FloatingActionDock>
  </Card>
</template>
