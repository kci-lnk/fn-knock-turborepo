<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  FolderOpen,
  Loader2,
  Search,
  TriangleAlert,
} from "lucide-vue-next";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  TagsInput,
  TagsInputInput,
  TagsInputItem,
  TagsInputItemDelete,
  TagsInputItemText,
} from "@/components/ui/tags-input";
import { ConfigAPI, type StaticPathProbeResult } from "@/lib/api/config";
import { useConfigStore } from "@/store/config";
import type { HostMappingStaticServe, HostMappingTargetType } from "@/types";
import {
  getStaticServeValidationIssue,
  type StaticServeValidationIssue,
} from "./host-mapping-target-model";

const props = defineProps<{
  modelValue: HostMappingStaticServe;
  open: boolean;
  targetType: Exclude<HostMappingTargetType, "proxy">;
}>();
const emit = defineEmits<{
  browse: [targetType: Exclude<HostMappingTargetType, "proxy">, path: string];
  "update:modelValue": [value: HostMappingStaticServe];
}>();

const { t } = useI18n();
const configStore = useConfigStore();
const isProbing = ref(false);
const probeResult = ref<StaticPathProbeResult | null>(null);
const probeError = ref("");
let probeRequestId = 0;

const patchStaticServe = (patch: Partial<HostMappingStaticServe>) => {
  emit("update:modelValue", { ...props.modelValue, ...patch });
};
const pathModel = computed({
  get: () => props.modelValue.path,
  set: (path: string) => patchStaticServe({ path }),
});
const indexFilesModel = computed({
  get: () => props.modelValue.index_files,
  set: (indexFiles: string[]) => patchStaticServe({ index_files: indexFiles }),
});
const listingEnabledModel = computed({
  get: () => props.modelValue.directory_listing.enabled,
  set: (enabled: boolean) =>
    patchStaticServe({
      directory_listing: {
        enabled,
        render_readme:
          enabled && props.modelValue.directory_listing.render_readme,
      },
    }),
});
const renderReadmeModel = computed({
  get: () => props.modelValue.directory_listing.render_readme,
  set: (renderReadme: boolean) =>
    patchStaticServe({
      directory_listing: {
        enabled: renderReadme || props.modelValue.directory_listing.enabled,
        render_readme: renderReadme,
      },
    }),
});
const pathPlaceholder = computed(() => {
  if (configStore.isWindowsDeployment) {
    return props.targetType === "directory"
      ? "C:\\Sites\\docs"
      : "C:\\Sites\\docs\\manual.pdf";
  }
  return props.targetType === "directory"
    ? "/srv/site"
    : "/srv/site/manual.pdf";
});
const validationIssue = computed<StaticServeValidationIssue | null>(() =>
  getStaticServeValidationIssue({
    isWindows: configStore.isWindowsDeployment,
    staticServe: props.modelValue,
    targetType: props.targetType,
  }),
);
const pathValidationIssue = computed(() => {
  const issue = validationIssue.value;
  return issue === "path_required" ||
    issue === "path_not_absolute" ||
    issue === "path_has_parent_segment" ||
    issue === "path_unsafe"
    ? issue
    : null;
});
const pathValidationMessage = computed(() =>
  pathValidationIssue.value
    ? t(
        `admin.subdomainProxy.staticServe.validation.${pathValidationIssue.value}`,
      )
    : "",
);
const indexFilesValidationMessage = computed(() =>
  validationIssue.value && !pathValidationIssue.value
    ? t(`admin.subdomainProxy.staticServe.validation.${validationIssue.value}`)
    : "",
);
const canProbe = computed(
  () =>
    !isProbing.value && !pathValidationIssue.value && !!pathModel.value.trim(),
);
const probeSucceeded = computed(
  () =>
    probeResult.value !== null &&
    !probeResult.value.error_code &&
    probeResult.value.exists &&
    probeResult.value.readable &&
    probeResult.value.target_type === props.targetType &&
    probeResult.value.actual_type === props.targetType,
);
const probeMessage = computed(() => {
  if (probeError.value) return probeError.value;
  if (!probeResult.value) return "";
  if (probeSucceeded.value) {
    return t("admin.subdomainProxy.staticServe.probeSuccess", {
      path: probeResult.value.normalized_path || pathModel.value,
    });
  }
  const code =
    probeResult.value.error_code ||
    (probeResult.value.actual_type !== props.targetType
      ? "type_mismatch"
      : "probe_failed");
  return t(`admin.subdomainProxy.staticServe.probeErrors.${code}`);
});

const invalidateProbe = () => {
  probeRequestId += 1;
  isProbing.value = false;
  probeResult.value = null;
  probeError.value = "";
};

const moveIndexFile = (index: number, offset: -1 | 1) => {
  const nextIndex = index + offset;
  if (nextIndex < 0 || nextIndex >= indexFilesModel.value.length) return;
  const next = [...indexFilesModel.value];
  const filename = next[index];
  if (filename === undefined) return;
  next.splice(index, 1);
  next.splice(nextIndex, 0, filename);
  indexFilesModel.value = next;
};

const probePath = async () => {
  if (!canProbe.value) return;
  const requestId = ++probeRequestId;
  const path = pathModel.value;
  isProbing.value = true;
  probeResult.value = null;
  probeError.value = "";
  try {
    const result = await ConfigAPI.probeHostMappingStaticPath(
      props.targetType,
      path,
    );
    if (requestId !== probeRequestId) return;
    probeResult.value = result;
  } catch (error) {
    if (requestId !== probeRequestId) return;
    probeError.value = extractErrorMessage(
      error,
      t("admin.subdomainProxy.staticServe.probeErrors.probe_failed"),
    );
  } finally {
    if (requestId === probeRequestId) isProbing.value = false;
  }
};

