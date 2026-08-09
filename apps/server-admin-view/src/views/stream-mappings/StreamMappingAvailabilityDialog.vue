<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[480px]">
      <DialogHeader>
        <DialogTitle>{{
          t("admin.streamMappings.availabilityTitle")
        }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.streamMappings.availabilityDescription") }}
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-5">
        <div
          class="flex items-center justify-between gap-4 rounded-md border p-3"
        >
          <div class="space-y-1">
            <Label for="stream-availability-enabled">
              {{ t("admin.streamMappings.availabilityEnabled") }}
            </Label>
            <p class="text-xs leading-5 text-muted-foreground">
              {{ t("admin.streamMappings.availabilityServerTimeHint") }}
            </p>
          </div>
          <Switch
            id="stream-availability-enabled"
            v-model="enabledModel"
            :disabled="loading"
          />
        </div>

        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-2">
            <Label for="stream-availability-start">
              {{ t("admin.streamMappings.availabilityStartTime") }}
            </Label>
            <Input
              id="stream-availability-start"
              v-model="startTimeModel"
              type="time"
              :disabled="!enabledModel || loading"
            />
          </div>
          <div class="space-y-2">
            <Label for="stream-availability-end">
              {{ t("admin.streamMappings.availabilityEndTime") }}
            </Label>
            <Input
              id="stream-availability-end"
              v-model="endTimeModel"
              type="time"
              :disabled="!enabledModel || loading"
            />
          </div>
        </div>
        <p v-if="validationMessage" class="text-sm text-destructive">
          {{ validationMessage }}
        </p>
      </div>

      <DialogFooter>
        <Button variant="outline" :disabled="loading" @click="emit('cancel')">
          {{ t("admin.streamMappings.cancel") }}
        </Button>
        <Button
          :disabled="loading || Boolean(validationMessage)"
          @click="emit('save')"
        >
          <span
            v-if="loading"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.streamMappings.saveSchedule") }}
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
  loading: boolean;
  open: boolean;
  startTime: string;
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
