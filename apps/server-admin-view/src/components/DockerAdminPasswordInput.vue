<script setup lang="ts">
import { ref, watch, type HTMLAttributes } from "vue";
import { useI18n } from "vue-i18n";
import { Eye, EyeOff } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

defineOptions({
  inheritAttrs: false,
});

const props = defineProps<{
  id: string;
  modelValue: string;
  disabled?: boolean;
  inputClass?: HTMLAttributes["class"];
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const { t } = useI18n();
const isPasswordVisible = ref(false);

const updatePassword = (value: string | number) => {
  emit("update:modelValue", String(value));
};

watch(
  () => props.modelValue,
  (value) => {
    if (!value) {
      isPasswordVisible.value = false;
    }
  },
);
</script>

<template>
  <div class="relative">
    <Input
      v-bind="$attrs"
      :id="id"
      :model-value="modelValue"
      :type="isPasswordVisible ? 'text' : 'password'"
      :disabled="disabled"
      :class="['pr-10', inputClass]"
      @update:model-value="updatePassword"
    />
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      class="absolute right-1 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
      :disabled="disabled"
      :title="
        isPasswordVisible
          ? t('admin.dockerAdmin.hidePassword')
          : t('admin.dockerAdmin.showPassword')
      "
      :aria-label="
        isPasswordVisible
          ? t('admin.dockerAdmin.hidePassword')
          : t('admin.dockerAdmin.showPassword')
      "
      @click="isPasswordVisible = !isPasswordVisible"
    >
      <component
        :is="isPasswordVisible ? EyeOff : Eye"
        class="h-4 w-4"
        aria-hidden="true"
      />
    </Button>
  </div>
</template>
