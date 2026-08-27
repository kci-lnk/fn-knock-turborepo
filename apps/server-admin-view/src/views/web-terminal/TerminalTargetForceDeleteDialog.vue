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
import { LoaderCircle, TriangleAlert } from "lucide-vue-next";

defineProps<{
  activeSessionCount: number;
  canConfirm: boolean;
  deleting: boolean;
  message: string;
  open: boolean;
  targetName: string;
}>();

const emit = defineEmits<{
  close: [];
  confirm: [];
}>();
const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="$event ? undefined : emit('close')">
    <DialogContent class="sm:max-w-[460px]">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2 text-destructive">
          <TriangleAlert class="h-5 w-5" />
          {{ t("admin.webTerminal.forceDeleteTargetTitle") }}
        </DialogTitle>
        <DialogDescription>
          {{
            t("admin.webTerminal.forceDeleteTargetDescription", {
              target: targetName,
            })
          }}
        </DialogDescription>
      </DialogHeader>

      <div
        class="rounded-lg border border-destructive/25 bg-destructive/5 p-3 text-sm"
      >
        <p>{{ message }}</p>
        <p v-if="activeSessionCount > 0" class="mt-2 text-muted-foreground">
          {{
            t("admin.webTerminal.forceDeleteSessionCount", {
              count: activeSessionCount,
            })
          }}
        </p>
      </div>

      <DialogFooter>
        <Button variant="outline" :disabled="deleting" @click="emit('close')">
          {{ t("common.cancel") }}
        </Button>
        <Button
          variant="destructive"
          :disabled="deleting || !canConfirm"
          @click="emit('confirm')"
        >
          <LoaderCircle v-if="deleting" class="mr-1.5 h-4 w-4 animate-spin" />
          {{ t("admin.webTerminal.forceDeleteTargetAction") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
