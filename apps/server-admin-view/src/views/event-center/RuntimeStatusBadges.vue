<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import type { RuntimeComponentHealth, RuntimeHealthStatus } from "../../types";
import { runtimeStatusClass } from "./runtimePresentation";

defineProps<{
  status: RuntimeHealthStatus;
  lifecycle?: RuntimeComponentHealth["lifecycle"];
  overall?: boolean;
}>();
const { t } = useI18n();
</script>

<template>
  <div class="flex shrink-0 flex-wrap justify-end gap-1">
    <Badge v-if="lifecycle" variant="outline">
      {{
        t(
          `admin.eventCenter.runtime.lifecycle.${overall ? "stopping" : lifecycle.phase}`,
        )
      }}
    </Badge>
    <Badge
      v-if="!lifecycle || status === 'unhealthy'"
      variant="outline"
      :class="runtimeStatusClass(status)"
    >
      {{ t(`admin.eventCenter.runtime.status.${status}`) }}
    </Badge>
  </div>
</template>
