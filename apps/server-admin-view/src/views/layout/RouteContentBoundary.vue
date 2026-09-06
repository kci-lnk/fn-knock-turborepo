<script setup lang="ts">
import { onErrorCaptured, ref, watch } from "vue";
import { RouterView } from "vue-router";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import {
  isDynamicImportFailure,
  replaceWithUpdatedApplication,
} from "@/lib/update-reload";

const props = defineProps<{ resetKey: string }>();
const { t } = useI18n();
const failed = ref(false);

watch(
  () => props.resetKey,
  () => {
    failed.value = false;
  },
);

onErrorCaptured((error) => {
  // Generic fetch errors can originate from business/API code in this subtree.
  if (!isDynamicImportFailure(error, false)) return;
  failed.value = true;
  console.error("Page resource loading failed", error);
  return false;
});
</script>

<template>
  <section
    v-if="failed"
    role="alert"
    class="flex min-h-64 flex-col items-center justify-center gap-3 rounded-xl border border-border bg-background p-6 text-center"
  >
    <h2 class="text-lg font-semibold">
      {{ t("admin.route.resourceLoadFailed") }}
    </h2>
    <p class="max-w-lg text-sm text-muted-foreground">
      {{ t("admin.route.resourceLoadFailedDescription") }}
    </p>
    <Button @click="replaceWithUpdatedApplication('chunk')">
      {{ t("admin.route.reloadPage") }}
    </Button>
  </section>
  <slot v-else><RouterView /></slot>
</template>
