<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, ShieldCheck } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import MappingVisibilityFields from "@/components/MappingVisibilityFields.vue";
import StreamProtocolMultiSelect from "@/components/StreamProtocolMultiSelect.vue";
import { useGatewayVisibilityStatus } from "@/composables/useGatewayVisibilityStatus";
import { getMappingVisibilityValidationIssue } from "@/lib/mapping-visibility";
import { CIDR_OPERATORS } from "@/types/cidr";
import { parseCidrTextarea } from "@admin-shared/utils/cidr";
import { isValidHostPort } from "@admin-shared/utils/parseHostPort";
import type {
  GatewayVisibilitySelection,
  HostVisibilityMode,
  StreamMapping,
  StreamMappingProtocol,
} from "@/types";
import {
  createMappingKey,
  DEFAULT_STREAM_PROTOCOL,
  formatProtocolLabel,
  getMappingKey,
  isObviousLocalStreamTargetLoop,
  normalizeProtocolSelection,
  normalizeStreamMapping,
  type StreamMappingEditorSubmission,
} from "./streamMappingModel";

const props = defineProps<{
  existingMappings: StreamMapping[];
  mapping: StreamMapping | null;
  open: boolean;
  saving: boolean;
}>();

const emit = defineEmits<{
  save: [submission: StreamMappingEditorSubmission];
  "update:open": [open: boolean];
}>();

const { t, locale } = useI18n();
const hasAttemptedSubmit = ref(false);
const hasPortBlurred = ref(false);
const hasTargetBlurred = ref(false);
const visibilityOpen = ref(false);
const visibilityMode = ref<HostVisibilityMode>("inherit");
const visibilitySelections = ref<GatewayVisibilitySelection[]>([]);
const visibilityCidrsText = ref("");
const {
  globalVisibilityEnabled,
  isGlobalVisibilityLoading,
  globalVisibilityLoadError,
  loadGlobalVisibilityStatus,
  resetGlobalVisibilityStatus,
} = useGatewayVisibilityStatus(() =>
  t("admin.subdomainProxy.visibilityLoadFailed"),
);
const form = reactive<{
  protocols: StreamMappingProtocol[];
  listenPort: string;
  comment: string;
  target: string;
  useAuth: boolean;
}>({
  protocols: [DEFAULT_STREAM_PROTOCOL],
  listenPort: "",
  comment: "",
  target: "",
  useAuth: true,
});

const editingKey = computed(() =>
  props.mapping ? getMappingKey(props.mapping) : null,
);
const isEditing = computed(() => editingKey.value !== null);
const parsedListenPort = computed(() => {
  const value = Number.parseInt(form.listenPort.trim(), 10);
  return Number.isFinite(value) ? value : null;
});
const selectedProtocols = computed(() =>
  normalizeProtocolSelection(form.protocols),
);
const duplicateProtocols = computed(() => {
  const port = parsedListenPort.value;
  if (port === null) return [];
  return selectedProtocols.value.filter((protocol) =>
    props.existingMappings.some(
      (mapping) =>
        getMappingKey(mapping) === createMappingKey(protocol, port) &&
        getMappingKey(mapping) !== editingKey.value,
    ),
  );
});

const visibilityModeLabel = computed(() => {
  const labels = {
    inherit: "visibilityInherit",
    custom: "visibilityCustom",
    disabled: "visibilityDisabled",
  } as const;
  return t(`admin.subdomainProxy.${labels[visibilityMode.value]}`);
});
const visibilityValidationMessage = computed(() => {
  const issue = getMappingVisibilityValidationIssue({
    available: globalVisibilityEnabled.value,
    mode: visibilityMode.value,
    selectionsCount: visibilitySelections.value.length,
    customCidrsText: visibilityCidrsText.value,
  });
  if (issue?.kind === "invalid_cidrs") {
    return t("admin.subdomainProxy.visibilityInvalidCidrs", {
      items: issue.invalid.join("、"),
    });
  }
  return issue?.kind === "empty"
    ? t("admin.subdomainProxy.visibilityRuleRequired")
    : "";
});

const formatProtocolList = (protocols: StreamMappingProtocol[]) => {
  const separator = String(locale.value).startsWith("en") ? ", " : "、";
  return protocols.map(formatProtocolLabel).join(separator);
};

const getPortValidationMessage = (showRequired: boolean): string => {
  const rawPort = form.listenPort.trim();
  if (!rawPort) {
    return showRequired ? t("admin.streamMappings.portRequired") : "";
  }

  const port = parsedListenPort.value;
  if (port === null) return t("admin.streamMappings.portInteger");
  if (port <= 0 || port > 65535) {
    return t("admin.streamMappings.portRange");
  }
  if (duplicateProtocols.value.length > 0) {
    return t("admin.streamMappings.duplicatePort", {
      protocols: formatProtocolList(duplicateProtocols.value),
      port,
    });
  }
  return "";
};

