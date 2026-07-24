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
import { Switch } from "@/components/ui/switch";
import ProxyTargetInputField from "@admin-shared/components/common/ProxyTargetInputField.vue";
import type { ProxyMapping } from "@/types";

const props = defineProps<{
  form: ProxyMapping;
  isEditing: boolean;
  isSaving: boolean;
  isValid: boolean;
  isWebSocketTarget: boolean;
  open: boolean;
}>();

const emit = defineEmits<{
  close: [];
  save: [];
  "update:open": [open: boolean];
  updateForm: [patch: Partial<ProxyMapping>];
}>();

const { t } = useI18n();

const pathModel = computed({
  get: () => props.form.path,
  set: (path: string) => emit("updateForm", { path }),
});

const targetModel = computed({
  get: () => props.form.target,
  set: (target: string) => emit("updateForm", { target }),
});

const rewriteHtmlModel = computed({
  get: () => props.form.rewrite_html,
  set: (rewriteHtml: boolean) =>
    emit("updateForm", { rewrite_html: rewriteHtml }),
});

const useAuthModel = computed({
  get: () => props.form.use_auth,
  set: (useAuth: boolean) => emit("updateForm", { use_auth: useAuth }),
});

const useRootModeModel = computed({
  get: () => props.form.use_root_mode,
  set: (useRootMode: boolean) =>
    emit("updateForm", { use_root_mode: useRootMode }),
});

const stripPathModel = computed({
  get: () => props.form.strip_path,
  set: (stripPath: boolean) => emit("updateForm", { strip_path: stripPath }),
});
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[425px]">
      <DialogHeader>
        <DialogTitle>
          {{
            isEditing
              ? t("admin.reverseProxy.editTitle")
              : t("admin.reverseProxy.addTitle")
          }}
        </DialogTitle>
        <DialogDescription>
          {{
            isEditing
              ? t("admin.reverseProxy.editDescription")
              : t("admin.reverseProxy.addDescription")
          }}
        </DialogDescription>
      </DialogHeader>
      <div class="grid gap-4 py-4">
        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="path" class="text-right">
            {{ t("admin.reverseProxy.pathLabel") }}
          </Label>
          <Input
            id="path"
            v-model="pathModel"
            :placeholder="t('admin.reverseProxy.pathPlaceholder')"
            class="col-span-3"
          />
        </div>
        <div class="grid grid-cols-4 items-start gap-4">
          <Label for="target-endpoint" class="pt-2 text-right">
            {{ t("admin.reverseProxy.targetLabel") }}
          </Label>
          <ProxyTargetInputField
            v-model="targetModel"
            input-id="target-endpoint"
            protocol-id="target-protocol"
            :placeholder="t('admin.reverseProxy.targetPlaceholder')"
            class="col-span-3"
          />
        </div>

        <div class="grid grid-cols-4 items-center gap-4">
          <div class="text-right">
            {{ t("admin.reverseProxy.optionsLabel") }}
          </div>
          <div
            role="group"
            :aria-label="t('admin.reverseProxy.optionsLabel')"
            class="col-span-3 space-y-2"
          >
            <div v-if="!isWebSocketTarget" class="flex items-center space-x-2">
              <Switch id="rewrite" v-model="rewriteHtmlModel" />
              <Label for="rewrite">
                {{ t("admin.reverseProxy.rewriteHtmlContent") }}
              </Label>
            </div>
            <div class="flex items-center space-x-2">
              <Switch id="auth" v-model="useAuthModel" />
              <Label for="auth">
                {{ t("admin.reverseProxy.requireAuth") }}
              </Label>
            </div>
            <div v-if="!isWebSocketTarget" class="flex items-center space-x-2">
              <Switch id="root" v-model="useRootModeModel" />
              <Label for="root">
                {{ t("admin.reverseProxy.useRootMode") }}
              </Label>
            </div>
            <div class="flex items-center space-x-2">
              <Switch id="strip" v-model="stripPathModel" />
              <Label for="strip">
                {{ t("admin.reverseProxy.stripRequestPrefix") }}
              </Label>
            </div>
          </div>
        </div>
      </div>
      <DialogFooter>
        <Button variant="outline" @click="emit('close')">
          {{ t("admin.reverseProxy.cancel") }}
        </Button>
        <Button :disabled="!isValid || isSaving" @click="emit('save')">
          {{ t("admin.reverseProxy.saveSettings") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
