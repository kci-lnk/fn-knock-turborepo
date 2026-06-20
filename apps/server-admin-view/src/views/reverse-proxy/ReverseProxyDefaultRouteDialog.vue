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

defineProps<{
  description: string;
  open: boolean;
  saving: boolean;
  showFnosHint: boolean;
  title: string;
}>();

const emit = defineEmits<{
  cancel: [];
  confirm: [];
  "update:open": [open: boolean];
}>();

const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription class="space-y-2 text-left">
          <p>{{ description }}</p>
          <p v-if="showFnosHint" class="text-amber-600">
            {{ t("admin.reverseProxy.fnosDefaultRouteHint") }}
          </p>
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="outline" :disabled="saving" @click="emit('cancel')">
          {{ t("admin.reverseProxy.cancel") }}
        </Button>
        <Button variant="destructive" :disabled="saving" @click="emit('confirm')">
          {{
            saving
              ? t("admin.reverseProxy.processing")
              : t("admin.reverseProxy.continueAction")
          }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