watch(
  () => [props.open, props.targetType, props.modelValue.path] as const,
  invalidateProbe,
);
</script>

<template>
  <div class="space-y-4 rounded-lg border bg-muted/15 px-4 py-4">
    <div class="space-y-2">
      <Label for="mapping-static-path">
        {{ t("admin.subdomainProxy.staticServe.pathLabel") }}
      </Label>
      <div class="flex flex-wrap gap-2 sm:flex-nowrap">
        <Input
          id="mapping-static-path"
          v-model="pathModel"
          class="font-mono"
          :placeholder="pathPlaceholder"
        />
        <Button
          type="button"
          variant="outline"
          class="shrink-0"
          data-testid="browse-static-path"
          @click="emit('browse', targetType, pathModel)"
        >
          <FolderOpen class="mr-2 h-4 w-4" />
          {{ t("admin.subdomainProxy.staticServe.browser.open") }}
        </Button>
        <Button
          type="button"
          variant="outline"
          class="shrink-0"
          :disabled="!canProbe"
          @click="probePath"
        >
          <Loader2 v-if="isProbing" class="mr-2 h-4 w-4 animate-spin" />
          <Search v-else class="mr-2 h-4 w-4" />
          {{ t("admin.subdomainProxy.staticServe.probe") }}
        </Button>
      </div>
      <p v-if="pathValidationMessage" class="text-xs text-destructive">
        {{ pathValidationMessage }}
      </p>
      <p
        v-else-if="probeMessage"
        class="flex items-start gap-1.5 text-xs"
        :class="probeSucceeded ? 'text-emerald-600' : 'text-destructive'"
      >
        <CheckCircle2 v-if="probeSucceeded" class="mt-0.5 h-3.5 w-3.5" />
        <TriangleAlert v-else class="mt-0.5 h-3.5 w-3.5" />
        <span>{{ probeMessage }}</span>
      </p>
      <p class="text-xs leading-5 text-muted-foreground">
        {{ t("admin.subdomainProxy.staticServe.pathHint") }}
      </p>
      <p
        v-if="configStore.isDockerDeployment"
        class="text-xs leading-5 text-muted-foreground"
      >
        {{ t("admin.subdomainProxy.staticServe.pathDockerHint") }}
      </p>
    </div>

    <template v-if="targetType === 'directory'">
      <div class="space-y-2">
        <Label for="mapping-index-files">
          {{ t("admin.subdomainProxy.staticServe.indexFiles") }}
        </Label>
        <TagsInput
          v-model="indexFilesModel"
          add-on-blur
          class="min-h-10"
          :aria-invalid="!!indexFilesValidationMessage"
        >
          <TagsInputItem
            v-for="(filename, index) in indexFilesModel"
            :key="`${filename}-${index}`"
            :value="filename"
          >
            <TagsInputItemText />
            <button
              type="button"
              class="rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30"
              :disabled="index === 0"
              :aria-label="t('admin.subdomainProxy.staticServe.moveIndexUp')"
              @click.prevent="moveIndexFile(index, -1)"
            >
              <ChevronUp class="h-3 w-3" />
            </button>
            <button
              type="button"
              class="rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-30"
              :disabled="index === indexFilesModel.length - 1"
              :aria-label="t('admin.subdomainProxy.staticServe.moveIndexDown')"
              @click.prevent="moveIndexFile(index, 1)"
            >
              <ChevronDown class="h-3 w-3" />
            </button>
            <TagsInputItemDelete />
          </TagsInputItem>
          <TagsInputInput
            id="mapping-index-files"
            :placeholder="
              indexFilesModel.length === 0
                ? t('admin.subdomainProxy.staticServe.indexFilesPlaceholder')
                : ''
            "
          />
        </TagsInput>
        <p class="text-xs leading-5 text-muted-foreground">
          {{ t("admin.subdomainProxy.staticServe.indexFilesHint") }}
        </p>
        <p v-if="indexFilesValidationMessage" class="text-xs text-destructive">
          {{ indexFilesValidationMessage }}
        </p>
      </div>

      <div
        class="flex items-center justify-between gap-4 rounded-lg border bg-background px-3 py-3"
      >
        <div class="min-w-0 space-y-1">
          <Label for="mapping-directory-listing">
            {{ t("admin.subdomainProxy.staticServe.directoryListing") }}
          </Label>
          <p class="text-xs leading-5 text-muted-foreground">
            {{ t("admin.subdomainProxy.staticServe.directoryListingHint") }}
          </p>
        </div>
        <Switch id="mapping-directory-listing" v-model="listingEnabledModel" />
      </div>

      <div
        class="flex items-center justify-between gap-4 rounded-lg border bg-background px-3 py-3"
      >
        <div class="min-w-0 space-y-1">
          <Label for="mapping-render-readme">
            {{ t("admin.subdomainProxy.staticServe.renderReadme") }}
          </Label>
          <p class="text-xs leading-5 text-muted-foreground">
            {{ t("admin.subdomainProxy.staticServe.renderReadmeHint") }}
          </p>
        </div>
        <Switch
          id="mapping-render-readme"
          v-model="renderReadmeModel"
          :disabled="!listingEnabledModel"
        />
      </div>
    </template>
  </div>
</template>
