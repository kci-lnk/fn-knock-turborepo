<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Check } from "lucide-vue-next";
import type { LocaleCode } from "@fn-knock/i18n/core";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

defineProps<{
  isSaving: boolean;
  open: boolean;
  options: Array<{ label: string; value: LocaleCode }>;
  selectedLocale: LocaleCode;
}>();

const emit = defineEmits<{
  select: [value: LocaleCode];
  "update:open": [value: boolean];
}>();

const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="gap-0 overflow-hidden p-0 sm:max-w-[420px]">
      <DialogHeader class="border-b px-5 py-4 text-left">
        <DialogTitle>{{ t("locale.label") }}</DialogTitle>
      </DialogHeader>
      <div class="divide-y">
        <button
          v-for="option in options"
          :key="option.value"
          type="button"
          :class="[
            'flex h-14 w-full items-center gap-3 px-5 text-left transition-colors',
            selectedLocale === option.value
              ? 'bg-muted/90'
              : 'hover:bg-muted/55',
            isSaving ? 'cursor-not-allowed opacity-60' : '',
          ]"
          :disabled="isSaving"
          :aria-current="selectedLocale === option.value ? 'true' : undefined"
          @click="emit('select', option.value)"
        >
          <span
            :class="[
              'grid h-5 w-5 shrink-0 place-items-center rounded-full border transition-colors',
              selectedLocale === option.value
                ? 'border-emerald-500 bg-emerald-500 text-white'
                : 'border-muted-foreground/35',
            ]"
          >
            <Check v-if="selectedLocale === option.value" class="h-3.5 w-3.5" />
          </span>
          <span class="min-w-0 flex-1 truncate text-sm font-medium">
            {{ option.label }}
          </span>
          <span
            class="grid h-6 w-8 shrink-0 place-items-center overflow-hidden rounded-[5px] bg-white shadow-sm ring-1 ring-black/10"
            aria-hidden="true"
          >
            <svg
              v-if="option.value === 'zh-CN'"
              viewBox="0 0 32 24"
              class="h-6 w-8"
            >
              <defs>
                <polygon
                  id="locale-flag-cn-star"
                  points="0,-1 0.24,-0.32 0.96,-0.31 0.38,0.12 0.59,0.82 0,0.4 -0.59,0.82 -0.38,0.12 -0.96,-0.31 -0.24,-0.32"
                />
              </defs>
              <rect width="32" height="24" fill="#f23b2f" />
              <g fill="#ffde45">
                <use
                  href="#locale-flag-cn-star"
                  transform="translate(6.2 6.3) scale(3)"
                />
                <use
                  href="#locale-flag-cn-star"
                  transform="translate(12.6 3.6) scale(0.95)"
                />
                <use
                  href="#locale-flag-cn-star"
                  transform="translate(14.5 6.1) scale(0.95)"
                />
                <use
                  href="#locale-flag-cn-star"
                  transform="translate(14.2 9.2) scale(0.95)"
                />
                <use
                  href="#locale-flag-cn-star"
                  transform="translate(12.1 11.3) scale(0.95)"
                />
              </g>
            </svg>
            <svg
              v-else-if="option.value === 'zh-Hant'"
              viewBox="0 0 32 24"
              class="h-6 w-8"
            >
              <defs>
                <path
                  id="locale-flag-hk-petal"
                  d="M0,-0.65 C-1.55,-3.25 -0.25,-5.95 2.35,-6.25 C4,-4 3.05,-1.45 0.8,0.45 C0.55,0.2 0.25,-0.15 0,-0.65Z"
                />
              </defs>
              <rect width="32" height="24" fill="#f43b2f" />
              <g fill="#fff" transform="translate(16 12)">
                <use href="#locale-flag-hk-petal" transform="rotate(0)" />
                <use href="#locale-flag-hk-petal" transform="rotate(72)" />
                <use href="#locale-flag-hk-petal" transform="rotate(144)" />
                <use href="#locale-flag-hk-petal" transform="rotate(216)" />
                <use href="#locale-flag-hk-petal" transform="rotate(288)" />
                <circle r="0.85" />
              </g>
            </svg>
            <svg
              v-else-if="option.value === 'ko-KR'"
              viewBox="-72 -48 144 96"
              class="h-6 w-8"
            >
              <path fill="#fff" d="M-72 -48h144v96H-72z" />
              <g fill="none" stroke="#000" stroke-width="4">
                <path
                  transform="rotate(33.69006752598)"
                  d="M-50 -12v24m6 0v-24m6 0v24m76 0V1m0 -2v-11m6 0v11m0 2v11m6 0V1m0 -2v-11"
                />
                <path
                  transform="rotate(-33.69006752598)"
                  d="M-50 -12v24m6 0V1m0 -2v-11m6 0v24m76 0V1m0 -2v-11m6 0v24m6 0V1m0 -2v-11"
                />
              </g>
              <g transform="rotate(33.69006752598)">
                <path
                  fill="#cd2e3a"
                  d="M12 0a18 18 0 1 1 -36 0 24 24 0 1 1 48 0"
                />
                <path
                  fill="#0047a0"
                  d="M0 0a12 12 0 1 1 24 0 24 24 0 1 1 -48 0 12 12 0 1 0 24 0"
                />
              </g>
            </svg>
            <svg
              v-else-if="option.value === 'ja-JP'"
              viewBox="0 0 32 24"
              class="h-6 w-8"
            >
              <rect width="32" height="24" fill="#fff" />
              <circle cx="16" cy="12" r="5.4" fill="#bc002d" />
            </svg>
            <svg v-else viewBox="0 0 32 24" class="h-6 w-8">
              <rect width="32" height="24" fill="#f8f8f8" />
              <g fill="#d62d2d">
                <rect y="0" width="32" height="2.3" />
                <rect y="4.3" width="32" height="2.3" />
                <rect y="8.6" width="32" height="2.3" />
                <rect y="12.9" width="32" height="2.3" />
                <rect y="17.2" width="32" height="2.3" />
                <rect y="21.5" width="32" height="2.5" />
              </g>
              <rect width="14" height="12.4" fill="#4b5fb8" />
              <g fill="#fff">
                <circle cx="2.3" cy="2.1" r="0.5" />
                <circle cx="5" cy="2.1" r="0.5" />
                <circle cx="7.7" cy="2.1" r="0.5" />
                <circle cx="10.4" cy="2.1" r="0.5" />
                <circle cx="3.65" cy="4.6" r="0.5" />
                <circle cx="6.35" cy="4.6" r="0.5" />
                <circle cx="9.05" cy="4.6" r="0.5" />
                <circle cx="11.75" cy="4.6" r="0.5" />
                <circle cx="2.3" cy="7.1" r="0.5" />
                <circle cx="5" cy="7.1" r="0.5" />
                <circle cx="7.7" cy="7.1" r="0.5" />
                <circle cx="10.4" cy="7.1" r="0.5" />
                <circle cx="3.65" cy="9.6" r="0.5" />
                <circle cx="6.35" cy="9.6" r="0.5" />
                <circle cx="9.05" cy="9.6" r="0.5" />
                <circle cx="11.75" cy="9.6" r="0.5" />
              </g>
            </svg>
          </span>
        </button>
      </div>
    </DialogContent>
  </Dialog>
</template>
