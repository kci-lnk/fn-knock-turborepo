<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { PopoverAnchor } from "reka-ui";
import { ChevronUp, HardDrive, X } from "lucide-vue-next";
import { Popover, PopoverContent } from "@/components/ui/popover";
import type { TerminalDisks } from "@/lib/api/terminal";
import { formatBytes, formatPercent } from "./terminal-metric-format";

const props = defineProps<{
  open: boolean;
  disks: TerminalDisks | null;
  loading: boolean;
  failed: boolean;
  stale: boolean;
  disabled: boolean;
}>();
const emit = defineEmits<{ "update:open": [open: boolean] }>();
const { t, locale } = useI18n();
const label = (key: string) => t(`admin.webTerminal.metrics.${key}`);
const trigger = ref<HTMLButtonElement>();
const content = ref<HTMLElement>();
let timer: ReturnType<typeof setTimeout> | undefined;
const clearTimer = () => clearTimeout(timer);
const setOpen = (open: boolean) => {
  clearTimer();
  emit("update:open", open && !props.disabled);
};
const enter = (event: PointerEvent) => {
  if (event.pointerType !== "mouse") return;
  clearTimer();
  if (!props.open) timer = setTimeout(() => setOpen(true), 150);
};
const leave = (event: PointerEvent) => {
  if (event.pointerType !== "mouse") return;
  clearTimer();
  timer = setTimeout(() => {
    const active = document.activeElement;
    if (active !== trigger.value && !content.value?.contains(active))
      setOpen(false);
  }, 200);
};
const focusOut = (event: FocusEvent) => {
  const next = event.relatedTarget;
  if (
    next instanceof Node &&
    (trigger.value?.contains(next) || content.value?.contains(next))
  )
    return;
  setOpen(false);
};
const touch = (event: PointerEvent) => {
  if (event.pointerType === "mouse") {
    event.preventDefault();
    return;
  }
  if (event.pointerType !== "touch" && event.pointerType !== "pen") return;
  // Avoid a synthesized focus/click immediately reopening or closing the panel.
  event.preventDefault();
  setOpen(!props.open);
};
watch(() => [props.disabled, props.open], clearTimer);
onBeforeUnmount(clearTimer);
</script>

