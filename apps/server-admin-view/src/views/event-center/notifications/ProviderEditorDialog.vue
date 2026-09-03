<script setup lang="ts">
import { useId } from "vue";
import { useI18n } from "vue-i18n";
import { AlertTriangle, Loader2, Send } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import type { NotificationProviderDefinition } from "@/types";
import SchemaFieldsEditor from "./SchemaFieldsEditor.vue";
import type { EditableProviderForm, ProviderDialogMode } from "./form-utils";

const a11yId = useId();

defineProps<{
  catalog: NotificationProviderDefinition[];
  connectionConfigInvalid: boolean;
  configuredSensitiveFields: string[];
  form: EditableProviderForm;
  generatedProviderName: string;
  mode: ProviderDialogMode;
  open: boolean;
  saving: boolean;
  selectedDefinition: NotificationProviderDefinition | null;
  showLegacyWebhookHeaderMigration: boolean;
  showWxPusherAlert: boolean;
  testingDraft: boolean;
}>();

const emit = defineEmits<{
  save: [];
  test: [];
  "preview-webhook-body": [];
  "type-change": [value: unknown];
  "update:open": [value: boolean];
}>();

const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="max-h-[85vh] min-w-0 overflow-x-hidden overflow-y-auto sm:max-w-[960px]"
    >
      <DialogHeader>
        <DialogTitle>
          {{
            mode === "create"
              ? t("admin.notifications.providers.createDialogTitle")
              : t("admin.notifications.providers.editDialogTitle")
          }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.notifications.providers.dialogDescription") }}
        </DialogDescription>
      </DialogHeader>

      <div class="min-w-0 space-y-5 py-2">
        <div class="grid gap-4 md:grid-cols-2">
          <div class="space-y-2">
            <Label :for="`${a11yId}-providereditordialog-1`">{{
              t("admin.notifications.providers.name")
            }}</Label>
            <Input
              :id="`${a11yId}-providereditordialog-1`"
              v-model="form.name"
              :placeholder="generatedProviderName"
              :disabled="saving"
            />
            <div class="text-xs text-muted-foreground">
              {{
                mode === "create"
                  ? t("admin.notifications.providers.createNameHelp", {
                      name: generatedProviderName,
                    })
                  : t("admin.notifications.providers.editNameHelp")
              }}
            </div>
          </div>

          <div class="space-y-2">
            <Label :for="`${a11yId}-providereditordialog-2`">
              {{ t("admin.notifications.providers.providerType") }}
            </Label>
            <Select
              :model-value="form.type"
              :disabled="mode === 'edit'"
              @update:model-value="emit('type-change', $event)"
            >
              <SelectTrigger :id="`${a11yId}-providereditordialog-2`">
                <SelectValue
                  :placeholder="
                    t('admin.notifications.providers.selectProviderType')
                  "
                />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="item in catalog"
                  :key="item.type"
                  :value="item.type"
                >
                  {{ item.label }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <Alert
          v-if="showWxPusherAlert"
          class="border-amber-200 bg-amber-50/80 text-amber-950"
        >
          <AlertTriangle class="h-4 w-4" />
          <AlertTitle>
            {{ t("admin.notifications.providers.wxpusherAlertTitle") }}
          </AlertTitle>
          <AlertDescription class="space-y-2">
            <p>{{ t("admin.notifications.providers.wxpusherAlertBody1") }}</p>
            <p>{{ t("admin.notifications.providers.wxpusherAlertBody2") }}</p>
          </AlertDescription>
        </Alert>

        <Alert
          v-if="showLegacyWebhookHeaderMigration"
          class="border-amber-200 bg-amber-50/80 text-amber-950"
        >
          <AlertTriangle class="h-4 w-4" />
          <AlertTitle>
            {{ t("admin.notifications.headers.migrationTitle") }}
          </AlertTitle>
          <AlertDescription>
            {{ t("admin.notifications.headers.migrationDescription") }}
          </AlertDescription>
        </Alert>

        <div class="flex items-center justify-between rounded-md border p-3">
          <div class="text-sm font-medium">
            {{ t("admin.notifications.providers.enabledStatus") }}
          </div>
          <Switch
            v-model="form.enabled"
            :aria-label="t('admin.notifications.providers.enabledStatus')"
          />
        </div>

        <div v-if="selectedDefinition" class="min-w-0 space-y-3">
          <div class="text-sm font-medium">
            {{ t("admin.notifications.providers.connectionConfig") }}
          </div>
          <SchemaFieldsEditor
            :fields="selectedDefinition.connection_schema"
            :model-value="form.connection_config"
            :configured-sensitive-fields="configuredSensitiveFields"
            :reveal-sensitive-values="mode === 'edit'"
            @update:model-value="form.connection_config = $event"
            @webhook-body-preview="emit('preview-webhook-body')"
          />
        </div>
      </div>

      <DialogFooter>
        <Button
          variant="outline"
          :disabled="saving || testingDraft"
          @click="emit('update:open', false)"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          variant="secondary"
          :disabled="saving || testingDraft || connectionConfigInvalid"
          @click="emit('test')"
        >
          <Loader2 v-if="testingDraft" class="mr-2 h-4 w-4 animate-spin" />
          <Send v-else class="mr-2 h-4 w-4" />
          {{ t("admin.notifications.providers.testProvider") }}
        </Button>
        <Button
          :disabled="saving || testingDraft || connectionConfigInvalid"
          @click="emit('save')"
        >
          <Loader2 v-if="saving" class="mr-2 h-4 w-4 animate-spin" />
          {{ t("common.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
