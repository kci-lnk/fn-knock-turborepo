<script setup lang="ts">
import { ArrowLeft, Route, ShieldOff } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
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
import ConfirmationDialog from "@admin-shared/components/common/ConfirmationDialog.vue";
import StreamBypassPolicyEditor from "./StreamBypassPolicyEditor.vue";
import { useStreamBypassPolicyPage } from "./useStreamBypassPolicyPage";

const { t } = useI18n();
const model = useStreamBypassPolicyPage();
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/mappings?tab=protocol">
            {{ t("admin.nav.mappingManagement") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{
            t("admin.streamMappings.bypassPolicyTitle")
          }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/50 shadow-none">
      <CardHeader>
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="space-y-1.5">
            <CardTitle class="flex items-center gap-2 text-xl">
              <ShieldOff class="h-5 w-5 text-primary" />
              {{ t("admin.streamMappings.bypassPolicyTitle") }}
            </CardTitle>
            <CardDescription class="flex items-center gap-2">
              <Route class="h-3.5 w-3.5" />
              {{ model.mappingLabel }}
            </CardDescription>
          </div>
          <Button variant="outline" @click="model.cancel">
            <ArrowLeft class="mr-2 h-4 w-4" />
            {{ t("admin.streamMappings.policyBack") }}
          </Button>
        </div>
      </CardHeader>

      <CardContent
        v-if="model.loading"
        class="py-12 text-center text-muted-foreground"
      >
        {{ t("common.loadingConfig") }}
      </CardContent>
      <CardContent v-else-if="model.missing" class="space-y-4 py-8">
        <p
          class="rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive"
        >
          {{ model.loadError || t("admin.streamMappings.policyNotFound") }}
        </p>
        <Button variant="outline" @click="model.cancel">
          {{ t("admin.streamMappings.policyBack") }}
        </Button>
      </CardContent>
      <StreamBypassPolicyEditor v-else :model="model" />
    </Card>

    <ConfirmationDialog
      :open="model.confirmationDialogOpen"
      v-bind="model.confirmationDialogOptions"
      @update:open="model.handleConfirmationDialogOpenChange"
      @confirm="model.confirmPendingAction"
    />
  </div>
</template>