const getTargetValidationMessage = (showRequired: boolean): string => {
  const rawTarget = form.target.trim();
  if (!rawTarget) {
    return showRequired ? t("admin.streamMappings.targetRequired") : "";
  }
  if (!isValidHostPort(rawTarget)) {
    return t("admin.streamMappings.targetInvalid");
  }
  const listenPort = parsedListenPort.value;
  if (
    listenPort !== null &&
    isObviousLocalStreamTargetLoop(rawTarget, listenPort)
  ) {
    return t("admin.streamMappings.localTargetLoop");
  }
  return "";
};

const submitValidationMessage = computed(() => {
  if (selectedProtocols.value.length === 0) {
    return t("admin.streamMappings.protocolRequired");
  }
  return (
    getPortValidationMessage(true) || getTargetValidationMessage(true) || ""
  );
});

const validationMessage = computed(() => {
  if (hasAttemptedSubmit.value) return submitValidationMessage.value;
  if (hasPortBlurred.value && form.listenPort.trim()) {
    const message = getPortValidationMessage(false);
    if (message) return message;
  }
  if (hasTargetBlurred.value && form.target.trim()) {
    return getTargetValidationMessage(false);
  }
  return "";
});

const resetForm = () => {
  const mapping = props.mapping ? normalizeStreamMapping(props.mapping) : null;
  form.protocols = mapping ? [mapping.protocol] : [DEFAULT_STREAM_PROTOCOL];
  form.listenPort = mapping ? String(mapping.listen_port) : "";
  form.comment = mapping?.comment ?? "";
  form.target = mapping?.target ?? "";
  form.useAuth = mapping?.use_auth ?? true;
  visibilityOpen.value = false;
  visibilityMode.value = mapping?.visibility?.mode ?? "inherit";
  visibilitySelections.value = (mapping?.visibility?.selections ?? []).map(
    (selection) => ({
      ...selection,
      operator: CIDR_OPERATORS.find(
        (operator) => operator === selection.operator,
      ),
    }),
  );
  visibilityCidrsText.value = (mapping?.visibility?.custom_cidrs ?? []).join(
    "\n",
  );
  hasAttemptedSubmit.value = false;
  hasPortBlurred.value = false;
  hasTargetBlurred.value = false;
};

const submit = () => {
  hasAttemptedSubmit.value = true;
  const port = parsedListenPort.value;
  if (visibilityValidationMessage.value) {
    visibilityOpen.value = true;
    return;
  }
  if (submitValidationMessage.value || port === null) return;
  const custom = visibilityMode.value === "custom";
  const visibility: StreamMapping["visibility"] = globalVisibilityEnabled.value
    ? {
        mode: visibilityMode.value,
        selections: custom
          ? visibilitySelections.value.map((selection) => ({
              ...selection,
              operator: selection.operator ?? null,
            }))
          : [],
        custom_cidrs: custom
          ? parseCidrTextarea(visibilityCidrsText.value).cidrs
          : [],
      }
    : props.mapping?.visibility;

  emit("save", {
    editingKey: editingKey.value,
    mappings: selectedProtocols.value.map((protocol) => ({
      protocol,
      listen_port: port,
      comment: form.comment.trim(),
      target: form.target.trim(),
      use_auth: form.useAuth,
      ...(visibility && { visibility }),
    })),
  });
};

