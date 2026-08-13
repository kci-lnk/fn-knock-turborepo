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
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import GatewayPortalSettingsPanel from "./gateway-portal/GatewayPortalSettingsPanel.vue";
import { useGatewayPortalSettings } from "./gateway-portal/useGatewayPortalSettings";

const { t } = useI18n();
const model = useGatewayPortalSettings();
</script>

<template>
  <div class="space-y-6">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">
            {{ t("admin.gatewayPortalSettings.systemSettings") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=gateway">
            {{ t("admin.gatewayPortalSettings.gateway") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>
            {{ t("admin.gatewayPortalSettings.title") }}
          </BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/60 shadow-none">
      <CardHeader class="space-y-3">
        <div class="space-y-1.5">
          <CardTitle class="text-xl">
            {{ t("admin.gatewayPortalSettings.title") }}
          </CardTitle>
          <CardDescription class="max-w-3xl leading-6">
            {{ t("admin.gatewayPortalSettings.description") }}
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent class="space-y-0 divide-y border-t p-0">
        <div
          v-if="model.isLoading"
          class="px-5 py-12 text-center text-sm text-muted-foreground"
          role="status"
        >
          {{ t("admin.gatewayPortalSettings.loadingConfig") }}
        </div>
        <div
          v-else-if="model.loadError"
          class="px-5 py-4 text-sm text-destructive"
          role="alert"
        >
          {{ model.loadError }}
        </div>
        <GatewayPortalSettingsPanel v-else :model="model" />
      </CardContent>
    </Card>
  </div>
</template>
