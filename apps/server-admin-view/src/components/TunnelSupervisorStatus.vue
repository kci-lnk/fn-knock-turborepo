<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import type { TunnelSupervisorStatus } from "@/lib/api";
import { supervisorTone } from "@/views/tunnel/tunnelSupervisorModel";

const props = withDefaults(
  defineProps<{
    supervisor: TunnelSupervisorStatus;
    compact?: boolean;
  }>(),
  { compact: false },
);

const { t } = useI18n();
const label = computed(() =>
  t(`admin.tunnelSupervisor.states.${props.supervisor.state}`),
);
const colorClass = computed(() => {
  switch (supervisorTone(props.supervisor)) {
    case "success":
      return "text-green-600";
    case "info":
      return "text-blue-600";
    case "warning":
      return "text-amber-600";
    default:
      return "text-muted-foreground";
  }
});
</script>

<template>
  <div class="space-y-1.5">
    <div class="inline-flex items-center gap-1.5 text-sm" :class="colorClass">
      <span
        class="h-2 w-2 rounded-full bg-current"
        :class="{ 'animate-pulse': supervisor.state === 'starting' }"
      />
      <span>{{ label }}</span>
    </div>
    <template v-if="!compact">
      <p
        v-if="supervisor.state === 'backoff' && supervisor.nextRestartAt"
        class="text-xs text-muted-foreground"
      >
        {{
          t("admin.tunnelSupervisor.nextRestart", {
            count: supervisor.consecutiveFailures,
          })
        }}
        <HumanFriendlyTime
          :value="supervisor.nextRestartAt"
          :refresh-interval-ms="1000"
        />
      </p>
      <p
        v-if="supervisor.lastFailure"
        class="break-words text-xs text-muted-foreground"
      >
        {{
          supervisor.lastFailure.diagnosis ||
          supervisor.lastFailure.reason
        }}
      </p>
    </template>
  </div>
</template>
