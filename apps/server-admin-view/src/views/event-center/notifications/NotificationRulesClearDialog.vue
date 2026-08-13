<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Loader2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { useNotificationRules } from "./useNotificationRules";

const props = defineProps<{
  controller: ReturnType<typeof useNotificationRules>;
}>();
const { clearAllDialogOpen, clearingAll, clearAllRules, rules } =
  props.controller;
const { t } = useI18n();
</script>

<template>
  <Dialog v-model:open="clearAllDialogOpen">
    <DialogContent class="sm:max-w-[420px]">
      <DialogHeader>
        <DialogTitle>
          {{ t("admin.notifications.rules.clearDialogTitle") }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.notifications.rules.clearDialogDescription") }}
        </DialogDescription>
      </DialogHeader>

      <DialogFooter class="gap-2">
        <Button
          variant="outline"
          :disabled="clearingAll"
          @click="clearAllDialogOpen = false"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          variant="destructive"
          :disabled="clearingAll || rules.length === 0"
          @click="clearAllRules"
        >
          <Loader2 v-if="clearingAll" class="mr-2 h-4 w-4 animate-spin" />
          {{ t("admin.notifications.rules.clearAllRules") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
