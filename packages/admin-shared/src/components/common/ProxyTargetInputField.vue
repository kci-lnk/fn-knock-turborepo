<script setup lang="ts">
import { computed } from "vue";
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
  hint: "左侧选择协议，右侧填写 IP 和端口；未填写端口时会在失焦后自动补成 80。",
  inputId: "proxy-target-endpoint",
  placeholder: "127.0.0.1:8080",
  protocolId: undefined,
});

const modelValue = defineModel<string>({ default: "" });

const resolvedProtocolId = computed(
  () => props.protocolId || `${props.inputId}-protocol`,
);

const { protocol, endpoint, normalize } = useProxyTargetInput(modelValue, {
  defaultPort: props.defaultPort,
});

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
    <p v-if="hint" class="text-xs text-muted-foreground">
      {{ hint }}
    </p>
  </div>
</template>
