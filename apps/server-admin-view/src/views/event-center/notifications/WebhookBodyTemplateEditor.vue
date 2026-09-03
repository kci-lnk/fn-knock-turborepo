<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Eye, FlaskConical, Plus } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import CodeMirrorEditor from "@/components/CodeMirrorEditor.vue";
import type {
  WebhookBodyConfig,
  WebhookBodyConstraints,
  WebhookBodyFormat,
  WebhookBodyMode,
  WebhookBodyPreview,
  WebhookBodyScope,
} from "./webhook-body";
import {
  coerceWebhookBodyConfig,
  createWebhookSampleContext,
  DEFAULT_WEBHOOK_BODY_CONSTRAINTS,
  validateWebhookBodyConfig,
  validateWebhookSampleContext,
  WEBHOOK_BODY_VARIABLES,
} from "./webhook-body";

const props = withDefaults(
  defineProps<{
    modelValue: unknown;
    constraints?: WebhookBodyConstraints;
    sampleContext?: string;
    preview?: WebhookBodyPreview | null;
    previewing?: boolean;
    testing?: boolean;
  }>(),
  {
    constraints: () => DEFAULT_WEBHOOK_BODY_CONSTRAINTS,
    sampleContext: "",
    preview: null,
    previewing: false,
    testing: false,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: WebhookBodyConfig];
  "update:sampleContext": [value: string];
  preview: [];
  test: [];
}>();

const { t } = useI18n();
const templateEditor = ref<InstanceType<typeof CodeMirrorEditor> | null>(null);
const fallbackSampleContext = createWebhookSampleContext();
const encoder = new TextEncoder();

const scope = computed<WebhookBodyScope>(() =>
  props.constraints.scope === "target" ? "target" : "provider",
);
const config = computed(() =>
  coerceWebhookBodyConfig(props.modelValue, scope.value),
);
const custom = computed(() => config.value.mode === "custom");
const issues = computed(() =>
  validateWebhookBodyConfig(props.modelValue, props.constraints, scope.value),
);
const sampleText = computed(() => props.sampleContext || fallbackSampleContext);
const sampleIssue = computed(() => {
  const issue = validateWebhookSampleContext(
    sampleText.value,
    props.constraints,
  )[0];
  if (!issue) return "";
  return t(`admin.notifications.body.errors.${issue.code}`, {
    max: props.constraints.max_sample_bytes || 64 * 1024,
  });
});
const invalid = computed(
  () => issues.value.length > 0 || Boolean(sampleIssue.value),
);
const templateBytes = computed(
  () => encoder.encode(config.value.template || "").length,
);

const defaultCustomTemplate = `{
  "source": "fn_knock",
  "provider_type": "webhook",
  "message": "{{message}}",
  "context": "{{context}}",
  "payload": {
    "event": "{{event}}",
    "extra_body": "{{legacy.extra_body}}"
  }
}`;

const updateConfig = (patch: Partial<WebhookBodyConfig>) => {
  emit("update:modelValue", { ...config.value, ...patch });
};

const updateMode = (value: unknown) => {
  const mode = String(value) as WebhookBodyMode;
  if (mode !== "custom") {
    emit("update:modelValue", { mode });
    return;
  }
  emit("update:modelValue", {
    mode: "custom",
    format: config.value.format || "json",
    content_type: config.value.content_type || "application/json",
    template: config.value.template || defaultCustomTemplate,
  });
};

const updateFormat = (value: unknown) => {
  const format = String(value) as WebhookBodyFormat;
  const previousDefault =
    config.value.format === "text"
      ? "text/plain; charset=utf-8"
      : "application/json";
  updateConfig({
    format,
    content_type:
      !config.value.content_type ||
      config.value.content_type === previousDefault
        ? format === "json"
          ? "application/json"
          : "text/plain; charset=utf-8"
        : config.value.content_type,
  });
};

const formatTemplate = () => {
  if (config.value.format !== "json") return;
  try {
    const parsed = JSON.parse(config.value.template || "");
    updateConfig({ template: JSON.stringify(parsed, null, 2) });
  } catch {
    // The inline validation already explains the syntax problem.
  }
};

const insertVariable = (path: string) => {
  templateEditor.value?.insertText(`{{${path}}}`);
};

const issueText = computed(() => {
  const issue = issues.value[0];
  if (!issue) return "";
  return t(`admin.notifications.body.errors.${issue.code}`, {
    detail: issue.detail || "",
    max:
      issue.code === "templateTooLarge"
        ? props.constraints.max_template_bytes || 64 * 1024
        : issue.code === "tooManyVariables"
          ? props.constraints.max_placeholders || 256
          : props.constraints.max_content_type_bytes || 256,
  });
});
</script>

