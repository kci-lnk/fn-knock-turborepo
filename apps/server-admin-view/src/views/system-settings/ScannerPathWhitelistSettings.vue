<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { RotateCcw } from "lucide-vue-next";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import ScannerPathWhitelistEditor from "./scanner-path-whitelist/ScannerPathWhitelistEditor.vue";
import { useScannerPathWhitelistSettings } from "./scanner-path-whitelist/useScannerPathWhitelistSettings";

const { t } = useI18n();
const model = useScannerPathWhitelistSettings();
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">
            {{ t("admin.scannerPathWhitelist.systemSettings") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=scanner-firewall">
            {{ t("admin.scannerPathWhitelist.scannerFirewall") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{
            t("admin.scannerPathWhitelist.title")
          }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/50 shadow-none">
      <CardHeader>
        <div
          class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between"
        >
          <div class="space-y-1.5">
            <CardTitle class="text-xl tracking-tight">
              {{ t("admin.scannerPathWhitelist.title") }}
            </CardTitle>
            <CardDescription class="max-w-2xl leading-6">
              {{ t("admin.scannerPathWhitelist.description") }}
            </CardDescription>
          </div>
          <Button
            type="button"
            variant="outline"
            class="shrink-0"
            :disabled="
              model.isLoading ||
              model.isSaving ||
              model.isDefault ||
              !model.hasSettings
            "
            @click="model.restoreDefaults"
          >
            <RotateCcw class="mr-2 h-4 w-4" />
            {{ t("admin.scannerPathWhitelist.restoreDefaults") }}
          </Button>
        </div>
      </CardHeader>

      <CardContent
        v-if="model.isLoading && model.showLoadingSkeleton"
        class="border-t pt-6"
      >
        <div class="space-y-4">
          <Skeleton v-for="index in 5" :key="index" class="h-10 w-full" />
        </div>
      </CardContent>
      <CardContent
        v-else-if="model.loadError"
        class="space-y-4 border-t py-8 text-center"
      >
        <p class="text-sm text-destructive" role="alert">
          {{ model.loadError }}
        </p>
        <Button
          type="button"
          variant="outline"
          :disabled="model.isLoading"
          @click="model.fetchSettings"
        >
          {{ t("admin.scannerPathWhitelist.retry") }}
        </Button>
      </CardContent>
      <CardContent v-else-if="model.hasSettings" class="border-t pt-6">
        <ScannerPathWhitelistEditor :model="model" />
      </CardContent>
      <CardContent v-else class="min-h-[260px]" aria-hidden="true" />

      <FloatingActionDock
        v-if="model.hasSettings"
        :active="model.isDirty"
        inline-class="flex items-center justify-between p-6 border-t bg-muted/20 rounded-b-xl"
      >
        <template #inline>
          <div class="text-sm text-muted-foreground">
            {{
              t(
                model.isDirty
                  ? "admin.scannerPathWhitelist.dirty"
                  : "admin.scannerPathWhitelist.clean",
              )
            }}
          </div>
          <div class="flex gap-3">
            <Button
              variant="outline"
              :disabled="!model.isDirty || model.isSaving"
              @click="model.discardChanges"
            >
              {{ t("admin.scannerPathWhitelist.discard") }}
            </Button>
            <Button
              :disabled="
                !model.isDirty ||
                model.isSaving ||
                Object.keys(model.entryErrors).length > 0
              "
              @click="model.saveSettings"
            >
              {{ t("admin.scannerPathWhitelist.save") }}
            </Button>
          </div>
        </template>
        <template #floating>
          <Button
            variant="outline"
            :disabled="!model.isDirty || model.isSaving"
            @click="model.discardChanges"
          >
            {{ t("admin.scannerPathWhitelist.discard") }}
          </Button>
          <Button
            :disabled="
              !model.isDirty ||
              model.isSaving ||
              Object.keys(model.entryErrors).length > 0
            "
            @click="model.saveSettings"
          >
            {{ t("admin.scannerPathWhitelist.save") }}
          </Button>
        </template>
      </FloatingActionDock>
    </Card>
  </div>
</template>
