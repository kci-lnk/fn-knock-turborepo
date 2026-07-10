<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronLeft, ChevronRight, Copy } from "lucide-vue-next";
import QrcodeVue from "qrcode.vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  InputOTP,
  InputOTPGroup,
  InputOTPSlot,
} from "@/components/ui/input-otp";
import { Label } from "@/components/ui/label";

type SetupData = { secret: string; uri: string };

const props = defineProps<{
  bindErrorMessage: string;
  bindView: "qr" | "manual";
  comment: string;
  description: string;
  enterFromClass: string;
  isBinding: boolean;
  leaveToClass: string;
  open: boolean;
  secretDisplay: string;
  setupData: SetupData | null;
  step: "BIND" | "NAME";
  title: string;
  verifyToken: string;
}>();

const emit = defineEmits<{
  "update:comment": [value: string];
  "update:open": [value: boolean];
  "update:verifyToken": [value: string];
  bind: [];
  cancel: [];
  copySecret: [];
  openManual: [];
  returnToQr: [];
  saveName: [];
}>();

const { t } = useI18n();
const otpInputAreaRef = ref<HTMLElement | null>(null);
let viewportResizeTimer: ReturnType<typeof window.setTimeout> | null = null;

const verificationToken = computed({
  get: () => props.verifyToken,
  set: (value: string) => emit("update:verifyToken", value),
});
const setupComment = computed({
  get: () => props.comment,
  set: (value: string) => emit("update:comment", value),
});

function handleOpenChange(open: boolean) {
  emit("update:open", open);
  if (!open) emit("cancel");
}

function scrollOtpIntoView(behavior: ScrollBehavior = "smooth") {
  otpInputAreaRef.value?.scrollIntoView({
    block: "center",
    inline: "nearest",
    behavior,
  });
}

function handleDialogFocusIn(event: FocusEvent) {
  if (props.step !== "BIND") return;
  const target = event.target as HTMLElement | null;
  if (!target || !otpInputAreaRef.value?.contains(target)) return;
  window.setTimeout(() => {
    scrollOtpIntoView();
  }, 120);
}

function handleVisualViewportResize() {
  if (!props.open || props.step !== "BIND") return;
  const viewport = window.visualViewport;
  if (!viewport) return;

  const keyboardHeight = window.innerHeight - viewport.height;
  if (keyboardHeight < 120) return;

  if (viewportResizeTimer) {
    window.clearTimeout(viewportResizeTimer);
  }
  viewportResizeTimer = window.setTimeout(() => {
    scrollOtpIntoView();
  }, 80);
}

watch(
  () => [props.open, props.step, props.setupData] as const,
  async ([isOpen, step, setup]) => {
    if (!isOpen || step !== "BIND" || !setup) return;
    await nextTick();
    scrollOtpIntoView("auto");
  },
);

onMounted(() => {
  window.visualViewport?.addEventListener("resize", handleVisualViewportResize);
});

onBeforeUnmount(() => {
  window.visualViewport?.removeEventListener(
    "resize",
    handleVisualViewportResize,
  );
  if (viewportResizeTimer) {
    window.clearTimeout(viewportResizeTimer);
    viewportResizeTimer = null;
  }
});
</script>

