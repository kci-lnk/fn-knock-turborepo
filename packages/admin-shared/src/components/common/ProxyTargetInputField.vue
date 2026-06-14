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

type Props = {
  defaultPort?: string;
  disabled?: boolean;
  hint?: string;
  inputId?: string;
  placeholder?: string;
  protocolId?: string;
};

const props = withDefaults(defineProps<Props>(), {
  defaultPort: "80",
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
const hintText = computed(() =>
  props.hint ?? t("shared.proxyTargetInputField.hint", { port: props.defaultPort }),
);

defineExpose({
  normalize,
});
</script>

<template>
  <div class="space-y-2">
    <div class="flex gap-2">
      <Select v-model="protocol" :disabled="disabled">
        <SelectTrigger :id="resolvedProtocolId" class="w-[110px] shrink-0">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="http">HTTP</SelectItem>
          <SelectItem value="https">HTTPS</SelectItem>
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