<template>
  <div class="min-w-0 max-w-full space-y-4">
    <div class="grid gap-3 sm:grid-cols-2">
      <div class="space-y-2">
        <div class="text-xs font-medium text-muted-foreground">
          {{ t("admin.notifications.body.mode") }}
        </div>
        <Select :model-value="config.mode" @update:model-value="updateMode">
          <SelectTrigger :aria-label="t('admin.notifications.body.mode')">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem :value="scope === 'provider' ? 'standard' : 'inherit'">
              {{
                scope === "provider"
                  ? t("admin.notifications.body.standard")
                  : t("admin.notifications.body.inherit")
              }}
            </SelectItem>
            <SelectItem value="custom">
              {{ t("admin.notifications.body.custom") }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div v-if="custom" class="space-y-2">
        <div class="text-xs font-medium text-muted-foreground">
          {{ t("admin.notifications.body.format") }}
        </div>
        <Select :model-value="config.format" @update:model-value="updateFormat">
          <SelectTrigger :aria-label="t('admin.notifications.body.format')">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="json">JSON</SelectItem>
            <SelectItem value="text">Text</SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>

    <template v-if="custom">
      <div class="space-y-2">
        <div class="text-xs font-medium text-muted-foreground">
          Content-Type
        </div>
        <Input
          aria-label="Content-Type"
          :model-value="config.content_type || ''"
          placeholder="application/json"
          @update:model-value="
            (value) => updateConfig({ content_type: String(value) })
          "
        />
      </div>

      <div class="space-y-2">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <div class="text-xs font-medium text-muted-foreground">
            {{ t("admin.notifications.body.template") }}
          </div>
          <div class="flex items-center gap-2">
            <Badge variant="secondary" class="font-mono text-[11px]">
              {{ templateBytes }} B
            </Badge>
            <Button
              v-if="config.format === 'json'"
              type="button"
              variant="outline"
              size="sm"
              @click="formatTemplate"
            >
              {{ t("admin.notifications.body.formatJson") }}
            </Button>
          </div>
        </div>
        <CodeMirrorEditor
          ref="templateEditor"
          class="min-w-0 max-w-full"
          :model-value="config.template || ''"
          :language="config.format === 'json' ? 'json' : 'text'"
          min-height="240px"
          :aria-label="t('admin.notifications.body.template')"
          @update:model-value="(value) => updateConfig({ template: value })"
        />
        <p v-if="issueText" class="text-xs text-destructive" role="alert">
          {{ issueText }}
        </p>
      </div>

      <details class="rounded-md border border-border/70 p-3">
        <summary class="cursor-pointer text-sm font-medium">
          {{ t("admin.notifications.body.variables") }}
        </summary>
        <p class="mt-2 text-xs text-muted-foreground">
          {{ t("admin.notifications.body.variablesHelp") }}
        </p>
        <div class="mt-3 flex max-h-44 flex-wrap gap-2 overflow-y-auto">
          <Button
            v-for="path in WEBHOOK_BODY_VARIABLES"
            :key="path"
            type="button"
            variant="outline"
            size="sm"
            class="h-7 font-mono text-[11px]"
            @click="insertVariable(path)"
          >
            <Plus class="mr-1 h-3 w-3" />{{ path }}
          </Button>
        </div>
      </details>
    </template>

    <details class="rounded-md border border-border/70 p-3">
      <summary class="cursor-pointer text-sm font-medium">
        {{ t("admin.notifications.body.sampleContext") }}
      </summary>
      <p class="mt-2 text-xs text-muted-foreground">
        {{ t("admin.notifications.body.sampleHelp") }}
      </p>
      <div class="mt-3">
        <CodeMirrorEditor
          class="min-w-0 max-w-full"
          :model-value="sampleText"
          language="json"
          min-height="220px"
          :aria-label="t('admin.notifications.body.sampleContext')"
          @update:model-value="(value) => emit('update:sampleContext', value)"
        />
      </div>
      <p v-if="sampleIssue" class="mt-2 text-xs text-destructive" role="alert">
        {{ sampleIssue }}
      </p>
    </details>

    <div class="flex flex-wrap gap-2">
      <Button
        type="button"
        variant="outline"
        size="sm"
        :disabled="invalid || previewing || testing"
        @click="emit('preview')"
      >
        <Eye class="mr-2 h-4 w-4" />
        {{
          previewing
            ? t("admin.notifications.body.previewing")
            : t("admin.notifications.body.preview")
        }}
      </Button>
      <Button
        v-if="scope === 'target'"
        type="button"
        variant="secondary"
        size="sm"
        :disabled="invalid || previewing || testing"
        @click="emit('test')"
      >
        <FlaskConical class="mr-2 h-4 w-4" />
        {{
          testing
            ? t("admin.notifications.body.testing")
            : t("admin.notifications.body.testSend")
        }}
      </Button>
    </div>

    <div
      v-if="preview"
      class="min-w-0 max-w-full overflow-hidden rounded-md border border-border/70 bg-muted/20"
    >
      <div
        class="flex min-w-0 flex-wrap items-center gap-2 border-b border-border/70 px-3 py-2 text-xs"
      >
        <Badge variant="secondary">{{ preview.format }}</Badge>
        <span class="min-w-0 break-all font-mono">{{
          preview.content_type
        }}</span>
        <span class="text-muted-foreground">{{ preview.byte_length }} B</span>
      </div>
      <pre
        class="m-0 block max-h-80 w-full min-w-0 max-w-full overflow-x-hidden overflow-y-auto whitespace-pre-wrap break-words p-3 text-xs [overflow-wrap:anywhere]"
        >{{ preview.body }}</pre>
      <div
        v-if="preview.missing_variables.length"
        class="break-words border-t border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800 [overflow-wrap:anywhere]"
      >
        {{ t("admin.notifications.body.missingVariables") }}:
        {{ preview.missing_variables.join(", ") }}
      </div>
    </div>
  </div>
</template>
