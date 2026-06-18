<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useProxyTargetInput } from "@admin-shared/composables/useProxyTargetInput";
import { PROXY_TARGET_PROTOCOLS } from "@admin-shared/utils/proxyTargetInput";

type Props = {
  defaultPort?: string;
  disabled?: boolean;
  hint?: string;
  inputId?: string;
  placeholder?: string;
  protocolId?: string;
};

const props = withDefaults(defineProps<Props>(), {
  disabled: false,
  inputId: "proxy-target-endpoint",
  placeholder: "127.0.0.1:8080",
  protocolId: undefined,
});

const { t } = useI18n();

const modelValue = defineModel<string>({ default: "" });

const resolvedProtocolId = computed(
  () => props.protocolId || `${props.inputId}-protocol`,
);

const { protocol, endpoint, normalize } = useProxyTargetInput(modelValue, {
  defaultPort: props.defaultPort,
});
const hintText = computed(
  () => props.hint ?? t("shared.proxyTargetInputField.hint"),
);

defineExpose({
  normalize,
});
</script>

<template>
  <div class="space-y-2">
    <div class="flex gap-2">
      <Select v-model="protocol" :disabled="disabled">
        <SelectTrigger :id="resolvedProtocolId" class="w-[116px] shrink-0">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="protocolOption in PROXY_TARGET_PROTOCOLS"
            :key="protocolOption"
            :value="protocolOption"
          >
            {{ protocolOption.toUpperCase() }}
          </SelectItem>
        </SelectContent>
      </Select>
      <Input
        :id="inputId"
        v-model="endpoint"
        :disabled="disabled"
        :placeholder="placeholder"
        class="flex-1"
        @blur="normalize"
      />
    </div>
    <p v-if="hintText" class="text-xs text-muted-foreground">
      {{ hintText }}
    </p>
  </div>
</template>
