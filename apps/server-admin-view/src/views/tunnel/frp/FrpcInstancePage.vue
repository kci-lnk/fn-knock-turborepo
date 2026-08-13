<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ArrowLeft, Play, Save, Square, Trash2 } from "lucide-vue-next";
import LogViewer from "@admin-shared/components/LogViewer.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import TunnelSupervisorStatus from "../../../components/TunnelSupervisorStatus.vue";
import FrpcInstanceEditor from "./FrpcInstanceEditor.vue";
import { useFrpcInstancePage } from "./useFrpcInstancePage";

const { t } = useI18n();
const {
  backToList,
  clearLogs,
  content,
  defaults,
  formatSummary,
  instance,
  instanceId,
  isClearingLogs,
  isCreateMode,
  isLoading,
  isSaving,
  isStarting,
  isStopping,
  logs,
  name,
  saveInstance,
  setConfigSectionRef,
  setEditorRef,
  startInstance,
  stopInstance,
  summary,
  title,
} = useFrpcInstancePage();
</script>

<template>
  <div class="space-y-6">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/tunnel?tab=frp">{{
            t("admin.frpcInstancePage.tunnel")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/tunnel?tab=frp">FRP</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{
            isCreateMode ? t("admin.frpcInstancePage.newInstance") : title
          }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="space-y-2">
        <Button
          variant="ghost"
          size="sm"
          class="w-fit px-2"
          @click="backToList"
        >
          <ArrowLeft class="mr-1.5 h-4 w-4" />
          {{ t("admin.frpcInstancePage.backToList") }}
        </Button>
        <div class="space-y-1">
          <h2 class="text-xl font-semibold">{{ title }}</h2>
          <p class="text-sm text-muted-foreground">
            {{ formatSummary(summary) }}
          </p>
        </div>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <Button
          v-if="
            !isCreateMode &&
            instance &&
            !instance.desiredRunning &&
            !instance.running
          "
          variant="outline"
          :disabled="isStarting"
          @click="startInstance"
        >
          <Play class="mr-1.5 h-4 w-4" />
          {{
            isStarting
              ? t("admin.frpcInstancePage.starting")
              : t("admin.frpcInstancePage.start")
          }}
        </Button>
        <Button
          v-if="
            !isCreateMode &&
            instance &&
            (instance.desiredRunning || instance.running)
          "
          variant="destructive"
          :disabled="isStopping"
          @click="stopInstance"
        >
          <Square class="mr-1.5 h-4 w-4" />
          {{
            isStopping
              ? t("admin.frpcInstancePage.stopping")
              : t("admin.frpcInstancePage.stop")
          }}
        </Button>
        <Button :disabled="isSaving || isLoading" @click="saveInstance">
          <Save class="mr-1.5 h-4 w-4" />
          {{ isSaving ? t("admin.frpcInstancePage.saving") : t("common.save") }}
        </Button>
      </div>
    </div>

    <Card v-if="!isCreateMode && instance">
      <CardHeader>
        <CardTitle class="text-base">{{
          t("admin.frpcInstancePage.runtimeInfo")
        }}</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="grid gap-3 text-sm sm:grid-cols-3">
          <div class="rounded-lg border px-4 py-3">
            <p class="text-xs text-muted-foreground">
              {{ t("admin.frpcInstancePage.status") }}
            </p>
            <div class="mt-1 flex items-center gap-2">
              <TunnelSupervisorStatus :supervisor="instance.supervisor" />
            </div>
          </div>
          <div class="rounded-lg border px-4 py-3">
            <p class="text-xs text-muted-foreground">PID</p>
            <p class="mt-1 font-mono">{{ instance.pid ?? "-" }}</p>
          </div>
          <div class="rounded-lg border px-4 py-3">
            <p class="text-xs text-muted-foreground">
              {{ t("admin.frpcInstancePage.logAttachment") }}
            </p>
            <p class="mt-1">
              {{
                instance.attached
                  ? t("admin.frpcInstancePage.currentProcess")
                  : t("admin.frpcInstancePage.historyBuffer")
              }}
            </p>
          </div>
          <div class="rounded-lg border px-4 py-3">
            <p class="text-xs text-muted-foreground">
              {{ t("admin.frpcInstancePage.lastStarted") }}
            </p>
            <p class="mt-1">
              <HumanFriendlyTime :value="instance.startedAt" />
            </p>
          </div>
          <div class="rounded-lg border px-4 py-3">
            <p class="text-xs text-muted-foreground">
              {{ t("admin.frpcInstancePage.lastStopped") }}
            </p>
            <p class="mt-1">
              <HumanFriendlyTime :value="instance.stoppedAt" />
            </p>
          </div>
          <div class="rounded-lg border px-4 py-3">
            <p class="text-xs text-muted-foreground">
              {{ t("admin.frpcInstancePage.createdAt") }}
            </p>
            <p class="mt-1">
              <HumanFriendlyTime :value="instance.createdAt" />
            </p>
          </div>
        </div>

        <div class="grid gap-3 text-sm md:grid-cols-2">
          <div class="rounded-lg border px-4 py-3">
            <p class="text-xs text-muted-foreground">
              {{ t("admin.frpcInstancePage.configPath") }}
            </p>
            <p class="mt-1 break-all font-mono text-xs">
              {{ instance.configPath }}
            </p>
          </div>
          <div class="rounded-lg border px-4 py-3">
            <p class="text-xs text-muted-foreground">
              {{ t("admin.frpcInstancePage.workDir") }}
            </p>
            <p class="mt-1 break-all font-mono text-xs">
              {{ instance.workDir }}
            </p>
          </div>
        </div>

        <p
          v-if="instance.lastMessage"
          class="rounded-lg border bg-muted/20 px-4 py-3 text-sm text-muted-foreground"
        >
          {{ instance.lastMessage }}
        </p>
      </CardContent>
    </Card>

    <div :ref="setConfigSectionRef">
      <Card>
        <CardHeader>
          <CardTitle class="text-base">{{
            t("admin.frpcInstancePage.instanceConfig")
          }}</CardTitle>
        </CardHeader>
        <CardContent class="space-y-4">
          <div class="grid gap-2 sm:grid-cols-[160px_1fr] sm:items-start">
            <div class="space-y-1 mt-1.5">
              <Label for="frp-instance-name">{{
                t("admin.frpcInstancePage.name")
              }}</Label>
              <p class="hidden text-xs text-muted-foreground sm:block">
                {{ t("admin.frpcInstancePage.nameHint") }}
              </p>
            </div>
            <Input
              id="frp-instance-name"
              v-model="name"
              :placeholder="t('admin.frpcInstancePage.namePlaceholder')"
            />
          </div>

          <FrpcInstanceEditor
            :ref="setEditorRef"
            v-model="content"
            :defaults="defaults"
            :id-prefix="
              isCreateMode
                ? 'frp-instance-create'
                : `frp-instance-${instanceId}`
            "
          />
        </CardContent>
      </Card>
    </div>

    <div v-if="!isCreateMode">
      <Card>
        <CardHeader>
          <div class="flex items-center justify-between gap-3">
            <CardTitle class="text-base">{{
              t("admin.frpcInstancePage.instanceLogs")
            }}</CardTitle>
            <Button
              variant="outline"
              size="sm"
              :disabled="isClearingLogs || logs.length === 0"
              @click="clearLogs"
            >
              <Trash2 class="mr-1.5 h-3.5 w-3.5" />
              {{ t("admin.frpcInstancePage.clear") }}
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <LogViewer :logs="logs" reversed :show-header="false" />
        </CardContent>
      </Card>
    </div>
  </div>
</template>
