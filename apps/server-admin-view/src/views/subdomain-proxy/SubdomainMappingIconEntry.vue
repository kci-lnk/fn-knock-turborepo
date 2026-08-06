<script setup lang="ts">
import { ref, watch, type UnwrapNestedRefs } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronRight, ImageIcon } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { mappingIconNeedsDarkPreviewBackground } from "./mapping-icon";
import type { useMappingIcon } from "./useMappingIcon";

const props = defineProps<{
  iconEditor: UnwrapNestedRefs<ReturnType<typeof useMappingIcon>>;
  openEditor: () => void;
}>();

const { t } = useI18n();
const previewBroken = ref(false);
const previewNeedsDarkBackground = ref(false);
const handlePreviewLoad = (event: Event) => {
  previewNeedsDarkBackground.value = mappingIconNeedsDarkPreviewBackground(
    event.currentTarget as HTMLImageElement,
  );
};
const handlePreviewError = () => {
  previewBroken.value = true;
  previewNeedsDarkBackground.value = false;
};
watch(
  () => props.iconEditor.effectiveFaviconSrc,
  () => {
    previewBroken.value = false;
    previewNeedsDarkBackground.value = false;
  },
);
</script>

<template>
  <Button
    type="button"
    variant="outline"
    class="h-auto w-full justify-between gap-3 px-4 py-3 text-left"
    @click="openEditor"
  >
    <span class="flex min-w-0 flex-1 items-center gap-3">
      <span
        class="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-lg border transition-colors"
        :class="previewNeedsDarkBackground ? 'bg-slate-700' : 'bg-muted/40'"
      >
        <img
          v-if="iconEditor.effectiveFaviconSrc && !previewBroken"
          :src="iconEditor.effectiveFaviconSrc"
          :alt="t('admin.subdomainProxy.iconPreviewAlt')"
          class="h-full w-full object-contain"
          @load="handlePreviewLoad"
          @error="handlePreviewError"
        />
        <ImageIcon v-else class="h-4 w-4 text-muted-foreground" />
      </span>
      <span class="min-w-0 flex-1 space-y-1">
        <span class="block text-sm font-medium">
          {{ t("admin.subdomainProxy.iconTitle") }}
        </span>
        <span
          class="block whitespace-normal break-words text-xs font-normal leading-5 text-muted-foreground"
        >
          {{ iconEditor.faviconSummary }}
        </span>
      </span>
    </span>
    <ChevronRight class="h-4 w-4 shrink-0 text-muted-foreground" />
  </Button>
</template>
