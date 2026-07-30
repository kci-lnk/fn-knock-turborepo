<script setup lang="ts">
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  ExternalLink,
  EyeIcon,
  EyeOffIcon,
  TriangleAlert,
  Trash2,
} from "lucide-vue-next";
import LogViewer from "@admin-shared/components/LogViewer.vue";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import TunnelSupervisorStatus from "@/components/TunnelSupervisorStatus.vue";
import { useCloudflareTunnelController } from "./cloudflare/useCloudflareTunnelController";

const {
  authServiceHost,
  canStart,
  canStop,
  cloudflaredLogAnalysis,
  cloudflaredLogAnalysisMessage,
  cloudflaredOriginServiceUrl,
  cloudflaredProtocolDescription,
  cloudflaredProtocolLabel,
  cloudflaredProtocolOptions,
  configLoaded,
  gotoResources,
  hasSubdomainRoot,
  isClearingLogs,
  isReverseProxySubdomainMode,
  isSaving,
  isStarting,
  isStopping,
  logs,
  onClearLogsClick,
  pid,
  protocol,
  publicWildcardHostname,
  saveConfig,
  showInitDialog,
  showToken,
  startCloudflared,
  stopCloudflared,
  supervisor,
  t,
  token,
} = useCloudflareTunnelController();
</script>

