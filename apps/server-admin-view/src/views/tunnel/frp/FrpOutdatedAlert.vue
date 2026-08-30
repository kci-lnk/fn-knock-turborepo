<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { TriangleAlert } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

const props = defineProps<{
  installationStatus: "missing" | "outdated" | "current";
  targetVersion: string;
  runningCount: number;
  outdatedRunningCount: number;
}>();

const emit = defineEmits<{ goUpdate: [] }>();
const { t } = useI18n();

const visible = computed(
  () =>
    props.installationStatus === "outdated" || props.outdatedRunningCount > 0,
);
const restartRequired = computed(
  () =>
    props.installationStatus === "current" && props.outdatedRunningCount > 0,
);
const titleKey = computed(() =>
  restartRequired.value
    ? "admin.frpTunnel.restartRequiredTitle"
    : props.runningCount > 0
      ? "admin.frpTunnel.outdatedRunningTitle"
      : "admin.frpTunnel.outdatedStoppedTitle",
);
const descriptionKey = computed(() =>
  restartRequired.value
    ? "admin.frpTunnel.restartRequiredDescription"
    : props.runningCount > 0
      ? "admin.frpTunnel.outdatedRunningDescription"
      : "admin.frpTunnel.outdatedStoppedDescription",
);
</script>

<template>
  <Alert
    v-if="visible"
    class="items-start rounded-xl border-amber-300 bg-amber-50 text-amber-950 dark:border-amber-800 dark:bg-amber-950/30 dark:text-amber-100"
    data-testid="frp-outdated-warning"
  >
    <TriangleAlert class="size-4 text-amber-700" />
    <AlertTitle>{{ t(titleKey) }}</AlertTitle>
    <AlertDescription class="space-y-3 text-amber-900 dark:text-amber-100">
      <p>
        {{
          t(descriptionKey, {
            count: props.outdatedRunningCount,
            version: props.targetVersion,
          })
        }}
      </p>
      <Button
        v-if="props.installationStatus !== 'current'"
        variant="outline"
        size="sm"
        @click="emit('goUpdate')"
      >
        {{ t("admin.frpTunnel.goUpdate") }}
      </Button>
    </AlertDescription>
  </Alert>
</template>
