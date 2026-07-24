<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Loader2 } from "lucide-vue-next";
import { toast } from "../../utils/toast";
import { copyTextToClipboard } from "../../utils/copyTextToClipboard";

const props = withDefaults(
  defineProps<{
    open: boolean;
    title: string;
    description?: string;
    maxWidthClass?: string;
    loading?: boolean;
    closeText?: string;
    closeVariant?:
      | "default"
      | "outline"
      | "secondary"
      | "ghost"
      | "destructive"
      | "link";
    showFooter?: boolean;
    copyText?: string | null;
    copyLabel?: string;
    copySuccessText?: string;
    copyUnverifiedText?: string;
    copyUnverifiedDescription?: string;
    copyErrorText?: string;
    copyDisabled?: boolean;
  }>(),
  {
    description: "",
    maxWidthClass: "sm:max-w-[700px]",
    loading: false,
    closeVariant: "outline",
    showFooter: true,
    copyDisabled: false,
  },
);

const emit = defineEmits<{
  "update:open": [value: boolean];
}>();

const { t } = useI18n();

const modelOpen = computed({
  get: () => props.open,
  set: (value: boolean) => emit("update:open", value),
});

const close = () => {
  modelOpen.value = false;
};

const showCopyButton = computed(() => props.copyText !== undefined);
const canCopy = computed(
  () => !props.copyDisabled && String(props.copyText ?? "").length > 0,
);
const closeText = computed(
  () => props.closeText ?? t("shared.detailDialog.close"),
);
const copyLabel = computed(
  () => props.copyLabel ?? t("shared.detailDialog.copyLog"),
);
const copySuccessText = computed(
  () => props.copySuccessText ?? t("shared.detailDialog.copySuccess"),
);
const copyUnverifiedText = computed(
  () => props.copyUnverifiedText ?? t("shared.detailDialog.copyUnverified"),
);
const copyUnverifiedDescription = computed(
  () =>
    props.copyUnverifiedDescription ??
    t("shared.detailDialog.copyUnverifiedDescription"),
);
const copyErrorText = computed(
  () => props.copyErrorText ?? t("shared.detailDialog.copyFailed"),
);

const copyDetailText = async () => {
  const text = String(props.copyText ?? "");
  if (!text) return;

  try {
    const result = await copyTextToClipboard(text);

    if (result.verified) {
      toast.success(copySuccessText.value);
      return;
    }

    toast.info(copyUnverifiedText.value, {
      description: copyUnverifiedDescription.value,
    });
  } catch {
    toast.error(copyErrorText.value, {
      description: t("shared.detailDialog.manualCopyHint"),
    });
  }
};
</script>

<template>
  <Dialog v-model:open="modelOpen">
    <DialogContent
      :class="[
        'flex max-h-[85vh] flex-col overflow-hidden',
        props.maxWidthClass,
      ]"
    >
      <DialogHeader class="shrink-0">
        <DialogTitle>{{ props.title }}</DialogTitle>
        <DialogDescription v-if="props.description">{{
          props.description
        }}</DialogDescription>
      </DialogHeader>

      <div class="min-h-0 flex-1 overflow-y-auto overflow-x-hidden pr-1">
        <div
          v-if="props.loading"
          class="py-10 text-center text-muted-foreground"
        >
          <Loader2 class="h-6 w-6 animate-spin mx-auto" />
        </div>
        <slot v-else />
      </div>

      <DialogFooter v-if="props.showFooter" class="shrink-0">
        <Button
          v-if="showCopyButton"
          variant="secondary"
          :disabled="!canCopy"
          @click="copyDetailText"
        >
          {{ copyLabel }}
        </Button>
        <Button :variant="props.closeVariant" @click="close">{{
          closeText
        }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
