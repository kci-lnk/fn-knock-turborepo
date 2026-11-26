<script setup lang="ts">
import { useI18n } from "vue-i18n";

const props = defineProps<{
  installedVersion?: string | null;
  availableVersion?: string | null;
  availableVersionLabel?: string;
  missing: boolean;
}>();

const { t } = useI18n();
</script>

<template>
  <dl
    class="grid gap-3 rounded-md bg-muted/50 p-3 sm:grid-cols-2"
    aria-live="polite"
  >
    <div class="min-w-0">
      <dt class="text-sm text-muted-foreground">
        {{ t("shared.binaryDownload.installedVersion") }}
      </dt>
      <dd
        v-if="props.installedVersion"
        class="mt-1 wrap-anywhere font-mono font-medium tabular-nums"
      >
        {{ props.installedVersion }}
      </dd>
      <dd v-else class="mt-1 text-sm font-medium">
        {{
          props.missing
            ? t("shared.binaryDownload.versionNotInstalled")
            : t("shared.binaryDownload.versionUnknown")
        }}
      </dd>
    </div>
    <div class="min-w-0">
      <dt class="text-sm text-muted-foreground">
        {{
          props.availableVersionLabel ??
          t("shared.binaryDownload.availableVersion")
        }}
      </dt>
      <dd
        v-if="props.availableVersion"
        class="mt-1 wrap-anywhere font-mono font-medium tabular-nums"
      >
        {{ props.availableVersion }}
      </dd>
      <dd v-else class="mt-1 text-sm font-medium">
        {{ t("shared.binaryDownload.versionUnknown") }}
      </dd>
    </div>
  </dl>
</template>
