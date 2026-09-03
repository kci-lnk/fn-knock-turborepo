<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ArrowLeft, ShieldOff } from "lucide-vue-next";
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
import SubdomainAdvancedAuthEditor from "./SubdomainAdvancedAuthEditor.vue";
import { useSubdomainAdvancedAuthPage } from "./useSubdomainAdvancedAuthPage";

const { t } = useI18n();
const model = useSubdomainAdvancedAuthPage();
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
          <BreadcrumbPage>{{ t("admin.advancedAuth.title") }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/50 shadow-none">
      <CardHeader>
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="space-y-1.5">
            <CardTitle class="flex items-center gap-2 text-xl">
              <ShieldOff class="h-5 w-5 text-primary" />
              {{ t("admin.advancedAuth.title") }}
            </CardTitle>
            <CardDescription>{{ model.host }}</CardDescription>
          </div>
          <Button variant="outline" @click="model.cancel">
            <ArrowLeft class="mr-2 h-4 w-4" />
            {{ t("admin.advancedAuth.back") }}
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
          {{ model.loadError || t("admin.advancedAuth.notFound") }}
        </p>
        <Button variant="outline" @click="model.cancel">
          {{ t("admin.advancedAuth.back") }}
        </Button>
      </CardContent>
      <SubdomainAdvancedAuthEditor v-else :model="model" />
    </Card>

    <ConfirmationDialog
      :open="model.confirmationDialogOpen"
      v-bind="model.confirmationDialogOptions"
      @update:open="model.handleConfirmationDialogOpenChange"
      @confirm="model.confirmPendingAction"
    />
  </div>
</template>
