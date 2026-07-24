<script setup lang="ts">
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

defineProps<{
  isClearing: boolean;
  open: boolean;
}>();

const emit = defineEmits<{
  confirm: [];
  "update:open": [value: boolean];
}>();

const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-[420px]">
      <DialogHeader>
        <DialogTitle>{{ t("admin.ddns.clearPrimaryTitle") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.ddns.clearPrimaryDescription") }}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button
          variant="outline"
          :disabled="isClearing"
          @click="emit('update:open', false)"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          variant="destructive"
          :disabled="isClearing"
          @click="emit('confirm')"
        >
          <RefreshCw v-if="isClearing" class="mr-2 h-4 w-4 animate-spin" />
          {{
            isClearing ? t("admin.ddns.clearing") : t("admin.ddns.confirmClear")
          }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