<template>
  <Popover :open="open" @update:open="setOpen">
    <PopoverAnchor as-child>
      <button
        ref="trigger"
        type="button"
        class="-my-1 inline-flex min-w-0 cursor-default py-1 items-center gap-1.5 rounded-sm outline-none transition-colors hover:text-white/90 focus-visible:ring-1 focus-visible:ring-emerald-400/70 disabled:opacity-60"
        :disabled="disabled"
        :aria-label="label('disksTitle')"
        aria-haspopup="dialog"
        :aria-expanded="open"
        @pointerenter="enter"
        @pointerleave="leave"
        @pointerdown="touch"
        @focus="setOpen(true)"
        @focusout="focusOut"
        @click.prevent="
          (event: MouseEvent) => {
            if (event.detail === 0) setOpen(true);
          }
        "
      >
        <slot />
        <ChevronUp class="size-2.5 shrink-0 text-white/35" aria-hidden="true" />
      </button>
    </PopoverAnchor>
    <PopoverContent
      side="top"
      align="end"
      :side-offset="8"
      :collision-padding="12"
      class="z-[60] w-[380px] max-w-[calc(100vw-24px)] overflow-hidden border-white/10 bg-[#252527] p-0 text-white/75 shadow-xl"
      :aria-label="label('disksTitle')"
      :aria-busy="loading"
      @pointerenter="clearTimer"
      @pointerleave="leave"
      @focusout="focusOut"
      @open-auto-focus.prevent
      @close-auto-focus.prevent
    >
      <div
        ref="content"
        class="flex max-h-[min(420px,var(--reka-popover-content-available-height))] flex-col"
      >
        <div
          class="flex shrink-0 items-center justify-between border-b border-white/8 px-4 py-3"
        >
          <span class="text-xs font-medium">{{ label("disksTitle") }}</span>
          <button
            type="button"
            class="rounded p-1 text-white/40 hover:text-white focus-visible:ring-1 focus-visible:ring-emerald-400"
            :aria-label="label('closeDetails')"
            @click="setOpen(false)"
          >
            <X class="size-3.5" aria-hidden="true" />
          </button>
        </div>
        <div
          class="min-h-0 overflow-y-auto overscroll-contain px-4"
          tabindex="0"
          :aria-label="label('disksTitle')"
        >
          <p
            v-if="loading && !disks"
            class="py-5 text-xs text-white/45"
            role="status"
          >
            {{ label("loading") }}
          </p>
          <p
            v-else-if="!disks?.disks.length"
            class="py-5 text-xs text-amber-300/80"
            role="status"
          >
            {{
              disks?.reason
                ? label(`reason.${disks.reason}`)
                : label(failed ? "failed" : "disksEmpty")
            }}
          </p>
          <template v-else>
            <p
              v-if="stale || failed || disks.status === 'partial'"
              class="pt-3 text-[11px] text-amber-300/80"
              role="status"
            >
              {{ label(stale ? "stale" : failed ? "failed" : "disksPartial") }}
            </p>
            <div
              v-for="disk in disks.disks"
              :key="JSON.stringify([disk.filesystem, disk.mountPoint])"
              class="border-b border-white/8 py-3 last:border-b-0"
              data-disk-row
            >
              <div class="flex items-start gap-2">
                <HardDrive
                  class="mt-0.5 size-3.5 shrink-0 text-white/35"
                  aria-hidden="true"
                />
                <div class="min-w-0 flex-1">
                  <p class="break-all font-mono text-xs text-white/85">
                    {{ disk.mountPoint }}
                  </p>
                  <p class="mt-0.5 break-all text-[10px] text-white/40">
                    {{ disk.filesystem }}
                  </p>
                </div>
                <span
                  class="shrink-0 text-xs tabular-nums"
                  :class="
                    stale
                      ? 'text-white/35'
                      : (disk.capacity.percent ?? 0) >= 90
                        ? 'text-amber-300'
                        : 'text-emerald-400/85'
                  "
                  >{{ formatPercent(disk.capacity.percent, locale) }}</span
                >
              </div>
              <div
                class="my-2 h-1 overflow-hidden rounded-full bg-white/8"
                aria-hidden="true"
              >
                <div
                  class="h-full rounded-full"
                  :class="
                    stale
                      ? 'bg-white/25'
                      : (disk.capacity.percent ?? 0) >= 90
                        ? 'bg-amber-400/80'
                        : 'bg-emerald-400/70'
                  "
                  :style="{
                    width: `${Math.min(100, Math.max(0, disk.capacity.percent ?? 0))}%`,
                  }"
                />
              </div>
              <div
                class="flex flex-wrap justify-between gap-x-3 gap-y-1 text-[10px] tabular-nums text-white/50"
              >
                <span
                  >{{ formatBytes(disk.capacity.usedBytes, locale) }} /
                  {{ formatBytes(disk.capacity.totalBytes, locale) }}</span
                >
                <span
                  >{{ label("diskAvailable") }}
                  {{ formatBytes(disk.availableBytes, locale) }}</span
                >
              </div>
              <p
                v-if="disk.capacity.reason"
                class="mt-1 text-[10px] text-amber-300/80"
              >
                {{ label(`reason.${disk.capacity.reason}`) }}
              </p>
            </div>
          </template>
        </div>
        <p
          class="shrink-0 border-t border-white/8 px-4 py-2.5 text-[10px] leading-relaxed text-white/35"
        >
          {{ label("disksNote") }}
        </p>
      </div>
    </PopoverContent>
  </Popover>
</template>
