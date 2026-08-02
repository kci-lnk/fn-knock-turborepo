<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

defineProps<{
  content: { title: string; description: string; items: string[] };
  dontShowAgain: boolean;
  open: boolean;
  saving: boolean;
}>();

const emit = defineEmits<{
  close: [];
  confirm: [];
  "update:dontShowAgain": [checked: boolean];
  "update:open": [open: boolean];
}>();

const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="overflow-hidden border-border bg-card p-0 text-card-foreground shadow-xl sm:max-w-[760px]"
    >
      <div class="px-8 pt-8 pb-6">
        <DialogHeader class="space-y-3 text-left">
          <p
            class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground"
          >
            {{ t("admin.runModeSettings.switchEyebrow") }}
          </p>
          <DialogTitle
            class="text-2xl font-semibold tracking-tight text-foreground"
          >
            {{ content.title }}
          </DialogTitle>
          <DialogDescription
            class="max-w-[56ch] text-sm leading-6 text-muted-foreground"
          >
            {{ content.description }}
          </DialogDescription>
        </DialogHeader>

        <ul class="mt-8 divide-y divide-border border-y border-border">
          <li
            v-for="(item, index) in content.items"
            :key="item"
            class="grid grid-cols-[auto_1fr] items-start gap-x-4 py-4"
          >
            <span
              class="pt-0.5 font-mono text-[11px] tracking-[0.18em] text-muted-foreground"
            >
              {{ String(index + 1).padStart(2, "0") }}
            </span>
            <p class="text-sm leading-6 text-foreground">{{ item }}</p>
          </li>
        </ul>

        <label
          class="mt-6 flex items-center gap-3 text-sm text-muted-foreground"
        >
          <Checkbox
            :model-value="dontShowAgain"
            @update:model-value="emit('update:dontShowAgain', $event === true)"
          />
          <span>{{ t("admin.runModeSettings.dontShowAgain") }}</span>
        </label>
      </div>

      <DialogFooter class="border-t border-border bg-muted/20 px-8 py-4">
        <Button variant="outline" @click="emit('close')">{{
          t("common.cancel")
        }}</Button>
        <Button :disabled="saving" @click="emit('confirm')">
          <span
            v-if="saving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          />
          {{ t("admin.runModeSettings.confirmSwitch") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
