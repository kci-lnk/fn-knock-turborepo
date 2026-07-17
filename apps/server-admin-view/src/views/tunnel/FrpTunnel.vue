<script setup lang="ts">
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Info,
  Pencil,
  Play,
  Plus,
  ScrollText,
  Square,
  Trash2,
} from "lucide-vue-next";
import LogViewer from "@admin-shared/components/LogViewer.vue";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import LiveStatusBadge from "@/components/LiveStatusBadge.vue";
import { docsUrls } from "@/lib/docs";
import FrpcInstanceEditor from "./frp/FrpcInstanceEditor.vue";
import { useFrpTunnelController } from "./frp/useFrpTunnelController";

withDefaults(
  defineProps<{
    showDocsButton?: boolean;
  }>(),
  { showDocsButton: false },
);

const {
  canStart,
  canStop,
  configLoaded,
  defaults,
  deleteInstance,
  deletingInstanceId,
  extraInstances,
  formatSummary,
  getInstanceDisplayName,
  gotoFrpResources,
  gotoInstanceCreate,
  gotoInstanceDetail,
  isClearingLogs,
  isSaving,
  isStarting,
  isStopping,
  onClearLogsClick,
  overview,
  pid,
  primaryConfig,
  primaryInstance,
  primaryLogs,
  primarySummary,
  running,
  saveConfig,
  setPrimaryEditorRef,
  showInitDialog,
  startFrpc,
  startInstance,
  startingInstanceId,
  stopFrpc,
  stopInstance,
  stoppingInstanceId,
  t,
} = useFrpTunnelController();
</script>

