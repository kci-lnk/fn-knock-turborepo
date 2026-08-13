<script setup lang="ts">
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { LoaderCircle, ShieldAlert } from "lucide-vue-next";
import type {
  CloudflareReconcileConflict,
  CloudflareReconcileOperation,
} from "@/lib/api/tunnel";
import {
  formatCloudflareManagedDate,
  managedCapabilityLabel,
  managedCapabilityStatusLabel,
  managedConflictMessageLabel,
  managedConflictTargetLabel,
  managedDnsOwnerLabel,
  managedDnsProxyLabel,
  managedOperationActionLabel,
  managedOperationKindLabel,
  managedOperationTargetLabel,
  managedPlanWarningLabel,
  optimizationConflictHostname,
} from "./cloudflareManagedPresentation";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

const { controller, preserveExistingDns } = defineProps<{
  controller: CloudflareTunnelController;
  preserveExistingDns: (conflict: CloudflareReconcileConflict) => Promise<void>;
}>();
const {
  applyReconcile,
  isApplyingReconcile,
  locale,
  reconcileHasUnconfirmedConflicts,
  reconcileJob,
  reconcilePlan,
  t,
  takeoverResourceIds,
  updatingOptimizationDomainHostname,
} = controller;

const toggleTakeover = (id: string, checked: boolean | "indeterminate") => {
  const next = new Set(takeoverResourceIds.value);
  if (checked === true) next.add(id);
  else next.delete(id);
  takeoverResourceIds.value = [...next];
};
const formatDate = (value?: string | null) =>
  formatCloudflareManagedDate(value, locale.value);
const operationKindLabel = (value: string) =>
  managedOperationKindLabel(value, t);
const operationActionLabel = (value: string) =>
  managedOperationActionLabel(value, t);
const formatOperationTarget = (operation: CloudflareReconcileOperation) =>
  managedOperationTargetLabel(operation, t);
const conflictTargetLabel = (conflict: CloudflareReconcileConflict) =>
  managedConflictTargetLabel(conflict, t);
const conflictMessageLabel = (conflict: CloudflareReconcileConflict) =>
  managedConflictMessageLabel(conflict, t);
const dnsOwnerLabel = (
  owner: NonNullable<
    CloudflareReconcileConflict["details"]
  >["records"][number]["ownerKind"],
) => managedDnsOwnerLabel(owner, t);
const dnsProxyLabel = (proxied: boolean | null) =>
  managedDnsProxyLabel(proxied, t);
const capabilityLabel = (value: string) => managedCapabilityLabel(value, t);
const capabilityStatusLabel = (capability: {
  required: boolean;
  readable: boolean | null;
}) => managedCapabilityStatusLabel(capability, t);
const planWarningLabel = (warning: string, index: number) =>
  managedPlanWarningLabel(
    warning,
    reconcilePlan.value?.warningCodes?.[index],
    t,
  );
</script>

<template>
  <div
    v-if="isApplyingReconcile && reconcileJob"
    class="flex items-center justify-end gap-2 text-sm text-muted-foreground"
    role="status"
  >
    <LoaderCircle class="size-4 animate-spin" />
    <span>
      {{ t("admin.cloudflareTunnel.managed.apply") }} ·
      {{ reconcileJob.progress }}%
    </span>
  </div>

  <div v-if="reconcilePlan" class="space-y-4 rounded-xl border p-4">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <div>
        <div class="font-medium">
          {{ t("admin.cloudflareTunnel.managed.previewTitle") }}
        </div>
        <div class="text-xs text-muted-foreground">
          {{
            t("admin.cloudflareTunnel.managed.expiresAt", {
              time: formatDate(reconcilePlan.expiresAt),
            })
          }}
        </div>
      </div>
      <Badge :variant="reconcilePlan.conflicts.length ? 'destructive' : 'default'">
        {{
          t("admin.cloudflareTunnel.managed.operationCount", {
            count: reconcilePlan.operations.length,
          })
        }}
      </Badge>
    </div>

    <details class="rounded-lg border bg-muted/20">
      <summary class="cursor-pointer list-none px-3 py-2 text-sm font-medium">
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
          <div class="truncate font-medium">
            {{ formatOperationTarget(operation) }}
          </div>
          <div class="text-xs text-muted-foreground">
            {{ operationKindLabel(operation.kind) }}
          </div>
        </div>
        <Badge variant="outline">{{ operationActionLabel(operation.action) }}</Badge>
      </div>
    </div>

    <Alert
      v-for="conflict in reconcilePlan.conflicts"
      :key="conflict.id"
      variant="destructive"
      class="items-start"
    >
      <ShieldAlert class="size-4" />
      <AlertTitle>{{ conflictTargetLabel(conflict) }}</AlertTitle>
      <AlertDescription class="space-y-3">
        <p>{{ conflictMessageLabel(conflict) }}</p>
        <div
          v-if="conflict.details"
          class="space-y-2 rounded-md border border-destructive/25 bg-background/70 p-3 text-xs"
        >
          <div class="font-medium">
            {{ t("admin.cloudflareTunnel.managed.dnsConflict.current") }}
          </div>
          <div
            v-for="(record, recordIndex) in conflict.details.records"
            :key="`${conflict.id}:${recordIndex}`"
            class="grid gap-1 rounded border px-2 py-1.5 sm:grid-cols-[auto_minmax(0,1fr)_auto] sm:items-center"
          >
            <Badge variant="outline">{{ record.type || "-" }}</Badge>
            <code class="break-all">{{ record.content || "-" }}</code>
            <span class="text-muted-foreground">
              {{ dnsOwnerLabel(record.ownerKind) }} ·
              {{ dnsProxyLabel(record.proxied) }}
            </span>
          </div>
          <div class="font-medium">
            {{ t("admin.cloudflareTunnel.managed.dnsConflict.desired") }}
          </div>
          <div
            class="grid gap-1 rounded border border-primary/25 bg-primary/5 px-2 py-1.5 sm:grid-cols-[auto_minmax(0,1fr)_auto] sm:items-center"
          >
            <Badge variant="outline">{{ conflict.details.desired.type }}</Badge>
            <code class="break-all">{{ conflict.details.desired.content }}</code>
            <span class="text-muted-foreground">
              {{ dnsProxyLabel(conflict.details.desired.proxied) }}
            </span>
          </div>
        </div>
        <label
          v-if="conflict.takeoverAllowed"
          class="flex cursor-pointer items-center gap-2 text-sm"
        >
          <Checkbox
            :model-value="takeoverResourceIds.includes(conflict.id)"
            @update:model-value="toggleTakeover(conflict.id, $event)"
          />
          {{ t("admin.cloudflareTunnel.managed.confirmTakeover") }}
        </label>
        <Button
          v-if="optimizationConflictHostname(conflict)"
          size="sm"
          variant="outline"
          :disabled="Boolean(updatingOptimizationDomainHostname)"
          @click="preserveExistingDns(conflict)"
        >
          {{ t("admin.cloudflareTunnel.optimization.domainActions.keepExternal") }}
        </Button>
      </AlertDescription>
    </Alert>

    <ul
      v-if="reconcilePlan.warnings.length"
      class="list-disc space-y-1 pl-5 text-xs text-muted-foreground"
    >
      <li
        v-for="(warning, index) in reconcilePlan.warnings"
        :key="reconcilePlan.warningCodes?.[index] || warning"
      >
        {{ planWarningLabel(warning, index) }}
      </li>
    </ul>

    <div class="flex justify-end">
      <Button
        :disabled="isApplyingReconcile || reconcileHasUnconfirmedConflicts"
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
</template>
