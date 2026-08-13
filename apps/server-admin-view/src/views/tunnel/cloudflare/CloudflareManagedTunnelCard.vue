<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import ConfirmationDialog from "@admin-shared/components/common/ConfirmationDialog.vue";
import { useConfirmationDialog } from "@admin-shared/composables/useConfirmationDialog";
import { RefreshCw } from "lucide-vue-next";
import type { CloudflareReconcileConflict } from "@/lib/api/tunnel";
import {
  managedTunnelStatusLabel,
  optimizationConflictHostname,
} from "./cloudflareManagedPresentation";
import CloudflareReconcilePlan from "./CloudflareReconcilePlan.vue";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

const { controller } = defineProps<{
  controller: CloudflareTunnelController;
}>();
const {
  apiTokenConfigured,
  cloudflaredOriginServiceUrl,
  configLoaded,
  deleteDedicatedTunnel,
  isApplyingReconcile,
  isLoadingManagedState,
  isPreviewingReconcile,
  managedState,
  optimizationEnabled,
  previewCleanup,
  previewReconcile,
  publicWildcardHostname,
  reconcileAttentionToken,
  selectedTunnelId,
  setOptimizationDomainMode,
  t,
  tunnelMode,
} = controller;

const managedCard = ref<{ expand: () => void } | null>(null);
const {
  confirmationDialogOpen,
  confirmationDialogOptions,
  confirmPendingAction,
  handleConfirmationDialogOpenChange,
  requestConfirmation,
} = useConfirmationDialog();

watch(reconcileAttentionToken, async () => {
  managedCard.value?.expand();
  await nextTick();
  document
    .getElementById("cloudflare-managed-tunnel-card")
    ?.scrollIntoView({ behavior: "smooth", block: "start" });
});

const tunnelStatusLabel = (status?: string | null) =>
  managedTunnelStatusLabel(status, t);
const preserveExistingDns = async (conflict: CloudflareReconcileConflict) => {
  const hostname = optimizationConflictHostname(conflict);
  if (!hostname) return;
  const confirmed = await requestConfirmation({
    title: t(
      "admin.cloudflareTunnel.optimization.domainActions.keepExternalTitle",
    ),
    description: t(
      "admin.cloudflareTunnel.optimization.domainActions.keepExternalDescription",
      { hostname },
    ),
    confirmText: t(
      "admin.cloudflareTunnel.optimization.domainActions.keepExternalConfirm",
    ),
  });
  if (confirmed) await setOptimizationDomainMode(hostname, "external");
};
</script>

<template>
  <ConfigCollapsibleCard
    v-if="apiTokenConfigured"
    id="cloudflare-managed-tunnel-card"
    ref="managedCard"
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
              <SelectTrigger id="cloudflare-tunnel-mode"><SelectValue /></SelectTrigger>
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
                <SelectValue :placeholder="t('admin.cloudflareTunnel.managed.selectTunnel')" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="item in managedState?.tunnels || []"
                  :key="item.id"
                  :value="item.id"
                >
                  {{ item.name }} · {{ tunnelStatusLabel(item.status) }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div class="flex items-start justify-between gap-4 rounded-lg border p-4">
          <div>
            <Label for="cloudflare-optimization-enabled">
              {{ t("admin.cloudflareTunnel.optimization.title") }}
              <Badge variant="secondary" class="ml-1">
                {{ t("admin.cloudflareTunnel.optimization.betaBadge") }}
              </Badge>
            </Label>
            <p class="mt-1 max-w-3xl text-xs leading-relaxed text-muted-foreground">
              {{ t("admin.cloudflareTunnel.optimization.description") }}
            </p>
          </div>
          <Switch id="cloudflare-optimization-enabled" v-model="optimizationEnabled" />
        </div>

        <div class="flex justify-end">
          <Button
            :disabled="
              isPreviewingReconcile ||
              isApplyingReconcile ||
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

        <CloudflareReconcilePlan
          :controller="controller"
          :preserve-existing-dns="preserveExistingDns"
        />

        <div
          v-if="managedState?.managed.tunnel"
          class="grid gap-3 sm:grid-cols-3"
        >
          <div class="rounded-md border bg-muted/20 p-3">
            <div class="text-xs text-muted-foreground">
              {{ t("admin.cloudflareTunnel.managed.tunnelLabel") }}
            </div>
            <div class="mt-1 font-medium">{{ managedState.managed.tunnel.name }}</div>
          </div>
          <div class="rounded-md border bg-muted/20 p-3">
            <div class="text-xs text-muted-foreground">
              {{ t("admin.cloudflareTunnel.managed.publicHostname") }}
            </div>
            <code class="mt-1 block break-all text-sm">{{ publicWildcardHostname }}</code>
          </div>
          <div class="rounded-md border bg-muted/20 p-3">
            <div class="text-xs text-muted-foreground">
              {{ t("admin.cloudflareTunnel.managed.originService") }}
            </div>
            <code class="mt-1 block break-all text-sm">{{ cloudflaredOriginServiceUrl }}</code>
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
              :disabled="isPreviewingReconcile || isApplyingReconcile"
              @click="previewCleanup"
            >
              {{ t("admin.cloudflareTunnel.managed.previewCleanup") }}
            </Button>
          </div>
        </details>
      </div>
    </template>

    <template #actions="{ collapse }">
      <div class="flex justify-end rounded-b-lg border-t bg-muted/30 p-4 sm:px-6">
        <Button variant="outline" @click="collapse">
          {{ t("admin.cloudflareTunnel.collapse") }}
        </Button>
      </div>
    </template>
  </ConfigCollapsibleCard>

  <ConfirmationDialog
    :open="confirmationDialogOpen"
    v-bind="confirmationDialogOptions"
    @update:open="handleConfirmationDialogOpenChange"
    @confirm="confirmPendingAction"
  />
</template>
