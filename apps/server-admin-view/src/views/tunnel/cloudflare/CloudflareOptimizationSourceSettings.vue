<script setup lang="ts">
import { computed } from "vue";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { LoaderCircle, ShieldCheck, TriangleAlert } from "lucide-vue-next";
import {
  optimizationBuiltinLabel,
  optimizationSourceSettingsErrorLabel,
} from "./cloudflareOptimizationPresentation";
import CloudflareResolverDiagnostics from "./CloudflareResolverDiagnostics.vue";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

const { controller } = defineProps<{
  controller: CloudflareTunnelController;
}>();
const {
  isSavingOptimizationSources,
  optimization,
  optimizationBuiltinIds,
  optimizationCustomHostnames,
  optimizationOfficialRanges,
  optimizationScan,
  saveOptimizationSources,
  t,
  toggleOptimizationBuiltin,
} = controller;

const builtinLabel = (id: string, hostname: string) =>
  optimizationBuiltinLabel(id, hostname, t);
const sourceSettingsErrorLabel = (message: string) =>
  optimizationSourceSettingsErrorLabel(message, t);
const resolverDiagnostics = computed(() =>
  optimizationScan.value
    ? optimizationScan.value.resolverDiagnostics
    : optimization.value?.resolverDiagnostics || [],
);
const resolverResolutionPath = computed(
  () =>
    (optimizationScan.value
      ? optimizationScan.value.resolutionPath
      : optimization.value?.resolutionPath) ?? null,
);
</script>

<template>
<details class="rounded-lg border bg-muted/20">
  <summary class="cursor-pointer list-none px-4 py-3">
    <div class="text-sm font-medium">
      {{
        t("admin.cloudflareTunnel.optimization.sources.advancedTitle")
      }}
    </div>
    <div class="mt-1 text-xs text-muted-foreground">
      {{ t("admin.cloudflareTunnel.optimization.sources.description") }}
    </div>
  </summary>
  <div class="space-y-4 border-t p-4">
    <Alert
      v-if="optimization?.candidateSources.error"
      variant="destructive"
      class="items-start"
    >
      <TriangleAlert class="size-4" />
      <AlertDescription>
        {{
          sourceSettingsErrorLabel(optimization.candidateSources.error)
        }}
      </AlertDescription>
    </Alert>

    <div class="flex items-start gap-3 rounded-md border p-3">
      <Checkbox
        id="optimization-official-ranges"
        v-model="optimizationOfficialRanges"
      />
      <Label
        for="optimization-official-ranges"
        class="grid cursor-pointer gap-1 font-normal"
      >
        <span class="text-sm font-medium">
          {{
            t(
              "admin.cloudflareTunnel.optimization.sources.officialRanges",
            )
          }}
        </span>
        <span class="text-xs text-muted-foreground">
          {{
            t(
              "admin.cloudflareTunnel.optimization.sources.officialRangesDescription",
            )
          }}
        </span>
      </Label>
    </div>

    <div>
      <div class="mb-2 text-sm font-medium">
        {{
          t("admin.cloudflareTunnel.optimization.sources.builtinTitle")
        }}
      </div>
      <div class="grid gap-2 sm:grid-cols-2">
        <div
          v-for="source in optimization?.candidateSources.builtins ||
          []"
          :key="source.id"
          class="flex items-start gap-3 rounded-md border p-3"
        >
          <Checkbox
            :id="`optimization-source-${source.id}`"
            :model-value="optimizationBuiltinIds.includes(source.id)"
            @update:model-value="
              (value) =>
                toggleOptimizationBuiltin(source.id, value === true)
            "
          />
          <Label
            :for="`optimization-source-${source.id}`"
            class="grid min-w-0 cursor-pointer gap-1 font-normal"
          >
            <span class="text-sm font-medium">
              {{ builtinLabel(source.id, source.hostname) }}
            </span>
            <code class="truncate text-xs text-muted-foreground">{{
              source.hostname
            }}</code>
          </Label>
        </div>
      </div>
    </div>

    <div class="space-y-2">
      <Label for="optimization-custom-hostnames">
        {{
          t("admin.cloudflareTunnel.optimization.sources.customTitle")
        }}
      </Label>
      <Textarea
        id="optimization-custom-hostnames"
        v-model="optimizationCustomHostnames"
        :rows="4"
        :placeholder="
          t(
            'admin.cloudflareTunnel.optimization.sources.customPlaceholder',
          )
        "
      />
      <div class="text-xs text-muted-foreground">
        {{
          t(
            "admin.cloudflareTunnel.optimization.sources.customDescription",
            {
              max:
                optimization?.candidateSources.maxCustomHostnames || 16,
            },
          )
        }}
      </div>
    </div>

    <Alert class="items-start">
      <ShieldCheck class="size-4" />
      <AlertDescription>
        {{ t("admin.cloudflareTunnel.optimization.sources.safety") }}
      </AlertDescription>
    </Alert>

    <CloudflareResolverDiagnostics
      v-if="resolverDiagnostics.length || resolverResolutionPath"
      :diagnostics="resolverDiagnostics"
      :resolution-path="resolverResolutionPath"
    />

    <div class="flex justify-end">
      <Button
        variant="outline"
        :disabled="isSavingOptimizationSources"
        @click="saveOptimizationSources"
      >
        <LoaderCircle
          v-if="isSavingOptimizationSources"
          class="mr-2 size-4 animate-spin"
        />
        {{ t("admin.cloudflareTunnel.optimization.sources.save") }}
      </Button>
    </div>
  </div>
</details>
</template>
