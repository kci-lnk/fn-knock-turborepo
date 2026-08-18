<script setup lang="ts">
import { useId } from "vue";
import { useI18n } from "vue-i18n";
import {
  AlertTriangle,
  CheckCircle2,
  Loader2,
  PlugZap,
  Save,
} from "lucide-vue-next";
import { Alert, AlertDescription } from "@/components/ui/alert";
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
import type {
  PanelProvider,
  PanelProviderDescriptor,
} from "@/lib/api/panel-sync-api";
import { panelApiPaths, type PanelSyncEditorForm } from "./panel-sync-model";
import ProviderPicker from "./ProviderPicker.vue";

defineProps<{
  autoSyncReady: boolean;
  draftVerified: boolean;
  form: PanelSyncEditorForm;
  isEditing: boolean;
  open: boolean;
  providers: PanelProviderDescriptor[];
  saving: boolean;
  testing: boolean;
}>();
const emit = defineEmits<{
  "update:open": [value: boolean];
  "select-provider": [provider: PanelProvider];
  save: [];
  test: [];
}>();
const { t } = useI18n();
const id = useId();
const endpointPlaceholder = (provider: PanelProvider) =>
  `https://panel.example.com${panelApiPaths[provider]}`;
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
      <DialogHeader>
        <DialogTitle>{{
          isEditing
            ? t("admin.panelSync.editor.editTitle")
            : t("admin.panelSync.editor.createTitle")
        }}</DialogTitle>
        <DialogDescription>{{
          t("admin.panelSync.editor.description")
        }}</DialogDescription>
      </DialogHeader>
      <div class="space-y-5 py-2">
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-2">
            <Label :for="`${id}-provider`">{{
              t("admin.panelSync.editor.provider")
            }}</Label>
            <ProviderPicker
              :input-id="`${id}-provider`"
              :model-value="form.provider"
              :providers="providers"
              :disabled="isEditing"
              @update:model-value="emit('select-provider', $event)"
            />
          </div>
          <div class="space-y-2">
            <Label :for="`${id}-name`">{{
              t("admin.panelSync.editor.name")
            }}</Label>
            <Input :id="`${id}-name`" v-model="form.name" autocomplete="off" />
          </div>
        </div>
        <div class="space-y-2">
          <Label :for="`${id}-url`">{{
            t("admin.panelSync.editor.endpointUrl")
          }}</Label>
          <Input
            :id="`${id}-url`"
            v-model="form.endpoint_url"
            :placeholder="endpointPlaceholder(form.provider)"
            autocomplete="url"
          />
          <p class="text-xs text-muted-foreground">
            {{ t("admin.panelSync.editor.endpointHint") }}
          </p>
        </div>
        <div class="space-y-2">
          <Label :for="`${id}-credential`">{{
            t("admin.panelSync.editor.credential")
          }}</Label>
          <Input
            :id="`${id}-credential`"
            v-model="form.credential"
            type="password"
            autocomplete="new-password"
            :disabled="form.clear_credential"
            :placeholder="
              isEditing ? t('admin.panelSync.editor.credentialKeep') : ''
            "
          />
          <label
            v-if="isEditing"
            class="flex items-center gap-2 text-sm text-muted-foreground"
          >
            <input v-model="form.clear_credential" type="checkbox" />
            {{ t("admin.panelSync.editor.clearCredential") }}
          </label>
        </div>
        <div
          class="grid gap-4"
          :class="form.grouping.mode === 'single' ? 'sm:grid-cols-2' : ''"
        >
          <div class="space-y-2">
            <Label :for="`${id}-group-mode`">{{
              t("admin.panelSync.editor.groupMode")
            }}</Label>
            <Select
              :model-value="form.grouping.mode"
              @update:model-value="
                form.grouping.mode =
                  $event as PanelSyncEditorForm['grouping']['mode']
              "
            >
              <SelectTrigger :id="`${id}-group-mode`" class="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="mirror">
                  {{ t("admin.panelSync.editor.mirror") }}
                </SelectItem>
                <SelectItem value="single">
                  {{ t("admin.panelSync.editor.single") }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div v-if="form.grouping.mode === 'single'" class="space-y-2">
            <Label :for="`${id}-namespace`">{{
              t("admin.panelSync.editor.singleGroup")
            }}</Label>
            <Input
              :id="`${id}-namespace`"
              v-model="form.grouping.single_group_name"
            />
          </div>
        </div>
        <div class="rounded-lg border p-3">
          <div class="flex items-center justify-between gap-4">
            <div>
              <div class="text-sm font-medium">
                {{ t("admin.panelSync.autoSync") }}
              </div>
              <div class="text-xs text-muted-foreground">
                {{ t("admin.panelSync.editor.autoHint") }}
              </div>
            </div>
            <Switch
              v-model="form.auto_sync.enabled"
              :disabled="form.clear_credential"
              :aria-label="t('admin.panelSync.autoSync')"
            />
          </div>
          <div class="mt-3 flex items-center gap-2">
            <Input
              :id="`${id}-interval`"
              v-model.number="form.auto_sync.interval_minutes"
              type="number"
              min="5"
              max="1440"
              class="w-28"
              :aria-label="
                t('admin.panelSync.everyMinutes', {
                  count: form.auto_sync.interval_minutes,
                })
              "
            />
            <span class="text-sm text-muted-foreground">{{
              t("admin.panelSync.minutes")
            }}</span>
          </div>
        </div>
        <Alert v-if="draftVerified">
          <CheckCircle2 class="h-4 w-4" />
          <AlertDescription>{{
            t("admin.panelSync.editor.draftTested")
          }}</AlertDescription>
        </Alert>
        <Alert v-else-if="!autoSyncReady">
          <AlertTriangle class="h-4 w-4" />
          <AlertDescription>{{
            t("admin.panelSync.editor.unverifiedHint")
          }}</AlertDescription>
        </Alert>
      </div>
      <DialogFooter class="gap-2 sm:gap-2">
        <Button
          variant="outline"
          :disabled="testing || saving || !form.endpoint_url.trim()"
          @click="emit('test')"
        >
          <Loader2 v-if="testing" class="mr-2 h-4 w-4 animate-spin" />
          <PlugZap v-else class="mr-2 h-4 w-4" />
          {{ t("admin.panelSync.testDraft") }}
        </Button>
        <Button
          :disabled="saving || !form.name.trim() || !form.endpoint_url.trim()"
          @click="emit('save')"
        >
          <Loader2 v-if="saving" class="mr-2 h-4 w-4 animate-spin" />
          <Save v-else class="mr-2 h-4 w-4" />
          {{ saving ? t("admin.panelSync.saving") : t("common.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
