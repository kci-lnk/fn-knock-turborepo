<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import ResponseBodyEditor from "@/components/ResponseBodyEditor.vue";
import ResponseContentTypeField from "@/components/ResponseContentTypeField.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import type { GatewayLocationForm } from "./gatewayLocationModel";

defineProps<{ form: GatewayLocationForm }>();
const emit = defineEmits<{
  addHeader: [];
  removeHeader: [index: number];
}>();
const { t } = useI18n();
</script>

<template>
  <div class="space-y-4">
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

    <ResponseBodyEditor
      v-model="form.response.body"
      :content-type="form.response.content_type"
    />

    <div class="space-y-3 rounded-md border border-border/60 px-4 py-3">
      <div class="flex items-center justify-between gap-3">
        <div class="text-sm font-medium">
          {{ t("admin.gatewayLocationsSettings.responseHeaders") }}
        </div>
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
        <Input
          v-model="header.name"
          aria-label="X-Example"
          placeholder="X-Example"
        />
        <Input v-model="header.value" aria-label="value" placeholder="value" />
        <ConfirmDangerPopover
          :title="t('admin.gatewayLocationsSettings.deleteHeaderTitle')"
          :description="
            t('admin.gatewayLocationsSettings.deleteHeaderDescription', {
              name:
                header.name.trim() ||
                t('admin.gatewayLocationsSettings.unnamedHeader'),
            })
          "
          :confirm-text="t('admin.gatewayLocationsSettings.confirmDelete')"
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
  </div>
</template>
