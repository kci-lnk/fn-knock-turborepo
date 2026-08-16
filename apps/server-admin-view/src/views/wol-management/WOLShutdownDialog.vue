<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
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
import { Loader2, Power } from "lucide-vue-next";
import type { WOLTarget } from "@/lib/api/wol";

const props = defineProps<{
  open: boolean;
  target: WOLTarget | null;
  loading: boolean;
}>();

const emit = defineEmits<{
  confirm: [];
  "update:open": [value: boolean];
}>();

const { t } = useI18n();
const deadline = ref(0);
const now = ref(0);
let timer: number | null = null;

const stopTimer = () => {
  if (timer !== null) globalThis.clearInterval(timer);
  timer = null;
};

const startTimer = () => {
  stopTimer();
  now.value = Date.now();
  deadline.value = now.value + 3_000;
  timer = globalThis.setInterval(() => {
    now.value = Date.now();
    if (now.value >= deadline.value) stopTimer();
  }, 100);
};

watch(
  () => [props.open, props.target?.id] as const,
  ([open]) => {
    if (open) startTimer();
    else stopTimer();
  },
  { immediate: true },
);
onBeforeUnmount(stopTimer);

const remainingSeconds = computed(() =>
  Math.max(0, Math.ceil((deadline.value - now.value) / 1_000)),
);
const canConfirm = computed(
  () =>
    props.open &&
    Boolean(props.target) &&
    !props.loading &&
    deadline.value > 0 &&
    now.value >= deadline.value,
);
const confirmLabel = computed(() =>
  remainingSeconds.value > 0
    ? t("admin.wol.ssh.confirmShutdownCountdown", {
        seconds: remainingSeconds.value,
      })
    : t("admin.wol.ssh.confirmShutdown"),
);

const confirm = () => {
  if (!canConfirm.value || Date.now() < deadline.value) return;
  emit("confirm");
};
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="max-h-[calc(100dvh-1rem)] w-[calc(100%-1rem)] overflow-y-auto sm:max-w-md"
    >
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2 text-destructive">
          <Power class="h-5 w-5" />
          {{ t("admin.wol.ssh.shutdownTitle") }}
        </DialogTitle>
        <DialogDescription>
          {{
            t("admin.wol.ssh.shutdownDescription", {
              target: target?.name ?? "",
              host: target?.ssh.host ?? "",
            })
          }}
        </DialogDescription>
      </DialogHeader>
      <div
        class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm"
      >
        {{ t("admin.wol.ssh.shutdownWarning") }}
      </div>
      <DialogFooter>
        <Button
          type="button"
          variant="outline"
          class="w-full sm:w-auto"
          :disabled="loading"
          @click="emit('update:open', false)"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          data-testid="wol-confirm-shutdown"
          type="button"
          variant="destructive"
          class="w-full sm:w-auto"
          :disabled="!canConfirm"
          @click="confirm"
        >
          <Loader2 v-if="loading" class="mr-2 h-4 w-4 animate-spin" />
          {{ confirmLabel }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
