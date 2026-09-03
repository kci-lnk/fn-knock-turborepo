<script setup lang="ts">
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
import type { HostLocationAction } from "@/types";
import GatewayLocationAccessSection from "./GatewayLocationAccessSection.vue";
import GatewayLocationMatchSection from "./GatewayLocationMatchSection.vue";
import GatewayLocationProxyFields from "./GatewayLocationProxyFields.vue";
import GatewayLocationResponseFields from "./GatewayLocationResponseFields.vue";
import type { GatewayLocationForm } from "./gatewayLocationModel";

defineProps<{
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
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="flex max-h-[calc(100dvh-1rem)] w-[calc(100%-1rem)] flex-col gap-0 overflow-hidden p-0 sm:max-h-[90vh] sm:w-full sm:max-w-[800px]"
    >
      <DialogHeader class="border-b px-4 py-5 pr-12 sm:px-6">
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

      <div class="flex-1 overflow-y-auto px-4 sm:px-6">
        <div class="divide-y divide-border/60">
          <GatewayLocationMatchSection :form="form" />
          <GatewayLocationAccessSection :form="form" />

          <section
            aria-labelledby="location-action-heading"
            class="space-y-4 py-5"
          >
            <h3 id="location-action-heading" class="text-sm font-semibold">
              {{ t("admin.gatewayLocationsSettings.responseActionSection") }}
            </h3>

            <div
              role="group"
              :aria-label="t('admin.gatewayLocationsSettings.action')"
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
                :aria-pressed="form.action === 'proxy'"
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
                :aria-pressed="form.action === 'response'"
                @click="emit('setAction', 'response')"
              >
                {{ t("admin.gatewayLocationsSettings.fixedResponse") }}
              </button>
            </div>

            <GatewayLocationProxyFields
              v-if="form.action === 'proxy'"
              :form="form"
              :is-web-socket-target="isProxyLocationWebSocketTarget"
            />
            <GatewayLocationResponseFields
              v-else
              :form="form"
              @add-header="emit('addHeader')"
              @remove-header="emit('removeHeader', $event)"
            />
          </section>
        </div>

        <p
          v-if="formError"
          role="alert"
          class="mb-5 rounded-md bg-destructive/5 px-3 py-2.5 text-sm text-destructive"
        >
          {{ formError }}
        </p>
      </div>

      <DialogFooter
        class="border-t bg-background px-4 py-4 sm:flex-row sm:justify-end sm:px-6"
      >
        <Button
          class="w-full sm:w-auto"
          variant="outline"
          @click="emit('close')"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          class="w-full sm:w-auto"
          :disabled="!!formError || isSaving"
          @click="emit('save')"
        >
          {{ t("admin.gatewayLocationsSettings.saveRule") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