<template>
  <div class="space-y-6">
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
    >
      <div class="space-y-1">
        <h2 class="text-xl font-semibold">{{ t("admin.frpTunnel.title") }}</h2>
        <p class="text-sm text-muted-foreground">
          {{
            t("admin.frpTunnel.runningSummary", {
              running: overview?.runningCount ?? 0,
              total: overview?.total ?? 0,
            })
          }}
        </p>
      </div>
      <div class="flex flex-wrap items-center gap-3">
        <DocsLinkButton
          v-if="showDocsButton"
          :href="docsUrls.guides.tunnel"
          size="default"
          class="shrink-0"
        />
        <Button
          v-if="!running"
          :disabled="!canStart || isStarting"
          @click="startFrpc"
        >
          <Play class="mr-1.5 h-4 w-4" />
          {{ t("admin.frpTunnel.start") }}
        </Button>
        <Button
          v-else
          variant="destructive"
          :disabled="!canStop || isStopping"
          @click="stopFrpc"
        >
          <Square class="mr-1.5 h-4 w-4" />
          {{ t("admin.frpTunnel.stop") }}
        </Button>
      </div>
    </div>

    <ConfigCollapsibleCard
      :title="t('admin.frpTunnel.primaryConfigTitle')"
      :configured="Boolean(primarySummary.serverAddr)"
      :ready="configLoaded"
      summary-class="text-xs text-muted-foreground"
      expanded-content-class="p-0 sm:p-0"
    >
      <template #summary>
        {{ formatSummary(primarySummary) }}
      </template>

      <template #default>
        <FrpcInstanceEditor
          :ref="setPrimaryEditorRef"
          v-model="primaryConfig"
          :defaults="defaults"
          id-prefix="frp-primary"
        />
      </template>

      <template #actions="{ collapse }">
        <div
          class="flex items-center justify-end gap-3 rounded-b-lg border-t bg-muted/30 p-4 sm:px-6 sm:py-4"
        >
          <Button variant="outline" @click="collapse">
            {{ t("admin.frpTunnel.collapse") }}
          </Button>
          <Button
            class="min-w-[100px] shadow-sm"
            :disabled="isSaving"
            @click="saveConfig"
          >
            {{ t("common.save") }}
          </Button>
        </div>
      </template>
    </ConfigCollapsibleCard>

    <Card>
      <CardHeader>
        <div class="flex items-center justify-between gap-3">
          <CardTitle class="text-base">
            {{ t("admin.frpTunnel.primaryConnectionInfo") }}
          </CardTitle>
          <Button
            variant="outline"
            size="sm"
            :disabled="isClearingLogs || primaryLogs.length === 0"
            @click="onClearLogsClick"
          >
            <Trash2 class="mr-1 h-3.5 w-3.5" />
            {{ t("admin.frpTunnel.clear") }}
          </Button>
        </div>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="grid gap-3 text-sm sm:grid-cols-3">
          <div>
            <div class="text-xs text-muted-foreground">
              {{ t("admin.frpTunnel.status") }}
            </div>
            <div class="mt-1 flex items-center gap-2">
              <LiveStatusBadge :active="running" />
              <span
                :class="
                  running ? 'text-green-600' : 'text-muted-foreground'
                "
              >
                {{
                  running
                    ? t("common.active")
                    : t("admin.frpTunnel.notRunning")
                }}
              </span>
            </div>
          </div>
          <div>
            <div class="text-xs text-muted-foreground">PID</div>
            <div class="mt-1 font-mono">{{ pid ?? "-" }}</div>
          </div>
          <div>
            <div class="text-xs text-muted-foreground">
              {{ t("admin.frpTunnel.logAttachment") }}
            </div>
            <div class="mt-1">
              {{
                primaryInstance?.attached
                  ? t("admin.frpTunnel.currentProcess")
                  : t("admin.frpTunnel.historyBuffer")
              }}
            </div>
          </div>
        </div>
        <LogViewer :logs="primaryLogs" reversed :show-header="false" />
      </CardContent>
    </Card>

    <Card class="gap-2">
      <CardHeader>
        <div
          class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
        >
          <div class="space-y-1">
            <CardTitle class="text-base">
              {{ t("admin.frpTunnel.moreFrp") }}
            </CardTitle>
            <p class="text-sm text-muted-foreground">
              {{ t("admin.frpTunnel.moreFrpDescription") }}
            </p>
          </div>
          <Button size="sm" @click="gotoInstanceCreate">
            <Plus class="mr-1.5 h-4 w-4" />
            {{ t("admin.frpTunnel.addFrp") }}
          </Button>
        </div>
      </CardHeader>
      <CardContent class="space-y-3">
        <div
          v-if="extraInstances.length === 0"
          class="rounded-lg border border-dashed px-4 py-6 text-sm text-muted-foreground"
        >
          {{ t("admin.frpTunnel.emptyExtra") }}
        </div>

        <div v-else class="space-y-3">
          <div
            v-for="instance in extraInstances"
            :key="instance.id"
            class="rounded-lg border bg-card px-4 py-4"
          >
            <div
              class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between"
            >
              <div class="min-w-0 space-y-2">
                <div class="flex flex-wrap items-center gap-2">
                  <p class="text-sm font-medium">
                    {{ getInstanceDisplayName(instance) }}
                  </p>
                  <span
                    class="inline-flex items-center gap-1.5 text-xs"
                    :class="
                      instance.running
                        ? 'text-green-600'
                        : 'text-muted-foreground'
                    "
                  >
                    <LiveStatusBadge :active="instance.running" size="xs" />
                    {{
                      instance.running
                        ? t("common.active")
                        : t("admin.frpTunnel.notRunning")
                    }}
                  </span>
                </div>
                <p class="break-all text-sm text-muted-foreground">
                  {{ formatSummary(instance.summary) }}
                </p>
                <p
                  v-if="instance.lastMessage"
                  class="text-xs text-muted-foreground"
                >
                  {{ instance.lastMessage }}
                </p>
              </div>

              <div class="grid gap-3 sm:grid-cols-3 lg:min-w-[360px]">
                <div class="rounded-lg px-3 py-2">
                  <p
                    class="text-[10px] uppercase tracking-wider text-muted-foreground"
                  >
                    PID
                  </p>
                  <p class="mt-1 font-mono text-sm">
                    {{ instance.pid ?? "-" }}
                  </p>
                </div>
                <div class="rounded-lg px-3 py-2">
                  <p
                    class="text-[10px] uppercase tracking-wider text-muted-foreground"
                  >
                    {{ t("admin.frpTunnel.lastStarted") }}
                  </p>
                  <p class="mt-1 text-sm">
                    <HumanFriendlyTime :value="instance.startedAt" />
                  </p>
                </div>
                <div class="rounded-lg px-3 py-2">
                  <p
                    class="text-[10px] uppercase tracking-wider text-muted-foreground"
                  >
                    {{ t("admin.frpTunnel.logs") }}
                  </p>
                  <p class="mt-1 text-sm">
                    {{
                      instance.attached
                        ? t("admin.frpTunnel.liveAttached")
                        : t("admin.frpTunnel.historyBuffer")
                    }}
                  </p>
                </div>
              </div>
            </div>

            <div class="mt-4 flex flex-wrap justify-end gap-2">
              <Button
                variant="outline"
                size="sm"
                @click="gotoInstanceDetail(instance, 'config')"
              >
                <Pencil class="mr-1.5 h-3.5 w-3.5" />
                {{ t("admin.frpTunnel.edit") }}
              </Button>
              <Button
                v-if="!instance.running"
                variant="outline"
                size="sm"
                :disabled="startingInstanceId === instance.id"
                @click="startInstance(instance)"
              >
                <Play class="mr-1.5 h-3.5 w-3.5" />
                {{
                  startingInstanceId === instance.id
                    ? t("admin.frpTunnel.starting")
                    : t("admin.frpTunnel.start")
                }}
              </Button>
              <Button
                v-else
                variant="outline"
                size="sm"
                :disabled="stoppingInstanceId === instance.id"
                @click="stopInstance(instance)"
              >
                <Square class="mr-1.5 h-3.5 w-3.5" />
                {{
                  stoppingInstanceId === instance.id
                    ? t("admin.frpTunnel.stopping")
                    : t("admin.frpTunnel.stop")
                }}
              </Button>
              <Button
                variant="outline"
                size="sm"
                @click="gotoInstanceDetail(instance, 'logs')"
              >
                <ScrollText class="mr-1.5 h-3.5 w-3.5" />
                {{ t("admin.frpTunnel.logs") }}
              </Button>
              <Button
                variant="outline"
                size="sm"
                @click="gotoInstanceDetail(instance)"
              >
                <Info class="mr-1.5 h-3.5 w-3.5" />
                {{ t("admin.frpTunnel.viewMore") }}
              </Button>
              <ConfirmDangerPopover
                :title="t('admin.frpTunnel.deleteTitle')"
                :description="
                  t('admin.frpTunnel.deleteDescription', {
                    name: getInstanceDisplayName(instance),
                  })
                "
                :loading="deletingInstanceId === instance.id"
                :disabled="deletingInstanceId === instance.id"
                :on-confirm="() => deleteInstance(instance)"
                content-class="w-72 text-left"
              >
                <template #trigger>
                  <Button
                    variant="outline"
                    size="sm"
                    :disabled="deletingInstanceId === instance.id"
                    class="text-destructive hover:text-destructive"
                  >
                    <Trash2 class="mr-1.5 h-3.5 w-3.5" />
                    {{
                      deletingInstanceId === instance.id
                        ? t("admin.frpTunnel.deleting")
                        : t("admin.frpTunnel.delete")
                    }}
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>

    <Dialog v-model:open="showInitDialog">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {{ t("admin.frpTunnel.notInitializedTitle") }}
          </DialogTitle>
        </DialogHeader>
        <p class="text-sm text-muted-foreground">
          {{ t("admin.frpTunnel.notInitializedDescription") }}
        </p>
        <DialogFooter>
          <Button @click="gotoFrpResources">
            {{ t("admin.frpTunnel.goInitialize") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
