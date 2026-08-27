<script setup lang="ts">
import { Copy, ExternalLink } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import { toast } from "@admin-shared/utils/toast";

const props = withDefaults(
  defineProps<{
    traceId?: string | null;
    showLabel?: boolean;
  }>(),
  { traceId: "", showLabel: true },
);

const { t } = useI18n();

const copy = async () => {
  if (!props.traceId) return;
  await copyTextToClipboard(props.traceId);
  toast.success(t("admin.trace.copied"));
};
</script>

<template>
  <div v-if="traceId" class="flex min-w-0 items-center gap-1.5">
    <span v-if="showLabel" class="shrink-0 text-sm text-muted-foreground">{{
      t("admin.trace.label")
    }}</span>
    <RouterLink
      :to="`/traces/${encodeURIComponent(traceId)}`"
      class="min-w-0 break-all font-mono text-sm text-primary underline-offset-4 hover:underline focus-visible:rounded focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      {{ traceId }}
      <ExternalLink class="ml-1 inline h-3.5 w-3.5" aria-hidden="true" />
    </RouterLink>
    <Button
      type="button"
      variant="ghost"
      size="icon"
      class="h-7 w-7 shrink-0"
      :aria-label="t('admin.trace.copy')"
      @click="copy"
    >
      <Copy class="h-3.5 w-3.5" />
    </Button>
  </div>
  <p v-else class="text-sm text-muted-foreground">
    {{ t("admin.trace.legacyRecord") }}
  </p>
</template>
