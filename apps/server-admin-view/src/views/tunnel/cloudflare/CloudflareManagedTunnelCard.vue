<script setup lang="ts">
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { LoaderCircle, RefreshCw, ShieldAlert } from "lucide-vue-next";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

const { controller } = defineProps<{
  controller: CloudflareTunnelController;
}>();
const {
  apiTokenConfigured,
  applyReconcile,
  cloudflaredOriginServiceUrl,
  configLoaded,
  deleteDedicatedTunnel,
  isApplyingReconcile,
  isLoadingManagedState,
  isPreviewingReconcile,
  managedState,
  optimizationEnabled,
  previewReconcile,
  previewCleanup,
  publicWildcardHostname,
  reconcileHasUnconfirmedConflicts,
  reconcilePlan,
  selectedTunnelId,
  t,
  takeoverResourceIds,
  tunnelMode,
} = controller;

const toggleTakeover = (id: string, checked: boolean | "indeterminate") => {
  const next = new Set(takeoverResourceIds.value);
  if (checked === true) next.add(id);
  else next.delete(id);
  takeoverResourceIds.value = [...next];
};

const formatDate = (value?: string | null) => {
  if (!value) return "-";
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleString();
};

const operationKindKeys: Record<string, string> = {
  tunnel: "tunnel",
  ingress: "ingress",
  dns: "dns",
  optimization: "optimization",
  "custom-hostname": "customHostname",
  permission: "permission",
};
const operationActionKeys: Record<string, string> = {
  create: "create",
  update: "update",
  delete: "delete",
  keep: "keep",
  fallback: "fallback",
  probe: "probe",
};
const capabilityKeys: Record<string, string> = {
  zoneRead: "zoneRead",
  tunnelEdit: "tunnelEdit",
  dnsEdit: "dnsEdit",
  sslCertificatesEdit: "sslCertificatesEdit",
};
const operationKindLabel = (value: string) => {
  const key = operationKindKeys[value];
  return key
    ? t(`admin.cloudflareTunnel.managed.operationKinds.${key}`)
    : value;
};
const operationActionLabel = (value: string) => {
  const key = operationActionKeys[value];
  return key
    ? t(`admin.cloudflareTunnel.managed.operationActions.${key}`)
    : value;
};
const capabilityLabel = (value: string) => {
  const key = capabilityKeys[value];
  return key ? t(`admin.cloudflareTunnel.managed.capabilities.${key}`) : value;
};
const capabilityStatusLabel = (capability: {
  required: boolean;
  readable: boolean | null;
}) => {
  if (!capability.required)
    return t("admin.cloudflareTunnel.managed.capabilityNotRequired");
  return capability.readable
    ? t("admin.cloudflareTunnel.managed.capabilityReadable")
    : t("admin.cloudflareTunnel.managed.capabilityMissing");
};
</script>

