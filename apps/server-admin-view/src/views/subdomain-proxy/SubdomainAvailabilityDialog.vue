<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[480px]">
      <DialogHeader>
        <DialogTitle>{{ title || t("admin.subdomainProxy.availabilityTitle") }}</DialogTitle>
        <DialogDescription>
          {{
            description ||
            t("admin.subdomainProxy.availabilityDescription", { host })
          }}
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-5">
        <div
          class="flex items-center justify-between gap-4 rounded-md border p-3"
        >
          <div class="space-y-1">
            <Label for="host-availability-enabled">
              {{ t("admin.subdomainProxy.availabilityEnabled") }}
            </Label>
            <p class="text-xs leading-5 text-muted-foreground">
              {{ t("admin.subdomainProxy.availabilityServerTimeHint") }}
            </p>
          </div>
          <Switch
            id="host-availability-enabled"
            v-model="enabledModel"
            :disabled="loading"
          />
        </div>

        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-2">
            <Label for="host-availability-start">
              {{ t("admin.subdomainProxy.availabilityStartTime") }}
            </Label>
            <Input
              id="host-availability-start"
              type="time"
              v-model="startTimeModel"
              :disabled="!enabledModel || loading"
            />
          </div>
          <div class="space-y-2">
            <Label for="host-availability-end">
              {{ t("admin.subdomainProxy.availabilityEndTime") }}
            </Label>
            <Input
              id="host-availability-end"
              type="time"
              v-model="endTimeModel"
              :disabled="!enabledModel || loading"
            />
          </div>
        </div>
        <p v-if="validationMessage" class="text-sm text-destructive">
          {{ validationMessage }}
        </p>
      </div>

      <DialogFooter>
        <Button variant="outline" @click="emit('cancel')">
          {{ t("admin.subdomainProxy.cancel") }}
        </Button>
        <Button
          :disabled="loading || Boolean(validationMessage)"
          @click="emit('save')"
        >
          <span
            v-if="loading"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ saveLabel || t("admin.subdomainProxy.saveMapping") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

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

const props = defineProps<{
  enabled: boolean;
  description?: string;
  host: string;
  loading: boolean;
  open: boolean;
  startTime: string;
  saveLabel?: string;
  title?: string;
  endTime: string;
  validationMessage: string;
}>();

const emit = defineEmits<{
  cancel: [];
  save: [];
  "update:enabled": [enabled: boolean];
  "update:endTime": [value: string];
  "update:open": [open: boolean];
  "update:startTime": [value: string];
}>();

const { t } = useI18n();

const enabledModel = computed({
  get: () => props.enabled,
  set: (value: boolean) => emit("update:enabled", value),
});

const startTimeModel = computed({
  get: () => props.startTime,
  set: (value: string | number) => emit("update:startTime", String(value)),
});

const endTimeModel = computed({
  get: () => props.endTime,
  set: (value: string | number) => emit("update:endTime", String(value)),
});
</script>
