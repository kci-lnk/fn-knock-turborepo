<script setup lang="ts">
import { ref, watch, type UnwrapNestedRefs } from "vue";
import { useI18n } from "vue-i18n";
import { ImageIcon, RefreshCw, Upload } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  mappingIconNeedsDarkPreviewBackground,
  MAPPING_ICON_FILE_ACCEPT,
} from "./mapping-icon";
import type { useMappingIcon } from "./useMappingIcon";

const props = defineProps<{
  iconEditor: UnwrapNestedRefs<ReturnType<typeof useMappingIcon>>;
  isSavingMappings: boolean;
}>();

const { t } = useI18n();
const fileInput = ref<HTMLInputElement | null>(null);
const previewBroken = ref(false);
const previewNeedsDarkBackground = ref(false);

const chooseFile = () => fileInput.value?.click();
const handleFileChange = async (event: Event) => {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (file) await props.iconEditor.uploadCustomFavicon(file);
};
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
  <div class="grid gap-6 pb-6 pt-6">
    <div class="flex flex-col items-center gap-3 text-center">
      <div
        class="flex h-20 w-20 items-center justify-center overflow-hidden rounded-2xl border shadow-sm transition-colors"
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
        <ImageIcon v-else class="h-8 w-8 text-muted-foreground/60" />
      </div>
      <div class="space-y-1">
        <Badge variant="secondary">
          {{ iconEditor.faviconSummary }}
        </Badge>
        <p class="text-xs leading-5 text-muted-foreground">
          {{ t("admin.subdomainProxy.iconPreviewDescription") }}
        </p>
      </div>
    </div>

    <div class="space-y-3 rounded-lg border px-4 py-4">
      <div class="space-y-1">
        <h3 class="text-sm font-medium">
          {{ t("admin.subdomainProxy.customIcon") }}
        </h3>
        <p class="text-xs leading-5 text-muted-foreground">
          {{ t("admin.subdomainProxy.customIconHelp") }}
        </p>
      </div>

      <div class="flex flex-col gap-2 sm:flex-row">
        <Button
          type="button"
          class="sm:flex-1"
          :disabled="iconEditor.isIconBusy || isSavingMappings"
          @click="chooseFile"
        >
          <RefreshCw
            v-if="iconEditor.isProcessingFavicon"
            class="mr-2 h-4 w-4 animate-spin"
          />
          <Upload v-else class="mr-2 h-4 w-4" />
          {{
            iconEditor.faviconSource === "custom"
              ? t("admin.subdomainProxy.replaceIcon")
              : t("admin.subdomainProxy.uploadIcon")
          }}
        </Button>
        <Button
          v-if="iconEditor.faviconSource === 'custom'"
          type="button"
          variant="outline"
          class="sm:flex-1"
          :disabled="iconEditor.isIconBusy || isSavingMappings"
          @click="iconEditor.restoreAutomaticFavicon"
        >
          <RefreshCw
            class="mr-2 h-4 w-4"
            :class="{ 'animate-spin': iconEditor.isRefreshingFavicon }"
          />
          {{
            t(
              iconEditor.canRefreshMetadata
                ? "admin.subdomainProxy.restoreAutomaticIcon"
                : "admin.subdomainProxy.staticServe.removeCustomIcon",
            )
          }}
        </Button>
        <Button
          v-else-if="iconEditor.canRefreshMetadata"
          type="button"
          variant="outline"
          class="sm:flex-1"
          :disabled="
            iconEditor.isIconBusy ||
            isSavingMappings ||
            !iconEditor.canRefreshMetadata
          "
          @click="iconEditor.refreshAutomaticFavicon"
        >
          <RefreshCw
            class="mr-2 h-4 w-4"
            :class="{ 'animate-spin': iconEditor.isRefreshingFavicon }"
          />
          {{ t("admin.subdomainProxy.recollectIcon") }}
        </Button>
      </div>

      <input
        ref="fileInput"
        class="hidden"
        type="file"
        :accept="MAPPING_ICON_FILE_ACCEPT"
        @change="handleFileChange"
      />

      <p
        v-if="iconEditor.iconErrorMessage"
        class="text-xs leading-5 text-destructive"
        role="alert"
      >
        {{ iconEditor.iconErrorMessage }}
      </p>
    </div>
  </div>
</template>
