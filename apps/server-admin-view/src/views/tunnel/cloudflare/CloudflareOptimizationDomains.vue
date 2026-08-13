<script setup lang="ts">
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import ConfirmationDialog from "@admin-shared/components/common/ConfirmationDialog.vue";
import { useConfirmationDialog } from "@admin-shared/composables/useConfirmationDialog";
import type { CloudflareOptimizationDomain } from "@/lib/api/tunnel";
import {
  optimizationDomainMessageLabel,
  optimizationDomainStatusLabel,
} from "./cloudflareOptimizationPresentation";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

const props = defineProps<{ controller: CloudflareTunnelController }>();
const {
  optimization,
  prepareOptimizationConflictResolution,
  setOptimizationDomainMode,
  t,
  updatingOptimizationDomainHostname,
} = props.controller;
const {
  confirmationDialogOpen,
  confirmationDialogOptions,
  confirmPendingAction,
  handleConfirmationDialogOpenChange,
  requestConfirmation,
} = useConfirmationDialog();

const domainStatusLabel = (status: string) =>
  optimizationDomainStatusLabel(status, t);
const domainMessageLabel = (domain: CloudflareOptimizationDomain) =>
  optimizationDomainMessageLabel(domain, t);
const preserveExistingDns = async (domain: CloudflareOptimizationDomain) => {
  const confirmed = await requestConfirmation({
    title: t(
      "admin.cloudflareTunnel.optimization.domainActions.keepExternalTitle",
    ),
    description: t(
      "admin.cloudflareTunnel.optimization.domainActions.keepExternalDescription",
      { hostname: domain.hostname },
    ),
    confirmText: t(
      "admin.cloudflareTunnel.optimization.domainActions.keepExternalConfirm",
    ),
  });
  if (confirmed) {
    await setOptimizationDomainMode(domain.hostname, "external");
  }
};
const retryDomainOptimization = (domain: CloudflareOptimizationDomain) =>
  setOptimizationDomainMode(domain.hostname, "optimize");
</script>

<template>
  <details
    v-if="optimization?.domains.length"
    class="rounded-lg border bg-muted/20"
  >
    <summary class="cursor-pointer list-none px-4 py-3 text-sm font-medium">
      {{ t("admin.cloudflareTunnel.optimization.domainStatus") }}
    </summary>
    <div class="grid gap-2 border-t p-4 sm:grid-cols-2">
      <div
        v-for="domain in optimization.domains"
        :key="domain.hostname"
        class="flex items-start justify-between gap-3 rounded-md border px-3 py-2"
      >
        <div class="min-w-0">
          <div class="truncate font-mono text-sm">{{ domain.hostname }}</div>
          <div
            v-if="domain.cleanupPending"
            class="mt-1 text-xs text-amber-700 dark:text-amber-300"
          >
            {{
              t(
                "admin.cloudflareTunnel.optimization.domainActions.externalCleanupPending",
              )
            }}
          </div>
          <div v-else-if="domain.message" class="mt-1 text-xs text-destructive">
            {{ domainMessageLabel(domain) }}
          </div>
        </div>
        <div class="flex shrink-0 flex-col items-end gap-2">
          <Badge :variant="domain.optimized ? 'default' : 'secondary'">
            {{ domainStatusLabel(domain.status) }}
          </Badge>
          <div
            v-if="domain.actionRequired"
            class="flex flex-wrap justify-end gap-1.5"
          >
            <Button
              size="sm"
              variant="outline"
              :disabled="Boolean(updatingOptimizationDomainHostname)"
              @click="preserveExistingDns(domain)"
            >
              {{
                t(
                  "admin.cloudflareTunnel.optimization.domainActions.keepExternal",
                )
              }}
            </Button>
            <Button
              size="sm"
              :disabled="Boolean(updatingOptimizationDomainHostname)"
              @click="prepareOptimizationConflictResolution"
            >
              {{
                t(
                  "admin.cloudflareTunnel.optimization.domainActions.resolveConflict",
                )
              }}
            </Button>
          </div>
          <Button
            v-else-if="domain.managementMode === 'external'"
            size="sm"
            variant="outline"
            :disabled="Boolean(updatingOptimizationDomainHostname)"
            @click="retryDomainOptimization(domain)"
          >
            {{
              t(
                "admin.cloudflareTunnel.optimization.domainActions.enableOptimization",
              )
            }}
          </Button>
        </div>
      </div>
    </div>
  </details>

  <ConfirmationDialog
    :open="confirmationDialogOpen"
    v-bind="confirmationDialogOptions"
    @update:open="handleConfirmationDialogOpenChange"
    @confirm="confirmPendingAction"
  />
</template>