<template>
  <ConfigCollapsibleCard
    v-if="apiTokenConfigured"
    :title="t('admin.cloudflareTunnel.managed.tunnelTitle')"
    :configured="Boolean(managedState?.managed.tunnel)"
    :ready="configLoaded && !isLoadingManagedState"
    :edit-label="t('admin.cloudflareTunnel.managed.viewOrChange')"
    collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
    summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
    expanded-content-class="p-0 sm:p-0"
  >
    <template #summary>
      {{
        managedState?.managed.tunnel
          ? t("admin.cloudflareTunnel.managed.tunnelSummaryConfigured", {
              hostname: publicWildcardHostname,
            })
          : t("admin.cloudflareTunnel.managed.tunnelSummaryNotConfigured")
      }}
    </template>

    <template #default>
      <div class="space-y-5 p-4 sm:p-6">
        <div>
          <div class="text-base font-semibold">
            {{ t("admin.cloudflareTunnel.managed.tunnelHeading") }}
          </div>
          <p class="mt-1 text-sm text-muted-foreground">
            {{ t("admin.cloudflareTunnel.managed.tunnelIntro") }}
          </p>
        </div>
        <div class="grid gap-4 lg:grid-cols-2">
          <div class="space-y-2">
            <Label for="cloudflare-tunnel-mode">
              {{ t("admin.cloudflareTunnel.managed.tunnelMode") }}
            </Label>
            <Select v-model="tunnelMode">
              <SelectTrigger id="cloudflare-tunnel-mode"
                ><SelectValue
              /></SelectTrigger>
              <SelectContent>
                <SelectItem value="dedicated">
                  {{ t("admin.cloudflareTunnel.managed.dedicatedTunnel") }}
                </SelectItem>
                <SelectItem value="existing">
                  {{ t("admin.cloudflareTunnel.managed.existingTunnel") }}
                </SelectItem>
              </SelectContent>
            </Select>
            <p class="text-xs text-muted-foreground">
              {{
                tunnelMode === "dedicated"
                  ? t("admin.cloudflareTunnel.managed.dedicatedDescription")
                  : t("admin.cloudflareTunnel.managed.existingDescription")
              }}
            </p>
          </div>

          <div v-if="tunnelMode === 'existing'" class="space-y-2">
            <Label for="cloudflare-existing-tunnel">
              {{ t("admin.cloudflareTunnel.managed.selectTunnel") }}
            </Label>
            <Select v-model="selectedTunnelId">
              <SelectTrigger id="cloudflare-existing-tunnel">
                <SelectValue
                  :placeholder="
                    t('admin.cloudflareTunnel.managed.selectTunnel')
                  "
                />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="item in managedState?.tunnels || []"
                  :key="item.id"
                  :value="item.id"
                >
                  {{ item.name }} · {{ item.status || "-" }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div
          class="flex items-start justify-between gap-4 rounded-lg border p-4"
        >
          <div>
            <Label for="cloudflare-optimization-enabled">
              {{ t("admin.cloudflareTunnel.optimization.title") }}
              <Badge variant="secondary" class="ml-1">Beta</Badge>
            </Label>
            <p
              class="mt-1 max-w-3xl text-xs leading-relaxed text-muted-foreground"
            >
              {{ t("admin.cloudflareTunnel.optimization.description") }}
            </p>
          </div>
          <Switch
            id="cloudflare-optimization-enabled"
            v-model="optimizationEnabled"
          />
        </div>

        <div class="flex justify-end">
          <Button
            :disabled="
              isPreviewingReconcile ||
              (tunnelMode === 'existing' && !selectedTunnelId)
            "
            @click="previewReconcile"
          >
            <RefreshCw
              class="mr-2 size-4"
              :class="{ 'animate-spin': isPreviewingReconcile }"
            />
            {{ t("admin.cloudflareTunnel.managed.preview") }}
          </Button>
        </div>

        <div v-if="reconcilePlan" class="space-y-4 rounded-xl border p-4">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <div>
              <div class="font-medium">
                {{ t("admin.cloudflareTunnel.managed.previewTitle") }}
              </div>
              <div class="text-xs text-muted-foreground">
                {{ t("admin.cloudflareTunnel.managed.expiresAt") }}:
                {{ formatDate(reconcilePlan.expiresAt) }}
              </div>
            </div>
            <Badge
              :variant="
                reconcilePlan.conflicts.length ? 'destructive' : 'default'
              "
            >
              {{
                t("admin.cloudflareTunnel.managed.operationCount", {
                  count: reconcilePlan.operations.length,
                })
              }}
            </Badge>
          </div>

          <details class="rounded-lg border bg-muted/20">
            <summary
              class="cursor-pointer list-none px-3 py-2 text-sm font-medium"
            >
              {{ t("admin.cloudflareTunnel.managed.technicalDetails") }}
            </summary>
            <div class="grid gap-2 border-t p-3 sm:grid-cols-2">
              <div
                v-for="(capability, key) in reconcilePlan.capabilities"
                :key="key"
                class="flex items-center justify-between gap-3 rounded-md border bg-background px-3 py-2 text-sm"
              >
                <span>{{ capabilityLabel(key) }}</span>
                <Badge
                  :variant="
                    capability.required && !capability.readable
                      ? 'destructive'
                      : 'outline'
                  "
                >
                  {{ capabilityStatusLabel(capability) }}
                </Badge>
              </div>
            </div>
          </details>

          <div class="grid gap-2 sm:grid-cols-2">
            <div
              v-for="operation in reconcilePlan.operations"
              :key="operation.id"
              class="flex items-start justify-between gap-3 rounded-md border px-3 py-2 text-sm"
            >
              <div class="min-w-0">
                <div class="truncate font-medium">{{ operation.target }}</div>
                <div class="text-xs text-muted-foreground">
                  {{ operationKindLabel(operation.kind) }}
                </div>
              </div>
              <Badge variant="outline">{{
                operationActionLabel(operation.action)
              }}</Badge>
            </div>
          </div>

          <Alert
            v-for="conflict in reconcilePlan.conflicts"
            :key="conflict.id"
            variant="destructive"
            class="items-start"
          >
            <ShieldAlert class="size-4" />
            <AlertTitle>{{ conflict.target }}</AlertTitle>
            <AlertDescription class="space-y-3">
              <p>{{ conflict.message }}</p>
              <label
                v-if="conflict.takeoverAllowed"
                class="flex cursor-pointer items-center gap-2 text-sm"
              >
                <Checkbox
                  :model-value="takeoverResourceIds.includes(conflict.id)"
                  @update:model-value="
                    (checked) => toggleTakeover(conflict.id, checked)
                  "
                />
                {{ t("admin.cloudflareTunnel.managed.confirmTakeover") }}
              </label>
            </AlertDescription>
          </Alert>

          <ul
            v-if="reconcilePlan.warnings.length"
            class="list-disc space-y-1 pl-5 text-xs text-muted-foreground"
          >
            <li v-for="warning in reconcilePlan.warnings" :key="warning">
              {{ warning }}
            </li>
          </ul>

          <div class="flex justify-end">
            <Button
              :disabled="
                isApplyingReconcile || reconcileHasUnconfirmedConflicts
              "
              @click="applyReconcile"
            >
              <LoaderCircle
                v-if="isApplyingReconcile"
                class="mr-2 size-4 animate-spin"
              />
              {{ t("admin.cloudflareTunnel.managed.apply") }}
            </Button>
          </div>
        </div>

        <div
          v-if="managedState?.managed.tunnel"
          class="grid gap-3 sm:grid-cols-3"
        >
          <div class="rounded-md border bg-muted/20 p-3">
            <div class="text-xs text-muted-foreground">Tunnel</div>
            <div class="mt-1 font-medium">
              {{ managedState.managed.tunnel.name }}
            </div>
          </div>
          <div class="rounded-md border bg-muted/20 p-3">
            <div class="text-xs text-muted-foreground">
              {{ t("admin.cloudflareTunnel.managed.publicHostname") }}
            </div>
            <code class="mt-1 block break-all text-sm">{{
              publicWildcardHostname
            }}</code>
          </div>
          <div class="rounded-md border bg-muted/20 p-3">
            <div class="text-xs text-muted-foreground">
              {{ t("admin.cloudflareTunnel.managed.originService") }}
            </div>
            <code class="mt-1 block break-all text-sm">{{
              cloudflaredOriginServiceUrl
            }}</code>
          </div>
        </div>

        <details
          v-if="managedState?.managed.tunnel"
          class="rounded-lg border border-destructive/30"
        >
          <summary
            class="cursor-pointer list-none px-4 py-3 text-sm font-medium text-destructive"
          >
            {{ t("admin.cloudflareTunnel.managed.removeManaged") }}
          </summary>
          <div
            class="flex flex-wrap items-center justify-between gap-3 border-t px-4 py-3"
          >
            <div>
              <p class="text-xs text-muted-foreground">
                {{ t("admin.cloudflareTunnel.managed.cleanupDescription") }}
              </p>
              <label
                v-if="managedState.managed.tunnel.ownership === 'dedicated'"
                class="mt-3 flex cursor-pointer items-center gap-2 text-xs"
              >
                <Checkbox v-model="deleteDedicatedTunnel" />
                {{ t("admin.cloudflareTunnel.managed.deleteDedicatedTunnel") }}
              </label>
            </div>
            <Button
              variant="outline"
              :disabled="isPreviewingReconcile"
              @click="previewCleanup"
            >
              {{ t("admin.cloudflareTunnel.managed.previewCleanup") }}
            </Button>
          </div>
        </details>
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
</template>
