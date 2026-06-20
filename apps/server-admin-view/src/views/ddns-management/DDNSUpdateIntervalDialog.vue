<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw } from "lucide-vue-next";
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
  MAX_DDNS_UPDATE_INTERVAL_MINUTES,
  MIN_DDNS_UPDATE_INTERVAL_MINUTES,
} from "./model";

const props = defineProps<{
  draft: string;
  isSaving: boolean;
  open: boolean;
}>();

const emit = defineEmits<{
  confirm: [];
  "update:draft": [value: string];
  "update:open": [value: boolean];
}>();

const { t } = useI18n();

const draftValue = computed({
  get: () => props.draft,
  set: (value: string) => emit("update:draft", value),
});
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[420px]">
      <DialogHeader>
        <DialogTitle>{{ t("admin.ddns.intervalDialogTitle") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.ddns.intervalDialogDescription") }}
        </DialogDescription>
      </DialogHeader>

      <div class="grid gap-2 py-2">
        <Label for="ddns-update-interval">
          {{ t("admin.ddns.intervalMinutes") }}
        </Label>
        <div class="relative">
          <Input
            id="ddns-update-interval"
            v-model="draftValue"
            type="number"
            inputmode="numeric"
            :min="MIN_DDNS_UPDATE_INTERVAL_MINUTES"
            :max="MAX_DDNS_UPDATE_INTERVAL_MINUTES"
            step="1"
            class="pr-14"
            @keydown.enter.prevent="emit('confirm')"
          />
          <span
            class="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-sm text-muted-foreground"
          >
            {{ t("admin.ddns.minutes") }}
          </span>
        </div>
        <p class="text-xs text-muted-foreground">
          {{
            t("admin.ddns.intervalHelp", {
              min: MIN_DDNS_UPDATE_INTERVAL_MINUTES,
              max: MAX_DDNS_UPDATE_INTERVAL_MINUTES,
            })
          }}
        </p>
      </div>

      <DialogFooter class="gap-2">
        <Button
          variant="outline"
          :disabled="isSaving"
          @click="emit('update:open', false)"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button :disabled="isSaving" @click="emit('confirm')">
          <RefreshCw v-if="isSaving" class="mr-1.5 h-4 w-4 animate-spin" />
          {{ isSaving ? t("admin.ddns.saving") : t("common.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
