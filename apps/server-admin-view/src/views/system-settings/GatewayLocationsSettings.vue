<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import GatewayLocationHostPickerDialog from "./gateway-locations/GatewayLocationHostPickerDialog.vue";
import GatewayLocationHostSummary from "./gateway-locations/GatewayLocationHostSummary.vue";
import GatewayLocationRuleDialog from "./gateway-locations/GatewayLocationRuleDialog.vue";
import GatewayLocationRulesTable from "./gateway-locations/GatewayLocationRulesTable.vue";
import { useGatewayLocationsPage } from "./gateway-locations/useGatewayLocationsPage";

const { t } = useI18n();
const controller = useGatewayLocationsPage();
const {
  addHeaderRow,
  availableMappings,
  backToSubdomains,
  closeDialog,
  editingIndex,
  form,
  formError,
  handleHostPickerOpenChange,
  isAvailable,
  isDialogOpen,
  isHostPickerOpen,
  isLoading,
  isMissingHost,
  isProxyLocationWebSocketTarget,
  isSaving,
  openCreateDialog,
  removeHeaderRow,
  saveDialogLocation,
  selectedHost,
  selectedMapping,
  selectHostFromDialog,
  setAction,
  showLoadingSkeleton,
} = controller;
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/mappings?tab=subdomain">
            {{ t("admin.nav.mappingManagement") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>
            {{ t("admin.gatewayLocationsSettings.breadcrumbTitle") }}
          </BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <div
      class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between"
    >
      <div class="max-w-3xl space-y-1.5">
        <h2 class="text-2xl font-semibold tracking-normal">
          {{ t("admin.gatewayLocationsSettings.title") }}
        </h2>
        <p class="text-sm leading-6 text-muted-foreground">
          {{ t("admin.gatewayLocationsSettings.description") }}
        </p>
      </div>
      <Button
        class="w-full sm:w-auto"
        :disabled="!selectedMapping || !isAvailable"
        @click="openCreateDialog"
      >
        {{ t("admin.gatewayLocationsSettings.addRule") }}
      </Button>
    </div>

    <Card class="border-border/60 shadow-none">
      <CardContent class="space-y-5 pt-6">
        <div
          v-if="isLoading && showLoadingSkeleton"
          class="space-y-4 rounded-md border border-border/60 bg-muted/20 p-5"
        >
          <Skeleton class="h-10 w-full rounded-md" />
          <Skeleton class="h-24 w-full rounded-md" />
        </div>

        <template v-else>
          <Alert
            v-if="isMissingHost"
            class="border-destructive/30 bg-destructive/5"
          >
            <AlertTitle>
              {{ t("admin.gatewayLocationsSettings.hostNotFoundTitle") }}
            </AlertTitle>
            <AlertDescription class="space-y-3 text-sm leading-6">
              <p>
                {{
                  t("admin.gatewayLocationsSettings.hostNotFoundDescription", {
                    host: selectedHost,
                  })
                }}
              </p>
              <Button variant="outline" size="sm" @click="backToSubdomains">
                {{ t("admin.gatewayLocationsSettings.backToSubdomains") }}
              </Button>
            </AlertDescription>
          </Alert>

          <Alert v-else-if="!isAvailable" class="border-zinc-200 bg-zinc-50">
            <AlertTitle>
              {{ t("admin.gatewayLocationsSettings.unavailableTitle") }}
            </AlertTitle>
            <AlertDescription class="text-sm leading-6 text-zinc-700">
              {{ t("admin.gatewayLocationsSettings.unavailableDescription") }}
            </AlertDescription>
          </Alert>

          <template v-if="!isMissingHost">
            <GatewayLocationHostSummary :controller="controller" />
            <GatewayLocationRulesTable :controller="controller" />
          </template>
        </template>
      </CardContent>
    </Card>

    <GatewayLocationHostPickerDialog
      :mappings="availableMappings"
      :open="isHostPickerOpen"
      :selected-host="selectedHost"
      :selected-mapping="selectedMapping"
      @select="selectHostFromDialog"
      @update:open="handleHostPickerOpenChange"
    />

    <GatewayLocationRuleDialog
      :open="isDialogOpen"
      :editing-index="editingIndex"
      :form="form"
      :form-error="formError"
      :is-proxy-location-web-socket-target="isProxyLocationWebSocketTarget"
      :is-saving="isSaving"
      @update:open="(open) => !open && closeDialog()"
      @add-header="addHeaderRow"
      @close="closeDialog"
      @remove-header="removeHeaderRow"
      @save="saveDialogLocation"
      @set-action="setAction"
    />
  </div>
</template>
