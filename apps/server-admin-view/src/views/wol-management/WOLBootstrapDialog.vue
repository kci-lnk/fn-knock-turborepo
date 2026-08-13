<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Copy, Link2, TriangleAlert } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { type WOLRelayCredentialResult } from "@/lib/api/wol";

defineProps<{
  open: boolean;
  credential: WOLRelayCredentialResult | null;
}>();

const emit = defineEmits<{
  copy: [value: string];
  "update:open": [value: boolean];
}>();

const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-xl" @escape-key-down.prevent>
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Link2 class="h-5 w-5" />
          {{ t("admin.wol.bootstrap.title") }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.wol.bootstrap.description") }}
        </DialogDescription>
      </DialogHeader>

      <ol class="space-y-3 rounded-xl border bg-muted/20 p-4 text-sm">
        <li class="flex gap-3">
          <span class="font-semibold text-primary">1</span>
          <span>{{ t("admin.wol.bootstrap.stepCopy") }}</span>
        </li>
        <li class="flex gap-3">
          <span class="font-semibold text-primary">2</span>
          <span>{{ t("admin.wol.bootstrap.stepOpenRemote") }}</span>
        </li>
        <li class="flex gap-3">
          <span class="font-semibold text-primary">3</span>
          <span>{{ t("admin.wol.bootstrap.stepPair") }}</span>
        </li>
      </ol>

      <div v-if="credential" class="space-y-2">
        <div class="flex items-center justify-between gap-3">
          <p class="text-sm font-medium">{{ t("admin.wol.bootstrap.code") }}</p>
          <Button
            size="sm"
            @click="emit('copy', credential.bootstrap.pairingCode)"
          >
            <Copy class="mr-1.5 h-3.5 w-3.5" />
            {{ t("admin.wol.bootstrap.copyCode") }}
          </Button>
        </div>
        <pre
          class="max-h-40 overflow-auto rounded-lg border bg-muted/50 p-3 font-mono text-xs break-all whitespace-pre-wrap select-all"
          >{{ credential.bootstrap.pairingCode }}</pre>
      </div>

      <Alert variant="destructive">
        <TriangleAlert class="h-4 w-4" />
        <AlertTitle>{{ t("admin.wol.bootstrap.onceTitle") }}</AlertTitle>
        <AlertDescription>
          {{ t("admin.wol.bootstrap.onceDescription") }}
        </AlertDescription>
      </Alert>

      <DialogFooter>
        <Button @click="emit('update:open', false)">
          {{ t("admin.wol.bootstrap.saved") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