<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <h2 class="text-xl font-semibold">
        {{ t("admin.cloudflareTunnel.title") }}
      </h2>
      <div class="flex gap-2">
        <Button
          v-if="!supervisor.desiredRunning && !supervisor.running"
          :disabled="!canStart || isStarting"
          @click="startCloudflared"
        >
          {{ t("admin.cloudflareTunnel.start") }}
        </Button>
        <Button
          v-else
          variant="destructive"
          :disabled="!canStop || isStopping"
          @click="stopCloudflared"
        >
          {{ t("admin.cloudflareTunnel.stop") }}
        </Button>
      </div>
    </div>

    <div class="grid grid-cols-1">
      <ConfigCollapsibleCard
        :title="t('admin.cloudflareTunnel.configTitle')"
        :configured="Boolean(token)"
        :ready="configLoaded"
        expanded-content-class="p-0 sm:p-0"
      >
        <template #summary>
          {{
            t("admin.cloudflareTunnel.configSummary", {
              token: token
                ? "********"
                : t("admin.cloudflareTunnel.notConfigured"),
              protocol: cloudflaredProtocolLabel,
            })
          }}
        </template>

        <template #default>
          <div class="divide-y divide-border">
            <div
              class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
            >
              <div class="mt-1.5 space-y-1">
                <Label
                  for="cloudflared-token"
                  class="flex items-center gap-1 text-sm font-medium"
                >
                  Tunnel Token
                  <span class="text-destructive">*</span>
                </Label>
                <p
                  class="hidden pr-4 text-xs leading-relaxed text-muted-foreground sm:block"
                >
                  {{ t("admin.cloudflareTunnel.tokenDescription") }}
                  {{ t("admin.cloudflareTunnel.tokenDescription") }}
                </p>
              </div>

              <div class="w-full max-w-md space-y-2">
                <div class="relative">
                  <Input
                    id="cloudflared-token"
                    v-model.trim="token"
                    class="pr-10"
                    placeholder="eyJh..."
                    :type="showToken ? 'text' : 'password'"
                    :autocomplete="showToken ? 'off' : 'new-password'"
                    autocapitalize="off"
                    autocorrect="off"
                    :spellcheck="false"
                    data-form-type="other"
                    data-1p-ignore="true"
                    data-lpignore="true"
                    data-bwignore="true"
                  />
                  <button
                    type="button"
                    :aria-label="
                      showToken
                        ? t('common.hideSecret')
                        : t('common.showSecret')
                    "
                    class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground transition-colors hover:text-foreground"
                    @click="showToken = !showToken"
                  >
                    <EyeIcon v-if="showToken" class="h-4 w-4" />
                    <EyeOffIcon v-else class="h-4 w-4" />
                  </button>
                </div>
                <p class="mt-1.5 text-[11px] text-muted-foreground sm:hidden">
                  {{ t("admin.cloudflareTunnel.tokenDescription") }}
                </p>
                <div
                  class="mt-2 space-y-1 text-xs leading-relaxed text-muted-foreground"
                >
                  <p>
                    {{ t("admin.cloudflareTunnel.configSourcePrefix") }}
                    <Button
                      as-child
                      variant="link"
                      data-affordance="details"
                      class="h-auto gap-1 p-0 align-baseline text-xs"
                    >
                      <a
                        href="https://one.dash.cloudflare.com/"
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        Cloudflare Zero Trust Dashboard
                        <ExternalLink class="size-3" aria-hidden="true" />
                      </a>
                    </Button>
                  </p>
                  <p>{{ t("admin.cloudflareTunnel.createTunnelHint") }}</p>
                  <p>{{ t("admin.cloudflareTunnel.copyTokenHint") }}</p>
                </div>
              </div>
            </div>

            <div
              class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
            >
              <div class="mt-1.5 space-y-1">
                <Label for="cloudflared-protocol" class="text-sm font-medium">
                  {{ t("admin.cloudflareTunnel.protocolLabel") }}
                </Label>
                <p
                  class="hidden pr-4 text-xs leading-relaxed text-muted-foreground sm:block"
                >
                  {{ t("admin.cloudflareTunnel.protocolDescription") }}
                </p>
              </div>

              <div class="w-full max-w-md space-y-2">
                <Select v-model="protocol">
                  <SelectTrigger id="cloudflared-protocol" class="w-full">
                    <SelectValue
                      :placeholder="
                        t('admin.cloudflareTunnel.protocolPlaceholder')
                      "
                    />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem
                      v-for="option in cloudflaredProtocolOptions"
                      :key="option.value"
                      :value="option.value"
                    >
                      {{ option.label }}
                    </SelectItem>
                  </SelectContent>
                </Select>
                <p class="text-xs leading-relaxed text-muted-foreground">
                  {{ cloudflaredProtocolDescription }}
                </p>
              </div>
            </div>

            <div
              v-if="isReverseProxySubdomainMode"
              class="space-y-4 p-4 sm:p-6"
            >
              <div class="space-y-1">
                <h3 class="text-sm font-semibold">
                  {{ t("admin.cloudflareTunnel.subdomainChecklistTitle") }}
                </h3>
                <p class="text-xs leading-relaxed text-muted-foreground">
                  {{
                    t("admin.cloudflareTunnel.subdomainChecklistDescription")
                  }}
                </p>
              </div>

              <Alert
                v-if="!hasSubdomainRoot"
                variant="destructive"
                class="items-start rounded-xl"
              >
                <TriangleAlert class="h-4 w-4" />
                <AlertTitle>
                  {{ t("admin.cloudflareTunnel.rootMissingTitle") }}
                </AlertTitle>
                <AlertDescription>
                  {{ t("admin.cloudflareTunnel.rootMissingDescription") }}
                </AlertDescription>
              </Alert>

              <div class="grid gap-3 lg:grid-cols-3">
                <div class="rounded-md border bg-muted/20 p-3">
                  <div class="text-xs font-medium text-muted-foreground">
                    1. Public Hostname
                  </div>
                  <code class="mt-1 block break-all text-sm">
                    {{ publicWildcardHostname }}
                  </code>
                </div>
                <div class="rounded-md border bg-muted/20 p-3">
                  <div class="text-xs font-medium text-muted-foreground">
                    2. Service
                  </div>
                  <code class="mt-1 block break-all text-sm">
                    {{ cloudflaredOriginServiceUrl }}
                  </code>
                </div>
                <div class="rounded-md border bg-muted/20 p-3">
                  <div class="text-xs font-medium text-muted-foreground">
                    3. {{ t("admin.cloudflareTunnel.localAuthHost") }}
                  </div>
                  <code class="mt-1 block break-all text-sm">
                    {{
                      authServiceHost ||
                      t("admin.cloudflareTunnel.notConfigured")
                    }}
                  </code>
                </div>
              </div>

              <div
                class="space-y-2 text-xs leading-relaxed text-muted-foreground"
              >
                <p>{{ t("admin.cloudflareTunnel.serviceHint") }}</p>
              </div>
            </div>
          </div>
        </template>

        <template #actions="{ collapse }">
          <div
            class="flex items-center justify-end gap-3 rounded-b-lg border-t bg-muted/30 p-4 sm:px-6 sm:py-4"
          >
            <Button variant="outline" @click="collapse">
              {{ t("admin.cloudflareTunnel.collapse") }}
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
    </div>

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
            <Trash2 class="mr-1 h-3.5 w-3.5" />
            {{ t("admin.cloudflareTunnel.clear") }}
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <div class="mb-4 flex flex-wrap items-start gap-4 text-sm">
          <TunnelSupervisorStatus :supervisor="supervisor" />
          <span v-if="pid">PID：{{ pid }}</span>
        </div>
        <Alert
          v-if="cloudflaredLogAnalysis"
          variant="destructive"
          class="mb-4 items-start rounded-xl"
        >
          <TriangleAlert class="h-4 w-4" />
          <AlertTitle>
            {{ t("admin.cloudflareTunnel.tlsMismatchTitle") }}
          </AlertTitle>
          <AlertDescription>
            <div class="grid gap-2">
              <p>{{ cloudflaredLogAnalysisMessage }}</p>
              <ul class="list-disc space-y-1 pl-5">
                <li>
                  {{ t("admin.cloudflareTunnel.tlsMismatchAdviceDisableTls") }}
                </li>
                <li>
                  {{ t("admin.cloudflareTunnel.tlsMismatchAdviceUseHttp") }}
                </li>
              </ul>
              <div
                class="break-all rounded-md border border-current/15 bg-background/60 px-3 py-2 font-mono text-xs"
              >
                {{ cloudflaredLogAnalysis.evidence }}
              </div>
            </div>
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
