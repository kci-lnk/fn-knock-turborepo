<template>
  <div
    :id="id"
    class="grid grid-cols-2 gap-2"
    role="group"
    :aria-label="resolvedAriaLabel"
  >
    <button
      v-for="option in protocolOptions"
      :key="option.value"
      type="button"
      :disabled="disabled"
      :aria-pressed="isSelected(option.value)"
      :class="
        cn(
          'flex h-11 items-center justify-between rounded-lg border px-4 text-left text-sm transition-all outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50',
          isSelected(option.value)
            ? 'border-primary bg-primary/10 text-primary shadow-xs ring-1 ring-primary/25'
            : 'border-input bg-background text-foreground hover:bg-accent hover:text-accent-foreground',
        )
      "
      @click="toggleProtocol(option.value)"
    >
      <span class="font-mono font-semibold uppercase tracking-[0.14em]">
        {{ option.label }}
      </span>
      <span
        :class="
          cn(
            'grid size-5 place-items-center rounded-full border transition-colors',
            isSelected(option.value)
              ? 'border-primary bg-primary text-primary-foreground'
              : 'border-muted-foreground/35 text-transparent',
          )
        "
        aria-hidden="true"
      >
        <Check class="size-3.5" />
      </span>
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Check } from "lucide-vue-next";
import { cn } from "@/lib/utils";
import type { StreamMappingProtocol } from "../types";

const protocolOptions: Array<{
  value: StreamMappingProtocol;
  label: string;
}> = [
  { value: "tcp", label: "TCP" },
  { value: "udp", label: "UDP" },
];

const protocolOrder: StreamMappingProtocol[] = protocolOptions.map(
  (option) => option.value,
);

const props = withDefaults(
  defineProps<{
    id?: string;
    disabled?: boolean;
    ariaLabel?: string;
  }>(),
  {},
);

const { t } = useI18n();

const modelValue = defineModel<StreamMappingProtocol[]>({
  default: () => ["tcp"],
});
const resolvedAriaLabel = computed(
  () => props.ariaLabel ?? t("shared.streamProtocolMultiSelect.ariaLabel"),
);

const selectedProtocols = computed(() =>
  normalizeProtocolSelection(modelValue.value),
);

function normalizeProtocolSelection(
  protocols: StreamMappingProtocol[] | undefined,
): StreamMappingProtocol[] {
  const selected = new Set(
    (protocols ?? []).filter(
      (protocol): protocol is StreamMappingProtocol =>
        protocol === "tcp" || protocol === "udp",
    ),
  );
  const normalized = protocolOrder.filter((protocol) => selected.has(protocol));
  return normalized.length > 0 ? normalized : ["tcp"];
}

function isSelected(protocol: StreamMappingProtocol): boolean {
  return selectedProtocols.value.includes(protocol);
}

function toggleProtocol(protocol: StreamMappingProtocol) {
  if (props.disabled) return;

  const next = new Set(selectedProtocols.value);
  if (next.has(protocol)) {
    if (next.size === 1) return;
    next.delete(protocol);
  } else {
    next.add(protocol);
  }

  modelValue.value = protocolOrder.filter((item) => next.has(item));
}
</script>
