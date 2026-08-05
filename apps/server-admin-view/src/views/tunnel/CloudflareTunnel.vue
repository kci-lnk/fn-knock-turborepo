<script setup lang="ts">
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import LogViewer from "@admin-shared/components/LogViewer.vue";
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

    <CloudflareApiConnectionCard :controller="controller" />
    <CloudflareManagedTunnelCard :controller="controller" />
    <CloudflareOptimizationCard :controller="controller" />
    <CloudflareManualConfigCard :controller="controller" />

    <Card>
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle>{{ t("admin.cloudflareTunnel.runtimeStatus") }}</CardTitle>
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
      </CardHeader>
      <CardContent>
        <div class="mb-4 flex flex-wrap items-start gap-4 text-sm">
          <TunnelSupervisorStatus :supervisor="supervisor" />
          <span v-if="running && pid">PID: {{ pid }}</span>
        </div>
        <Alert
          v-if="cloudflaredLogAnalysis"
          variant="destructive"
          class="mb-4 items-start rounded-xl"
        >
          <TriangleAlert class="size-4" />
          <AlertTitle>{{
            t("admin.cloudflareTunnel.tlsMismatchTitle")
          }}</AlertTitle>
          <AlertDescription>
            <p>{{ cloudflaredLogAnalysisMessage }}</p>
          </AlertDescription>
        </Alert>
        <LogViewer :logs="logs" reversed wrap :show-header="false" />
      </CardContent>
    </Card>

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
