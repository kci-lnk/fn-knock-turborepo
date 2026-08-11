<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import type {
  CloudflareOptimizationResolverDiagnostic,
  CloudflareOptimizationScan,
} from "@/lib/api";
import {
  optimizationResolverProviderLabel,
  optimizationResolverPathLabel,
  optimizationResolverStatusLabel,
} from "./cloudflareOptimizationPresentation";

const props = defineProps<{
  diagnostics: CloudflareOptimizationResolverDiagnostic[];
  resolutionPath: CloudflareOptimizationScan["resolutionPath"];
}>();
const { t } = useI18n();

const providerLabel = (
  provider: CloudflareOptimizationResolverDiagnostic["provider"],
) => optimizationResolverProviderLabel(provider, t);
const statusLabel = (
  status: CloudflareOptimizationResolverDiagnostic["status"],
) => optimizationResolverStatusLabel(status, t);
const availableProviders = computed(() =>
  props.diagnostics
    .filter((diagnostic) => diagnostic.successCount > 0)
    .map((diagnostic) => providerLabel(diagnostic.provider)),
);
const resolutionPath = computed(() =>
  optimizationResolverPathLabel(
    props.resolutionPath,
    availableProviders.value,
    t,
  ),
);
</script>

<template>
  <div class="space-y-2">
    <div class="text-sm font-medium">
      {{
        t(
          "admin.cloudflareTunnel.optimization.sources.resolverDiagnosticsTitle",
        )
      }}
    </div>
    <div class="text-xs text-muted-foreground">{{ resolutionPath }}</div>
    <div class="grid gap-2 sm:grid-cols-2">
      <div
        v-for="diagnostic in diagnostics"
        :key="diagnostic.provider"
        class="rounded-md border px-3 py-2"
      >
        <div class="flex items-center justify-between gap-3">
          <span class="text-sm font-medium">
            {{ providerLabel(diagnostic.provider) }}
          </span>
          <Badge variant="outline">{{ statusLabel(diagnostic.status) }}</Badge>
        </div>
        <div class="mt-1 text-xs text-muted-foreground">
          {{
            t("admin.cloudflareTunnel.optimization.sources.resolverCounts", {
              success: diagnostic.successCount,
              failure: diagnostic.failureCount,
            })
          }}
        </div>
        <div
          v-if="diagnostic.lastErrorCode"
          class="mt-1 break-words text-xs text-muted-foreground"
        >
          <code>{{ diagnostic.lastErrorCode }}</code>
          <span v-if="diagnostic.lastErrorDetail">
            — {{ diagnostic.lastErrorDetail }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>
