<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@/components/ui/popover";
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
  suggestions?: string[];
  suggestionsLabel?: string;
};

const props = withDefaults(defineProps<Props>(), {
  disabled: false,
  inputId: "proxy-target-endpoint",
  placeholder: "127.0.0.1:8080",
  protocolId: undefined,
  suggestions: () => [],
});

const { t } = useI18n();

const modelValue = defineModel<string>({ default: "" });
const isEndpointFocused = ref(false);
const isSuggestionFocused = ref(false);
const suggestionsContainer = ref<HTMLElement | null>(null);

const resolvedProtocolId = computed(
  () => props.protocolId || `${props.inputId}-protocol`,
);
const suggestionListId = computed(() => `${props.inputId}-suggestions`);
const suggestionItems = computed(() =>
  props.suggestions.filter((suggestion) => suggestion.length > 0),
);

const { protocol, endpoint, normalize } = useProxyTargetInput(modelValue, {
  defaultPort: props.defaultPort,
});
const hintText = computed(
  () => props.hint ?? t("shared.proxyTargetInputField.hint"),
);
const resolvedSuggestionsLabel = computed(
  () =>
    props.suggestionsLabel ?? t("shared.proxyTargetInputField.suggestionsLabel"),
);
const shouldShowSuggestions = computed(
  () =>
    !props.disabled &&
    endpoint.value === "" &&
    suggestionItems.value.length > 0 &&
    (isEndpointFocused.value || isSuggestionFocused.value),
);

const isSuggestionTarget = (target: EventTarget | null) =>
  target instanceof Node && suggestionsContainer.value?.contains(target);

const isEndpointTarget = (target: EventTarget | null) =>
  target instanceof HTMLElement && target.id === props.inputId;

const getSuggestionOptionId = (index: number) =>
  `${suggestionListId.value}-${index}`;

const focusEndpointInput = () => {
  void nextTick(() => {
    const input = document.getElementById(props.inputId);
    if (!(input instanceof HTMLInputElement)) return;

    input.focus();
    const cursorPosition = endpoint.value.length;
    input.setSelectionRange(cursorPosition, cursorPosition);
  });
};

const focusFirstSuggestion = () => {
  if (!shouldShowSuggestions.value) return;

  void nextTick(() => {
    document.getElementById(getSuggestionOptionId(0))?.focus();
  });
};

const handleEndpointArrowDown = (event: KeyboardEvent) => {
  if (!shouldShowSuggestions.value) return;

  event.preventDefault();
  focusFirstSuggestion();
};

const handleEndpointFocus = () => {
  isEndpointFocused.value = true;
};

const handleEndpointBlur = (event: FocusEvent) => {
  normalize();
  if (isSuggestionTarget(event.relatedTarget)) return;
  isEndpointFocused.value = false;
};

const handleSuggestionFocus = () => {
  isSuggestionFocused.value = true;
};

const handleSuggestionBlur = (event: FocusEvent) => {
  if (
    isEndpointTarget(event.relatedTarget) ||
    isSuggestionTarget(event.relatedTarget)
  ) {
    return;
  }

  isEndpointFocused.value = false;
  isSuggestionFocused.value = false;
};

const applySuggestion = (suggestion: string) => {
  endpoint.value = suggestion;
  isSuggestionFocused.value = false;
  isEndpointFocused.value = true;
  focusEndpointInput();
};

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
      <Popover :open="shouldShowSuggestions">
        <PopoverAnchor as-child>
          <div class="flex-1">
            <Input
              :id="inputId"
              v-model="endpoint"
              :disabled="disabled"
              :placeholder="placeholder"
              role="combobox"
              aria-autocomplete="list"
              :aria-expanded="shouldShowSuggestions"
              :aria-controls="shouldShowSuggestions ? suggestionListId : undefined"
              class="w-full"
              @focus="handleEndpointFocus"
              @blur="handleEndpointBlur"
              @keydown.down="handleEndpointArrowDown"
            />
          </div>
        </PopoverAnchor>
        <PopoverContent
          v-if="suggestionItems.length > 0"
          side="bottom"
          align="start"
          class="w-[var(--reka-popover-trigger-width)] p-1"
          @open-auto-focus.prevent
          @close-auto-focus.prevent
        >
          <div
            :id="suggestionListId"
            ref="suggestionsContainer"
            role="listbox"
            :aria-label="resolvedSuggestionsLabel"
            class="space-y-1"
          >
            <button
              v-for="(suggestion, index) in suggestionItems"
              :id="getSuggestionOptionId(index)"
              :key="suggestion"
              type="button"
              role="option"
              class="flex w-full items-center rounded-sm px-2 py-1.5 text-left text-sm font-medium outline-none transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:bg-accent focus-visible:text-accent-foreground"
              @mousedown.prevent
              @focus="handleSuggestionFocus"
              @blur="handleSuggestionBlur"
              @click="applySuggestion(suggestion)"
            >
              {{ suggestion }}
            </button>
          </div>
        </PopoverContent>
      </Popover>
    </div>
    <p v-if="hintText" class="text-xs text-muted-foreground">
      {{ hintText }}
    </p>
  </div>
</template>