watch(
  () => [props.open, props.mapping] as const,
  ([open]) => {
    if (open) {
      resetForm();
      void loadGlobalVisibilityStatus();
    } else {
      resetGlobalVisibilityStatus();
    }
  },
  { immediate: true },
);
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="flex max-h-[85dvh] flex-col overflow-hidden sm:max-w-[520px]"
    >
      <DialogHeader class="shrink-0">
        <DialogTitle>
          {{
            isEditing
              ? t("admin.streamMappings.editTitle")
              : t("admin.streamMappings.createTitle")
          }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.streamMappings.dialogDescription") }}
        </DialogDescription>
      </DialogHeader>

      <div
        data-dialog-input-scroll
        class="grid min-h-0 gap-4 overflow-y-auto overscroll-contain py-2 pr-1"
      >
        <div class="space-y-2">
          <Label for="stream-protocol">
            {{ t("admin.streamMappings.transportProtocol") }}
          </Label>
          <StreamProtocolMultiSelect
            id="stream-protocol"
            v-model="form.protocols"
          />
          <p class="text-xs text-muted-foreground">
            {{ t("admin.streamMappings.protocolHint") }}
          </p>
        </div>

        <div class="space-y-2">
          <Label for="stream-listen-port">
            {{ t("admin.streamMappings.listenPort") }}
          </Label>
          <Input
            id="stream-listen-port"
            v-model="form.listenPort"
            inputmode="numeric"
            :placeholder="t('admin.streamMappings.listenPortPlaceholder')"
            @blur="hasPortBlurred = true"
          />
          <p class="text-xs text-muted-foreground">
            {{ t("admin.streamMappings.listenPortHint") }}
          </p>
        </div>

        <div class="space-y-2">
          <Label for="stream-target">
            {{ t("admin.streamMappings.target") }}
          </Label>
          <Input
            id="stream-target"
            v-model="form.target"
            :placeholder="t('admin.streamMappings.targetPlaceholder')"
            @blur="hasTargetBlurred = true"
          />
          <p class="text-xs text-muted-foreground">
            {{ t("admin.streamMappings.targetHint") }}
          </p>
        </div>

        <div class="space-y-2">
          <Label for="stream-comment">
            {{ t("admin.streamMappings.comment") }}
          </Label>
          <Input
            id="stream-comment"
            v-model="form.comment"
            :placeholder="t('admin.streamMappings.commentPlaceholder')"
          />
          <p class="text-xs text-muted-foreground">
            {{ t("admin.streamMappings.commentHint") }}
          </p>
        </div>

        <div
          class="flex items-center justify-between rounded-lg border px-4 py-3"
        >
          <div class="space-y-1">
            <Label for="stream-auth">
              {{ t("admin.streamMappings.authRequiredLabel") }}
            </Label>
            <p class="text-xs text-muted-foreground">
              {{
                form.useAuth
                  ? t("admin.streamMappings.authRequiredEnabledHint")
                  : t("admin.streamMappings.authRequiredDisabledHint")
              }}
            </p>
          </div>
          <Switch id="stream-auth" v-model="form.useAuth" />
        </div>

        <Collapsible v-model:open="visibilityOpen" class="space-y-3">
          <CollapsibleTrigger as-child>
            <Button
              type="button"
              variant="outline"
              class="group h-auto w-full justify-between gap-3 px-4 py-3 text-left"
            >
              <span class="flex min-w-0 items-start gap-3">
                <ShieldCheck
                  class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground"
                />
                <span class="min-w-0 space-y-1 whitespace-normal">
                  <span class="block">{{
                    t("admin.subdomainProxy.visibilityTitle")
                  }}</span>
                  <span class="block text-xs font-normal text-muted-foreground">
                    {{ visibilityModeLabel }}
                  </span>
                </span>
              </span>
              <ChevronDown
                class="h-4 w-4 shrink-0 text-muted-foreground group-aria-expanded:rotate-180"
              />
            </Button>
          </CollapsibleTrigger>
          <CollapsibleContent class="space-y-3">
            <p
              v-if="isGlobalVisibilityLoading"
              role="status"
              class="text-sm text-muted-foreground"
            >
              {{ t("admin.gatewayVisibilitySettings.loadingConfig") }}
            </p>
            <div v-else-if="globalVisibilityLoadError" class="space-y-2">
              <p role="alert" class="break-words text-sm text-destructive">
                {{ globalVisibilityLoadError }}
              </p>
              <Button
                type="button"
                variant="outline"
                size="sm"
                @click="loadGlobalVisibilityStatus"
              >
                {{ t("admin.subdomainProxy.retry") }}
              </Button>
            </div>
            <template v-else-if="globalVisibilityEnabled">
              <p
                v-if="selectedProtocols.length > 1"
                class="text-sm text-muted-foreground"
              >
                {{ t("admin.streamMappings.visibilityMultiProtocol") }}
              </p>
              <MappingVisibilityFields
                v-model:mode="visibilityMode"
                v-model:selections="visibilitySelections"
                v-model:custom-cidrs-text="visibilityCidrsText"
                :behavior-description="
                  t('admin.streamMappings.visibilityDescription')
                "
                :regions-description="
                  t('admin.streamMappings.visibilityRegionsDescription')
                "
                :cidrs-description="
                  t('admin.streamMappings.visibilityCidrsDescription')
                "
                :validation-message="visibilityValidationMessage"
              />
            </template>
            <p v-else class="text-sm text-muted-foreground">
              {{ t("admin.streamMappings.visibilityGatewayOff") }}
            </p>
          </CollapsibleContent>
        </Collapsible>

        <div
          v-if="validationMessage"
          class="rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
        >
          {{ validationMessage }}
        </div>
      </div>

      <DialogFooter class="shrink-0">
        <Button variant="outline" @click="emit('update:open', false)">
          {{ t("common.cancel") }}
        </Button>
        <Button :disabled="saving" @click="submit">
          <span
            v-if="saving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          />
          {{ t("admin.streamMappings.saveMapping") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
