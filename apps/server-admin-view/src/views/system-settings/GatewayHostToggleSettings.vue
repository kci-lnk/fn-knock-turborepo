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
import { Skeleton } from "@/components/ui/skeleton";
import GatewayHostToggleTable from "./gateway-host-toggle/GatewayHostToggleTable.vue";
import type {
  GatewayHostConfigStoreKey,
  GatewayHostToggleDetails,
  GatewayHostToggleField,
} from "./gateway-host-toggle/gatewayHostToggleTypes";
import { useGatewayHostToggleSettings } from "./gateway-host-toggle/useGatewayHostToggleSettings";

const props = defineProps<{
  configStoreKey: GatewayHostConfigStoreKey;
  descriptionCode: string;
  fetchDetails: () => Promise<GatewayHostToggleDetails>;
  messageKeyPrefix: string;
  saveDetails: (payload: {
    disabled_hosts: string[];
  }) => Promise<GatewayHostToggleDetails>;
  toggleColumnLabelKey: string;
  toggleField: GatewayHostToggleField;
}>();

const { t } = useI18n();
const model = useGatewayHostToggleSettings(props);
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">
            {{ t("admin.nav.systemSettings") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=gateway">
            {{ t("admin.systemSettingsTabs.gateway") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{ model.message("title") }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/60 shadow-none">
      <CardHeader class="space-y-3">
        <div class="space-y-1.5">
          <CardTitle class="text-xl">{{ model.message("title") }}</CardTitle>
          <CardDescription class="leading-6">
            {{ model.message("descriptionPrefix") }}
            <code>{{ descriptionCode }}</code>
            {{ model.message("descriptionSuffix") }}
          </CardDescription>
        </div>
      </CardHeader>

      <CardContent class="space-y-6">
        <div
          v-if="model.isLoading && model.showLoadingSkeleton"
          class="space-y-4 rounded-xl border border-border/60 bg-muted/20 p-5"
        >
          <Skeleton class="h-16 w-full rounded-xl" />
          <Skeleton class="h-16 w-full rounded-xl" />
          <Skeleton class="h-16 w-full rounded-xl" />
        </div>
        <div
          v-else-if="model.loadError && !model.details"
          class="rounded-xl border border-destructive/25 bg-destructive/5 px-5 py-4 text-sm text-destructive"
          role="alert"
        >
          {{ model.loadError }}
        </div>
        <GatewayHostToggleTable
          v-else-if="model.details"
          :model="model"
          :toggle-column-label-key="toggleColumnLabelKey"
        />
      </CardContent>
    </Card>
  </div>
</template>
