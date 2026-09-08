<script setup lang="ts">
import { Mail, Loader2, RotateCcw, Save } from "lucide-vue-next";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
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
import BackupEmailSettings from "./BackupEmailSettings.vue";
import BackupEmailStatus from "./BackupEmailStatus.vue";
import { useAutomaticBackupSettings } from "./useAutomaticBackupSettings";
const {
  t,
  details,
  emailForm,
  isLoading,
  isSaving,
  isDirty,
  isValid,
  requestErrorMessage,
  reset,
  save,
  load,
} = useAutomaticBackupSettings(true);
</script>
<template>
  <div class="w-full space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem
          ><BreadcrumbLink href="#/system?tab=maintenance">{{
            t("admin.systemSettingsTabs.maintenance")
          }}</BreadcrumbLink></BreadcrumbItem
        >
        <BreadcrumbSeparator />
        <BreadcrumbItem
          ><BreadcrumbPage>{{
            t("admin.maintenanceSettings.emailTitle")
          }}</BreadcrumbPage></BreadcrumbItem
        >
      </BreadcrumbList>
    </Breadcrumb>
    <div>
      <div class="space-y-1">
        <h1 class="text-xl font-semibold tracking-tight">
          {{ t("admin.maintenanceSettings.emailTitle") }}
        </h1>
        <p class="text-sm leading-6 text-muted-foreground">
          {{ t("admin.maintenanceSettings.emailDescription") }}
        </p>
      </div>
    </div>
    <div
      v-if="isLoading"
      class="space-y-4"
      role="status"
      :aria-label="t('admin.maintenanceSettings.automaticLoading')"
    >
      <Skeleton class="h-28 w-full rounded-xl" /><Skeleton
        class="h-80 w-full rounded-xl"
      />
    </div>
    <div
      v-if="requestErrorMessage"
      class="rounded-xl border border-destructive/25 bg-destructive/5 p-4 text-sm text-destructive"
      role="alert"
    >
      {{ requestErrorMessage
      }}<Button v-if="!details" variant="outline" class="ml-3" @click="load">{{
        t("admin.maintenanceSettings.emailReload")
      }}</Button>
    </div>
    <template v-if="details && !isLoading">
      <div
        v-if="!details.config.enabled"
        class="rounded-xl border bg-muted/30 px-4 py-3 text-sm text-muted-foreground"
      >
        {{ t("admin.maintenanceSettings.emailBackupDisabled") }}
      </div>
      <BackupEmailSettings v-model="emailForm" :disabled="isSaving" />
      <Card
        v-if="emailForm.enabled && details.status.email"
        class="border-border/60 shadow-none"
      >
        <CardHeader
          ><CardTitle class="flex items-center gap-2 text-base"
            ><Mail class="h-4 w-4" />{{
              t("admin.maintenanceSettings.emailDeliveryStatus")
            }}</CardTitle
          ><CardDescription>{{
            t("admin.maintenanceSettings.emailStatusDescription")
          }}</CardDescription></CardHeader
        >
        <CardContent
          ><BackupEmailStatus :status="details.status.email"
        /></CardContent>
      </Card>
      <FloatingActionDock
        :active="isDirty"
        inline-class="flex items-center justify-end gap-3"
      >
        <template #inline>
          <Button
            variant="outline"
            :disabled="!isDirty || isSaving"
            @click="reset"
            ><RotateCcw class="mr-2 h-4 w-4" />{{
              t("admin.maintenanceSettings.resetAutomatic")
            }}</Button
          >
          <Button :disabled="!isDirty || !isValid || isSaving" @click="save"
            ><Loader2 v-if="isSaving" class="mr-2 h-4 w-4 animate-spin" /><Save
              v-else
              class="mr-2 h-4 w-4"
            />{{ t("admin.maintenanceSettings.emailSave") }}</Button
          >
        </template>
      </FloatingActionDock>
    </template>
  </div>
</template>
