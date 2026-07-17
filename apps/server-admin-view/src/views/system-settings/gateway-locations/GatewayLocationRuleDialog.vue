<script setup lang="ts">
import { computed } from "vue";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import ProxyTargetInputField from "@admin-shared/components/common/ProxyTargetInputField.vue";
import {
  buildProxyPathForwardingPreview,
  type ProxyPathForwardingMode,
} from "@admin-shared/utils/proxyPathForwarding";
import { Trash2 } from "lucide-vue-next";
import ResponseBodyEditor from "@/components/ResponseBodyEditor.vue";
import ResponseContentTypeField from "@/components/ResponseContentTypeField.vue";
import type { HostLocationAction } from "@/types";
import type { GatewayLocationForm } from "./gatewayLocationModel";

const props = defineProps<{
  editingIndex: number | null;
  form: GatewayLocationForm;
  formError: string;
  isProxyLocationWebSocketTarget: boolean;
  isSaving: boolean;
  open: boolean;
}>();

const emit = defineEmits<{
  addHeader: [];
  close: [];
  removeHeader: [index: number];
  save: [];
  setAction: [action: HostLocationAction];
  "update:open": [open: boolean];
}>();

const { t } = useI18n();

const pathForwardingMode = computed<ProxyPathForwardingMode>({
  get: () => (props.form.strip_path ? "strip" : "keep"),
  set: (mode) => {
    props.form.strip_path = mode === "strip";
  },
});

