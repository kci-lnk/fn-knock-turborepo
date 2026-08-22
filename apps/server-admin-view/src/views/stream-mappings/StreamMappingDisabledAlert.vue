<script setup lang="ts">
import { computed } from "vue";
import { TriangleAlert } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import type { ProtocolMappingFeatureConfig } from "@/types";

type ProtocolMappingRuntimeIssue = NonNullable<
  ProtocolMappingFeatureConfig["runtime_issue"]
>;

const props = defineProps<{
  runtimeIssue?: ProtocolMappingRuntimeIssue | null;
}>();

const { t } = useI18n();
const protocolLabel = computed(
  () => props.runtimeIssue?.protocol?.toUpperCase() ?? "TCP/UDP",
);
const issueSummary = computed(() => {
  const issue = props.runtimeIssue;
  if (!issue) return "";
  if (
    issue.code === "local_port_loop" &&
    issue.listen_port !== null &&
    issue.target
  ) {
    return t("admin.streamMappings.runtimeIssueLocalLoop", {
      protocol: protocolLabel.value,
      port: issue.listen_port,
      target: issue.target,
    });
  }
  if (issue.code === "listen_port_in_use" && issue.listen_port !== null) {
    return t("admin.streamMappings.runtimeIssuePortInUse", {
      protocol: protocolLabel.value,
      port: issue.listen_port,
    });
  }
  return t("admin.streamMappings.runtimeIssueFallback");
});
</script>

<template>
  <Alert
    class="items-start rounded-xl border-amber-200 bg-amber-50/80 text-amber-950 shadow-none"
  >
    <TriangleAlert class="mt-0.5 h-4 w-4 shrink-0" />
    <div class="min-w-0 space-y-2">
      <AlertTitle>
        {{
          runtimeIssue
            ? t("admin.streamMappings.runtimeDisabledTitle")
            : t("admin.streamMappings.disabledTitle")
        }}
      </AlertTitle>
      <AlertDescription class="space-y-2 text-sm leading-6 text-amber-900">
        <p v-if="runtimeIssue">{{ issueSummary }}</p>
        <p>
          {{
            runtimeIssue
              ? t("admin.streamMappings.runtimeIssueRecovery")
              : t("admin.streamMappings.disabledDescription")
          }}
        </p>
        <div
          v-if="runtimeIssue"
          class="rounded-md border border-amber-200/80 bg-white/60 px-3 py-2"
        >
          <div class="text-xs font-medium text-amber-800">
            {{ t("admin.streamMappings.runtimeIssueDetails") }}
          </div>
          <code class="mt-1 block break-all text-xs text-amber-950">{{
            runtimeIssue.message
          }}</code>
        </div>
      </AlertDescription>
    </div>
  </Alert>
</template>