<template>
  <Dialog :open="open" @update:open="handleOpenChange">
    <DialogContent
      class="max-w-md !top-[5vh] !translate-y-0 max-h-[85vh] overflow-y-auto overscroll-contain max-sm:!inset-x-0 max-sm:!top-auto max-sm:!bottom-0 max-sm:!translate-x-0 max-sm:!translate-y-0 max-sm:!max-w-none max-sm:max-h-[100dvh] max-sm:rounded-b-none max-sm:border-b-0 max-sm:pb-[calc(env(safe-area-inset-bottom)+1rem)]"
      @focusin="handleDialogFocusIn"
    >
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription>{{ description }}</DialogDescription>
      </DialogHeader>
      <div
        v-if="setupData && step === 'BIND'"
        class="w-full py-4 max-sm:py-2"
      >
        <Transition
          mode="out-in"
          enter-active-class="transition duration-150 ease-out"
          leave-active-class="transition duration-100 ease-in"
          :enter-from-class="enterFromClass"
          enter-to-class="translate-x-0 opacity-100"
          leave-from-class="translate-x-0 opacity-100"
          :leave-to-class="leaveToClass"
        >
          <div
            v-if="bindView === 'qr'"
            key="setup-qr"
            class="flex flex-col items-center gap-4"
          >
            <div class="rounded-xl border bg-white p-4">
              <QrcodeVue :value="setupData.uri" :size="200" level="M" />
            </div>
            <Button
              type="button"
              variant="link"
              class="h-auto gap-1 px-0 text-sm"
              @click="emit('openManual')"
            >
              {{ t("admin.authSettings.manualSetupEntry") }}
              <ChevronRight class="h-4 w-4" />
            </Button>
          </div>

          <div v-else key="setup-manual" class="w-full space-y-4">
            <button
              type="button"
              class="-mx-2 inline-flex w-[calc(100%+1rem)] items-center gap-3 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              :aria-label="t('admin.authSettings.backToQRCodeSetupAria')"
              @click="emit('returnToQr')"
            >
              <ChevronLeft class="h-4 w-4 shrink-0" />
              <span class="text-sm font-semibold">
                {{ t("admin.authSettings.manualSetupTitle") }}
              </span>
            </button>
            <div class="space-y-3 rounded-md border bg-muted/30 p-3">
              <p class="text-xs leading-5 text-muted-foreground">
                {{ t("admin.authSettings.manualSetupDescription") }}
              </p>
              <div
                class="flex items-start gap-2 rounded-md border bg-background px-2.5 py-2"
              >
                <div class="min-w-0 flex-1 space-y-1">
                  <Label class="text-xs text-muted-foreground">
                    {{ t("admin.authSettings.manualSetupSecretLabel") }}
                  </Label>
                  <p
                    class="break-all font-mono text-xs leading-5 text-muted-foreground"
                  >
                    {{ secretDisplay }}
                  </p>
                </div>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  class="size-8 shrink-0"
                  :title="t('admin.authSettings.copySetupSecret')"
                  :aria-label="t('admin.authSettings.copySetupSecret')"
                  @click="emit('copySecret')"
                >
                  <Copy class="h-4 w-4" />
                </Button>
              </div>
            </div>
          </div>
        </Transition>

        <div class="mt-6 w-full space-y-4 max-sm:mt-4">
          <div
            ref="otpInputAreaRef"
            class="space-y-2 flex flex-col items-center scroll-mt-24"
          >
            <Label class="text-sm text-muted-foreground self-center">
              {{ t("admin.authSettings.otpLabel") }}
            </Label>
            <div class="w-full flex justify-center py-2">
              <InputOTP
                v-model="verificationToken"
                inputmode="numeric"
                :maxlength="6"
                :disabled="isBinding"
                :autofocus="true"
                autocomplete="off"
                data-form-type="other"
                data-1p-ignore="true"
                data-lpignore="true"
                data-bwignore="true"
                @complete="emit('bind')"
              >
                <InputOTPGroup>
                  <InputOTPSlot v-for="i in 6" :key="i - 1" :index="i - 1" />
                </InputOTPGroup>
              </InputOTP>
            </div>
            <p v-if="isBinding" class="text-sm text-muted-foreground">
              {{ t("admin.authSettings.verifying") }}
            </p>
            <p v-if="bindErrorMessage" class="text-sm text-destructive">
              {{ bindErrorMessage }}
            </p>
          </div>
        </div>
      </div>
      <div v-else-if="step === 'NAME'" class="flex flex-col gap-4 py-4">
        <div class="space-y-2">
          <Label>{{ t("admin.authSettings.nameSuccessLabel") }}</Label>
          <Input
            v-model="setupComment"
            :placeholder="t('admin.authSettings.namePlaceholder')"
            @keyup.enter="emit('saveName')"
          />
          <p class="text-xs text-muted-foreground">
            {{ t("admin.authSettings.nameHelp") }}
          </p>
        </div>
        <p v-if="bindErrorMessage" class="text-sm text-destructive">
          {{ bindErrorMessage }}
        </p>
        <div class="flex justify-end gap-2 mt-4">
          <Button :disabled="isBinding" @click="emit('saveName')">
            <span
              v-if="isBinding"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{ t("common.save") }}
          </Button>
        </div>
      </div>
      <div v-else class="flex items-center justify-center py-12">
        <span
          class="animate-spin h-5 w-5 border-2 border-primary border-t-transparent rounded-full mr-2"
        ></span>{{ t("admin.authSettings.generating") }}
      </div>
    </DialogContent>
  </Dialog>
</template>