const pathForwardingPreview = computed(() =>
  buildProxyPathForwardingPreview({
    routePath: props.form.path,
    target: props.form.target,
    mode: pathForwardingMode.value,
  }),
);
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-h-[85vh] overflow-y-auto sm:max-w-[800px]">
      <DialogHeader>
        <DialogTitle>
          {{
            editingIndex === null
              ? t("admin.gatewayLocationsSettings.addRuleDialog")
              : t("admin.gatewayLocationsSettings.editRuleDialog")
          }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.gatewayLocationsSettings.ruleDialogDescription") }}
        </DialogDescription>
      </DialogHeader>

      <div class="grid gap-5">
        <div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_14rem]">
          <div class="space-y-2">
            <Label for="location-path">
              {{ t("admin.gatewayLocationsSettings.path") }}
            </Label>
            <Input id="location-path" v-model="form.path" placeholder="/api" />
          </div>
          <div class="space-y-2">
            <Label for="location-match">
              {{ t("admin.gatewayLocationsSettings.match") }}
            </Label>
            <Select v-model="form.match">
              <SelectTrigger id="location-match" class="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="exact">
                  {{ t("admin.gatewayLocationsSettings.exactMatch") }}
                </SelectItem>
                <SelectItem value="prefix">
                  {{ t("admin.gatewayLocationsSettings.prefixMatch") }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div class="space-y-2">
          <Label>{{ t("admin.gatewayLocationsSettings.action") }}</Label>
          <div
            class="grid grid-cols-2 rounded-lg bg-muted p-[3px] text-sm text-muted-foreground"
          >
            <button
              type="button"
              class="h-9 rounded-md font-medium transition-colors"
              :class="
                form.action === 'proxy'
                  ? 'bg-background text-foreground shadow-sm'
                  : 'hover:text-foreground'
              "
              @click="emit('setAction', 'proxy')"
            >
              {{ t("admin.gatewayLocationsSettings.proxyAction") }}
            </button>
            <button
              type="button"
              class="h-9 rounded-md font-medium transition-colors"
              :class="
                form.action === 'response'
                  ? 'bg-background text-foreground shadow-sm'
                  : 'hover:text-foreground'
              "
              @click="emit('setAction', 'response')"
            >
              {{ t("admin.gatewayLocationsSettings.fixedResponse") }}
            </button>
          </div>
        </div>

        <template v-if="form.action === 'proxy'">
          <div class="space-y-2">
            <Label for="location-target">
              {{ t("admin.gatewayLocationsSettings.target") }}
            </Label>
            <ProxyTargetInputField
              v-model="form.target"
              input-id="location-target"
              protocol-id="location-target-protocol"
              placeholder="127.0.0.1:8080"
            />
          </div>
          <div class="grid gap-3 sm:grid-cols-2">
            <div class="space-y-3 rounded-md border px-4 py-3">
              <div class="space-y-2">
                <Label for="location-path-forwarding">
                  {{ t("admin.gatewayLocationsSettings.pathForwarding") }}
                </Label>
                <Select v-model="pathForwardingMode">
                  <SelectTrigger id="location-path-forwarding" class="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="strip">
                      {{
                        t("admin.gatewayLocationsSettings.pathForwardingStrip")
                      }}
                    </SelectItem>
                    <SelectItem value="keep">
                      {{
                        t("admin.gatewayLocationsSettings.pathForwardingKeep")
                      }}
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div class="space-y-1 text-xs text-muted-foreground">
                <div class="font-medium text-foreground/80">
                  {{ t("admin.gatewayLocationsSettings.pathPreview") }}
                </div>
                <div class="grid gap-1 font-mono">
                  <span class="break-all">
                    {{ pathForwardingPreview.requestPath }}
                  </span>
                  <span class="text-foreground">-&gt;</span>
                  <span class="break-all">
                    {{ pathForwardingPreview.upstreamPath }}
                  </span>
                </div>
              </div>
            </div>
            <div
              v-if="!isProxyLocationWebSocketTarget"
              class="flex items-center justify-between gap-4 rounded-md border px-4 py-3"
            >
              <Label for="location-rewrite-html">
                {{ t("admin.gatewayLocationsSettings.rewriteHtmlPath") }}
              </Label>
              <Switch id="location-rewrite-html" v-model="form.rewrite_html" />
            </div>
          </div>
        </template>

        <template v-else>
          <div
            class="grid gap-3 rounded-md border border-border/60 bg-muted/10 p-4 sm:grid-cols-[8.5rem_minmax(0,1fr)]"
          >
            <div class="space-y-2">
              <Label for="response-status">
                {{ t("admin.gatewayLocationsSettings.statusCode") }}
              </Label>
              <Input
                id="response-status"
                v-model.number="form.response.status"
                type="number"
                min="100"
                max="599"
              />
            </div>
            <ResponseContentTypeField
              v-model="form.response.content_type"
              input-id="response-content-type"
              select-id="response-content-type-preset"
            />
          </div>

          <div class="space-y-3 rounded-md border px-4 py-3">
            <div class="flex items-center justify-between gap-3">
              <Label>
                {{ t("admin.gatewayLocationsSettings.responseHeaders") }}
              </Label>
              <Button
                type="button"
                variant="outline"
                size="sm"
                @click="emit('addHeader')"
              >
                {{ t("admin.gatewayLocationsSettings.addResponseHeader") }}
              </Button>
            </div>
            <div
              v-if="form.headers.length === 0"
              class="text-sm text-muted-foreground"
            >
              {{ t("admin.gatewayLocationsSettings.noCustomResponseHeaders") }}
            </div>
            <div
              v-for="(header, index) in form.headers"
              :key="index"
              class="grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_2.5rem]"
            >
              <Input v-model="header.name" placeholder="X-Example" />
              <Input v-model="header.value" placeholder="value" />
              <ConfirmDangerPopover
                :title="t('admin.gatewayLocationsSettings.deleteHeaderTitle')"
                :description="
                  t('admin.gatewayLocationsSettings.deleteHeaderDescription', {
                    name:
                      header.name.trim() ||
                      t('admin.gatewayLocationsSettings.unnamedHeader'),
                  })
                "
                :confirm-text="
                  t('admin.gatewayLocationsSettings.confirmDelete')
                "
                :on-confirm="() => emit('removeHeader', index)"
                content-class="w-64 text-left"
              >
                <template #trigger>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                  >
                    <Trash2 class="h-4 w-4" />
                    <span class="sr-only">
                      {{ t("admin.gatewayLocationsSettings.deleteHeaderSr") }}
                    </span>
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </div>
          </div>

          <ResponseBodyEditor
            v-model="form.response.body"
            :content-type="form.response.content_type"
          />
        </template>

        <p v-if="formError" class="text-sm text-destructive">
          {{ formError }}
        </p>
      </div>

      <DialogFooter>
        <Button variant="outline" @click="emit('close')">
          {{ t("common.cancel") }}
        </Button>
        <Button :disabled="!!formError || isSaving" @click="emit('save')">
          {{ t("admin.gatewayLocationsSettings.saveRule") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
