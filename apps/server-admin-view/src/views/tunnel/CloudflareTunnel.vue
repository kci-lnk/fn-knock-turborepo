<script setup lang="ts">
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import LogViewer from "@admin-shared/components/LogViewer.vue";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import { LoaderCircle, Trash2, TriangleAlert } from "lucide-vue-next";
import TunnelSupervisorStatus from "@/components/TunnelSupervisorStatus.vue";
import CloudflareApiConnectionCard from "./cloudflare/CloudflareApiConnectionCard.vue";
import CloudflareManagedTunnelCard from "./cloudflare/CloudflareManagedTunnelCard.vue";
import CloudflareManualConfigCard from "./cloudflare/CloudflareManualConfigCard.vue";
import CloudflareOptimizationCard from "./cloudflare/CloudflareOptimizationCard.vue";
import { useCloudflareTunnelController } from "./cloudflare/useCloudflareTunnelController";

const controller = useCloudflareTunnelController();
const {
  canStart,
  canStop,
  cloudflaredLogAnalysis,
  cloudflaredLogAnalysisMessage,
  configLoaded,
  gotoResources,
  hasSubdomainRoot,
  isClearingLogs,
  isReverseProxySubdomainMode,
  isStarting,
  isStopping,
  logs,
  onClearLogsClick,
  pid,
  running,
  showInitDialog,
  startCloudflared,
  stopCloudflared,
  supervisor,
  t,
} = controller;
</script>

<template>
  <div class="space-y-6">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h2 class="text-xl font-semibold">
          {{ t("admin.cloudflareTunnel.title") }}
        </h2>
        <p class="mt-1 text-sm text-muted-foreground">
          {{ t("admin.cloudflareTunnel.managed.pageDescription") }}
        </p>
      </div>
      <div class="flex gap-2">
        <Button
          v-if="!supervisor.desiredRunning && !supervisor.running"
          :disabled="!canStart || isStarting"
          @click="startCloudflared"
        >
          <LoaderCircle v-if="isStarting" class="mr-2 size-4 animate-spin" />
          {{ t("admin.cloudflareTunnel.start") }}
        </Button>
        <Button
          v-else
          variant="destructive"
          :disabled="!canStop || isStopping"
          @click="stopCloudflared"
        >
          <LoaderCircle v-if="isStopping" class="mr-2 size-4 animate-spin" />
          {{ t("admin.cloudflareTunnel.stop") }}
        </Button>
      </div>
    </div>

    <Alert
      v-if="!isReverseProxySubdomainMode || !hasSubdomainRoot"
      variant="destructive"
      class="items-start rounded-xl"
    >
      <TriangleAlert class="size-4" />
      <AlertTitle>{{
        t("admin.cloudflareTunnel.rootMissingTitle")
      }}</AlertTitle>
      <AlertDescription>
        {{ t("admin.cloudflareTunnel.rootMissingDescription") }}
      </AlertDescription>
    </Alert>

    <ConfigCollapsibleCard
      :title="t('admin.cloudflareTunnel.runtimeStatus')"
      :configured="false"
      :ready="configLoaded"
      :edit-label="t('admin.cloudflareTunnel.managed.viewDetails')"
      collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
      summary-class="max-w-full"
      expanded-content-class="p-0 sm:p-0"
    >
      <template #summary>
        <div class="flex flex-wrap items-center gap-3">
          <TunnelSupervisorStatus :supervisor="supervisor" compact />
          <span v-if="running && pid" class="text-xs text-muted-foreground">
            PID: {{ pid }}
          </span>
        </div>
      </template>

      <template #default>
        <div class="space-y-4 p-4 sm:p-6">
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div class="flex flex-wrap items-start gap-4 text-sm">
              <TunnelSupervisorStatus :supervisor="supervisor" />
              <span v-if="running && pid">PID: {{ pid }}</span>
            </div>
            <Button
              variant="outline"
              size="sm"
              :disabled="isClearingLogs || logs.length === 0"
              @click="onClearLogsClick"
            >
              <Trash2 class="mr-1 size-3.5" />
              {{ t("admin.cloudflareTunnel.clear") }}
            </Button>
          </div>
          <LogViewer :logs="logs" reversed wrap :show-header="false" />
        </div>
      </template>

      <template #actions="{ collapse }">
        <div
          class="flex justify-end rounded-b-lg border-t bg-muted/30 p-4 sm:px-6"
        >
          <Button variant="outline" @click="collapse">
            {{ t("admin.cloudflareTunnel.collapse") }}
          </Button>
        </div>
      </template>
    </ConfigCollapsibleCard>

    <Alert
      v-if="cloudflaredLogAnalysis"
      variant="destructive"
      class="items-start rounded-xl"
    >
      <TriangleAlert class="size-4" />
      <AlertTitle>{{
        t("admin.cloudflareTunnel.tlsMismatchTitle")
      }}</AlertTitle>
      <AlertDescription>
        <p>{{ cloudflaredLogAnalysisMessage }}</p>
      </AlertDescription>
    </Alert>

    <CloudflareApiConnectionCard :controller="controller" />
    <CloudflareManagedTunnelCard :controller="controller" />
    <CloudflareOptimizationCard :controller="controller" />
    <CloudflareManualConfigCard :controller="controller" />

    <Dialog v-model:open="showInitDialog">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {{ t("admin.cloudflareTunnel.notInitializedTitle") }}
          </DialogTitle>
        </DialogHeader>
        <p class="text-sm text-muted-foreground">
          {{ t("admin.cloudflareTunnel.notInitializedDescription") }}
        </p>
        <DialogFooter>
          <Button @click="gotoResources">
            {{ t("admin.cloudflareTunnel.goInitialize") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
