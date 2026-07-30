<template>
  <div class="group relative min-h-[24px] min-w-0 max-w-full">
    <div class="flex min-w-0 items-center">
      <span
        v-if="!isEditing"
        class="min-w-0 flex-1 truncate pr-7 text-sm"
        :title="displayText"
      >
        {{ displayText }}
      </span>
      <Input
        v-else
        ref="inputRef"
        v-model="draft"
        :aria-label="editLabel"
        class="h-7 min-w-0 flex-1 px-2 py-1 text-sm"
        :disabled="isSaving"
        :placeholder="placeholderText"
        @keyup="handleKeyup"
      />

      <div
        v-if="!isEditing"
        class="pointer-events-none absolute right-0 top-1/2 -translate-y-1/2 opacity-0 transition-opacity group-hover:pointer-events-auto group-hover:opacity-100 group-focus-within:pointer-events-auto group-focus-within:opacity-100 [@media(hover:none)]:pointer-events-auto [@media(hover:none)]:opacity-100"
      >
        <Button
          variant="ghost"
          size="icon"
          class="h-6 w-6"
          :title="editLabel"
          :aria-label="editLabel"
          @click="startEdit"
        >
          <Pencil class="h-3 w-3" />
        </Button>
      </div>
      <div v-else class="ml-1 flex shrink-0 gap-1">
        <Button
          variant="ghost"
          size="icon"
          class="h-6 w-6 text-green-600"
          :disabled="isSaving"
          :aria-label="saveLabel"
          :title="saveLabel"
          @click="saveEdit"
        >
          <Check class="h-3 w-3" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="h-6 w-6 text-red-600"
          :disabled="isSaving"
          :aria-label="cancelLabel"
          :title="cancelLabel"
          @click="cancelEdit"
        >
          <X class="h-3 w-3" />
        </Button>
      </div>
    </div>
    <p
      v-if="isEditing && warningMessage"
      class="mt-1 text-xs text-amber-600 dark:text-amber-400"
      role="status"
    >
      {{ warningMessage }}
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Pencil, Check, X } from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

type ValidateFn = (value: string) => string | void;
type SaveFn = (value: string) => Promise<void> | void;

const props = withDefaults(
  defineProps<{
    text?: string | null;
    placeholder?: string;
    emptyText?: string;
    allowEmpty?: boolean;
    warning?: ValidateFn;
    validate?: ValidateFn;
    save: SaveFn;
  }>(),
  {
    text: "",
    allowEmpty: true,
    warning: undefined,
    validate: undefined,
  },
);

const { t } = useI18n();

const isEditing = ref(false);
const isSaving = ref(false);
const draft = ref("");
const inputRef = ref<InstanceType<typeof Input> | null>(null);

const normalizedText = computed(() => props.text ?? "");
const displayText = computed(
  () => normalizedText.value || props.emptyText || "-",
);
const placeholderText = computed(
  () => props.placeholder ?? t("shared.inlineCommentEditor.placeholder"),
);
const editLabel = computed(() => t("shared.inlineCommentEditor.edit"));
const saveLabel = computed(() => t("shared.inlineCommentEditor.save"));
const cancelLabel = computed(() => t("shared.inlineCommentEditor.cancel"));
const warningMessage = computed(() => props.warning?.(draft.value.trim()));

async function startEdit() {
  draft.value = normalizedText.value;
  isEditing.value = true;
  await nextTick();
  inputRef.value?.$el?.focus?.();
}

function cancelEdit() {
  isEditing.value = false;
  draft.value = "";
}

function handleKeyup(event: KeyboardEvent) {
  if (event.key === "Enter") {
    void saveEdit();
    return;
  }

  if (event.key === "Escape") {
    cancelEdit();
  }
}

async function saveEdit() {
  const nextValue = draft.value.trim();

  if (nextValue === normalizedText.value) {
    cancelEdit();
    return;
  }

  if (!props.allowEmpty && !nextValue) {
    toast.error(t("shared.inlineCommentEditor.required"));
    return;
  }

  const validationMessage = props.validate?.(nextValue);
  if (validationMessage) {
    toast.error(validationMessage);
    return;
  }

  isSaving.value = true;
  try {
    await props.save(nextValue);
    cancelEdit();
  } catch (error: any) {
    const message =
      error?.message || t("shared.inlineCommentEditor.updateFailed");
    toast.error(message);
  } finally {
    isSaving.value = false;
  }
}
</script>
