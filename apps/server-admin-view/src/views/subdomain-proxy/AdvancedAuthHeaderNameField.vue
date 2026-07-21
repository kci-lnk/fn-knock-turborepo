<script setup lang="ts">
import { ChevronDown, Check } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import {
  AutocompleteAnchor,
  AutocompleteContent,
  AutocompleteEmpty,
  AutocompleteGroup,
  AutocompleteInput,
  AutocompleteItem,
  AutocompleteItemIndicator,
  AutocompleteLabel,
  AutocompletePortal,
  AutocompleteRoot,
  AutocompleteTrigger,
  AutocompleteViewport,
} from "reka-ui";
import { advancedAuthRequestHeaderGroups } from "./advanced-auth-request-headers";

const props = defineProps<{
  id: string;
  modelValue?: string;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const { t } = useI18n();

const updateValue = (value: string) => emit("update:modelValue", value);
</script>

<template>
  <AutocompleteRoot
    :model-value="props.modelValue ?? ''"
    :disabled="props.disabled"
    open-on-focus
    open-on-click
    :reset-search-term-on-blur="false"
    @update:model-value="updateValue"
  >
    <AutocompleteAnchor class="relative w-full">
      <!--
        Let Reka own the native input and composition events. Wrapping the
        project Input here adds a second v-model that can drop rapid or IME
        input while the suggestion list is filtering.
      -->
      <AutocompleteInput
        :id="props.id"
        data-slot="input"
        class="file:text-foreground placeholder:text-muted-foreground selection:bg-primary selection:text-primary-foreground dark:bg-input/30 border-input h-9 w-full min-w-0 rounded-md border bg-transparent py-1 pr-10 pl-3 text-base shadow-xs transition-[color,box-shadow] outline-none file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive"
        :placeholder="t('admin.advancedAuth.headerNamePlaceholder')"
        :disabled="props.disabled"
        autocomplete="off"
        autocapitalize="off"
        autocorrect="off"
        :spellcheck="false"
        data-form-type="other"
        data-1p-ignore="true"
        data-lpignore="true"
        data-bwignore="true"
      />
      <AutocompleteTrigger
        class="absolute inset-y-0 right-0 flex w-10 items-center justify-center rounded-r-md text-muted-foreground outline-none transition-colors hover:text-foreground focus-visible:ring-ring/50 focus-visible:ring-[3px] disabled:pointer-events-none disabled:opacity-50"
        :aria-label="t('admin.advancedAuth.openHeaderSuggestions')"
      >
        <ChevronDown class="h-4 w-4" />
      </AutocompleteTrigger>
    </AutocompleteAnchor>

    <AutocompletePortal>
      <AutocompleteContent
        position="popper"
        align="start"
        :side-offset="4"
        class="z-50 max-h-[min(20rem,var(--reka-combobox-content-available-height))] w-[var(--reka-combobox-trigger-width)] min-w-56 overflow-hidden rounded-md border bg-popover text-popover-foreground shadow-md data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95"
      >
        <AutocompleteViewport
          class="max-h-[min(20rem,var(--reka-combobox-content-available-height))] overflow-y-auto p-1"
        >
          <AutocompleteEmpty
            class="px-3 py-6 text-center text-sm text-muted-foreground"
          >
            {{ t("admin.advancedAuth.customHeaderHint") }}
          </AutocompleteEmpty>

          <AutocompleteGroup
            v-for="group in advancedAuthRequestHeaderGroups"
            :key="group.id"
          >
            <AutocompleteLabel
              class="px-2 py-1.5 text-xs font-medium text-muted-foreground"
            >
              {{ t(group.labelKey) }}
            </AutocompleteLabel>
            <AutocompleteItem
              v-for="header in group.headers"
              :key="header"
              :value="header"
              :text-value="header"
              class="relative flex cursor-default select-none items-center rounded-sm py-1.5 pr-8 pl-2 text-sm outline-none data-[disabled]:pointer-events-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground data-[disabled]:opacity-50"
            >
              <span class="font-mono">{{ header }}</span>
              <AutocompleteItemIndicator
                class="absolute right-2 flex h-4 w-4 items-center justify-center"
              >
                <Check class="h-4 w-4" />
              </AutocompleteItemIndicator>
            </AutocompleteItem>
          </AutocompleteGroup>
        </AutocompleteViewport>
      </AutocompleteContent>
    </AutocompletePortal>
  </AutocompleteRoot>
</template>
