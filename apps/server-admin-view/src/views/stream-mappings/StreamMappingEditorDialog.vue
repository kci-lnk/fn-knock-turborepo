<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
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
import StreamProtocolMultiSelect from "@/components/StreamProtocolMultiSelect.vue";
import { isValidHostPort } from "@admin-shared/utils/parseHostPort";
import type { StreamMapping, StreamMappingProtocol } from "@/types";
import {
  createMappingKey,
  DEFAULT_STREAM_PROTOCOL,
  formatProtocolLabel,
  getMappingKey,
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
  return isValidHostPort(rawTarget)
    ? ""
    : t("admin.streamMappings.targetInvalid");
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
  hasAttemptedSubmit.value = false;
  hasPortBlurred.value = false;
  hasTargetBlurred.value = false;
};

const submit = () => {
  hasAttemptedSubmit.value = true;
  const port = parsedListenPort.value;
  if (submitValidationMessage.value || port === null) return;

  emit("save", {
    editingKey: editingKey.value,
    mappings: selectedProtocols.value.map((protocol) => ({
      protocol,
      listen_port: port,
      comment: form.comment.trim(),
      target: form.target.trim(),
      use_auth: form.useAuth,
    })),
  });
};

watch(
  () => [props.open, props.mapping] as const,
  ([open]) => {
    if (open) resetForm();
  },
  { immediate: true },
);
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[520px]">
      <DialogHeader>
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

      <div class="grid gap-4 py-4">
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
              {{ t("admin.streamMappings.authRequiredHint") }}
            </p>
          </div>
          <Switch id="stream-auth" v-model="form.useAuth" />
        </div>

        <div
          v-if="validationMessage"
          class="rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
        >
          {{ validationMessage }}
        </div>
      </div>

      <DialogFooter>
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
