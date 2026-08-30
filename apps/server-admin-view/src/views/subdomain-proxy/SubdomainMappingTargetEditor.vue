<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { AcceptableValue } from "reka-ui";
import ConfirmationDialog from "@admin-shared/components/common/ConfirmationDialog.vue";
import { useConfirmationDialog } from "@admin-shared/composables/useConfirmationDialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Label } from "@/components/ui/label";
import type {
  HostMapping,
  HostMappingStaticServe,
  HostMappingTargetType,
} from "@/types";
import {
  createDefaultMapping,
  createDisabledMappingBasicAuth,
  createDefaultStaticServe,
  normalizeHostMappingTargetType,
} from "./model";
import SubdomainMappingStaticTargetField from "./SubdomainMappingStaticTargetField.vue";
import SubdomainMappingTargetField from "./SubdomainMappingTargetField.vue";

const props = defineProps<{
  allowTargetPathMode: boolean;
  mappingForm: HostMapping;
  open: boolean;
  updateMappingForm: (patch: Partial<HostMapping>) => void;
}>();

const { t } = useI18n();
const confirmation = useConfirmationDialog();
type ProxyDraft = Pick<
  HostMapping,
  | "target"
  | "target_path_mode"
  | "basic_auth"
  | "locations"
  | "preserve_host"
  | "suppress_toolbar"
>;
const proxyDraft = ref<ProxyDraft | null>(null);
const staticDrafts = ref<
  Partial<Record<"file" | "directory", HostMappingStaticServe>>
>({});

const cloneStaticServe = (
  value: HostMappingStaticServe,
): HostMappingStaticServe => ({
  path: value.path,
  index_files: [...value.index_files],
  directory_listing: { ...value.directory_listing },
});
const captureProxyDraft = (): ProxyDraft => ({
  target: props.mappingForm.target,
  target_path_mode: props.mappingForm.target_path_mode,
  basic_auth: { ...props.mappingForm.basic_auth },
  locations: props.mappingForm.locations.map((location) => ({
    ...location,
    response: {
      ...location.response,
      headers: { ...location.response.headers },
    },
  })),
  preserve_host: props.mappingForm.preserve_host,
  suppress_toolbar: props.mappingForm.suppress_toolbar,
});
const hasProxySettingsToClear = () =>
  !!props.mappingForm.target.trim() ||
  props.mappingForm.locations.length > 0 ||
  props.mappingForm.basic_auth.enabled ||
  props.mappingForm.preserve_host ||
  props.mappingForm.target_path_mode === "prefix";

const applyTargetType = (targetType: HostMappingTargetType) => {
  const currentType = normalizeHostMappingTargetType(
    props.mappingForm.target_type,
  );
  if (currentType === targetType) return;

  if (currentType === "proxy") {
    proxyDraft.value = captureProxyDraft();
  } else if (props.mappingForm.static_serve) {
    staticDrafts.value[currentType] = cloneStaticServe(
      props.mappingForm.static_serve,
    );
  }

  if (targetType === "proxy") {
    const fallback = createDefaultMapping();
    const draft = proxyDraft.value;
    props.updateMappingForm({
      target_type: "proxy",
      target: draft?.target ?? "",
      static_serve: null,
      target_path_mode: draft?.target_path_mode ?? fallback.target_path_mode,
      basic_auth: draft?.basic_auth ?? fallback.basic_auth,
      locations: draft?.locations ?? [],
      preserve_host: draft?.preserve_host ?? false,
      suppress_toolbar: draft?.suppress_toolbar ?? false,
    });
    return;
  }

  const staticServe =
    staticDrafts.value[targetType] ?? createDefaultStaticServe(targetType);
  props.updateMappingForm({
    target_type: targetType,
    target: "",
    static_serve: cloneStaticServe(staticServe),
    target_path_mode: "entry",
    basic_auth: createDisabledMappingBasicAuth(),
    locations: [],
    preserve_host: false,
    suppress_toolbar: true,
  });
};

const handleTargetTypeChange = async (value: AcceptableValue) => {
  const targetType = normalizeHostMappingTargetType(value);
  const currentType = normalizeHostMappingTargetType(
    props.mappingForm.target_type,
  );
  if (targetType === currentType) return;
  if (
    currentType === "proxy" &&
    targetType !== "proxy" &&
    hasProxySettingsToClear()
  ) {
    const confirmed = await confirmation.requestConfirmation({
      title: t("admin.subdomainProxy.staticServe.switchConfirmTitle"),
      description: t(
        "admin.subdomainProxy.staticServe.switchConfirmDescription",
      ),
      confirmText: t("admin.subdomainProxy.staticServe.switchConfirmAction"),
    });
    if (!confirmed) return;
  }
  applyTargetType(targetType);
};

const updateStaticServe = (value: HostMappingStaticServe) => {
  const targetType = normalizeHostMappingTargetType(
    props.mappingForm.target_type,
  );
  if (targetType === "proxy") return;
  props.updateMappingForm({
    static_serve: cloneStaticServe(value),
  });
};

watch(
  () => props.open,
  (open) => {
    if (!open) {
      proxyDraft.value = null;
      staticDrafts.value = {};
      return;
    }
    const targetType = normalizeHostMappingTargetType(
      props.mappingForm.target_type,
    );
    if (targetType === "proxy") {
      proxyDraft.value = captureProxyDraft();
    } else if (props.mappingForm.static_serve) {
      staticDrafts.value[targetType] = cloneStaticServe(
        props.mappingForm.static_serve,
      );
    }
  },
  { immediate: true },
);
</script>

<template>
  <div class="space-y-3">
    <div class="space-y-2">
      <Label for="mapping-target-type">
        {{ t("admin.subdomainProxy.staticServe.targetType") }}
      </Label>
      <Select
        :model-value="mappingForm.target_type"
        @update:model-value="handleTargetTypeChange"
      >
        <SelectTrigger id="mapping-target-type" class="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="proxy">
            {{ t("admin.subdomainProxy.staticServe.targetTypes.proxy") }}
          </SelectItem>
          <SelectItem value="file">
            {{ t("admin.subdomainProxy.staticServe.targetTypes.file") }}
          </SelectItem>
          <SelectItem value="directory">
            {{ t("admin.subdomainProxy.staticServe.targetTypes.directory") }}
          </SelectItem>
        </SelectContent>
      </Select>
      <p class="text-xs text-muted-foreground">
        {{
          t(
            `admin.subdomainProxy.staticServe.targetTypeHints.${mappingForm.target_type}`,
          )
        }}
      </p>
    </div>

    <SubdomainMappingTargetField
      v-if="mappingForm.target_type === 'proxy'"
      v-model="mappingForm.target"
      v-model:target-path-mode="mappingForm.target_path_mode"
      :allow-target-path-mode="allowTargetPathMode"
      :open="open"
    />
    <SubdomainMappingStaticTargetField
      v-else-if="mappingForm.static_serve"
      :model-value="mappingForm.static_serve"
      :target-type="mappingForm.target_type"
      :open="open"
      @update:model-value="updateStaticServe"
    />
  </div>

  <ConfirmationDialog
    :open="confirmation.confirmationDialogOpen.value"
    v-bind="confirmation.confirmationDialogOptions.value"
    @confirm="confirmation.confirmPendingAction"
    @update:open="confirmation.handleConfirmationDialogOpenChange"
  />
</template>
