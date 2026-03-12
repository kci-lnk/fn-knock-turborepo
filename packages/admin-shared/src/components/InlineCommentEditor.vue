<template>
  <div class="group relative flex min-h-[24px] min-w-[100px] items-center">
    <span v-if="!isEditing" class="text-sm">
      {{ displayText }}
    </span>
    <Input
      v-else
      ref="inputRef"
      v-model="draft"
      class="h-7 w-[180px] px-2 py-1 text-sm"
      :disabled="isSaving"
      :placeholder="placeholder"
      autofocus
      @keyup.enter="saveEdit"
      @keyup.esc="cancelEdit"
    />

    <div
      v-if="!isEditing"
      class="absolute right-0 opacity-0 transition-opacity group-hover:opacity-100"
    >
      <Button variant="ghost" size="icon" class="h-6 w-6" @click="startEdit">
        <Pencil class="h-3 w-3" />
      </Button>
    </div>
    <div v-else class="ml-1 flex gap-1">
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-green-600"
        :disabled="isSaving"
        @click="saveEdit"
      >
        <Check class="h-3 w-3" />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        class="h-6 w-6 text-red-600"
        :disabled="isSaving"
        @click="cancelEdit"
      >
        <X class="h-3 w-3" />
      </Button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';
import { Pencil, Check, X } from 'lucide-vue-next';
import { toast } from '@admin-shared/utils/toast';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

type ValidateFn = (value: string) => string | void;
type SaveFn = (value: string) => Promise<void> | void;

const props = withDefaults(defineProps<{
  text?: string | null;
  placeholder?: string;
  emptyText?: string;
  allowEmpty?: boolean;
  validate?: ValidateFn;
  save: SaveFn;
}>(), {
  text: '',
  placeholder: '输入备注...',
  emptyText: '-',
  allowEmpty: true,
  validate: undefined,
});

const isEditing = ref(false);
const isSaving = ref(false);
const draft = ref('');
const inputRef = ref<InstanceType<typeof Input> | null>(null);

const normalizedText = computed(() => props.text ?? '');
const displayText = computed(() => normalizedText.value || props.emptyText);

async function startEdit() {
  draft.value = normalizedText.value;
  isEditing.value = true;
  await nextTick();
  inputRef.value?.$el?.focus?.();
}

function cancelEdit() {
  isEditing.value = false;
  draft.value = '';
}

async function saveEdit() {
  const nextValue = draft.value.trim();

  if (nextValue === normalizedText.value) {
    cancelEdit();
    return;
  }

  if (!props.allowEmpty && !nextValue) {
    toast.error('备注名称不能为空');
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
    const message = error?.message || '更新备注失败';
    toast.error(message);
  } finally {
    isSaving.value = false;
  }
}
</script>
