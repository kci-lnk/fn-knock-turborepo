<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { CircleAlert, Pencil } from "lucide-vue-next";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@/components/ui/popover";
import { TableCell } from "@/components/ui/table";
import type { HostMapping } from "@/types";
import { getMappingDisplayTitle } from "./model";

withDefaults(
  defineProps<{
    asCell?: boolean;
    compact?: boolean;
    deepMonitorActive: boolean;
    formatHost: (host: string) => string;
    getMappingTitleForDisplay: (mapping: HostMapping) => string;
    handleProtocolHeadersWarningOpenChange: (
      host: string,
      open: boolean,
    ) => void;
    isProtocolHeadersWarningOpen: (host: string) => boolean;
    mapping: HostMapping;
    openProtocolHeadersWarning: (host: string) => void;
    scheduleCloseProtocolHeadersWarning: (host: string) => void;
    shouldShowProtocolHeadersWarning: (mapping: HostMapping) => boolean;
    toggleProtocolHeadersWarning: (host: string) => void;
  }>(),
  {
    asCell: true,
    compact: false,
  },
);

const emit = defineEmits<{
  edit: [mapping: HostMapping];
}>();

const { t } = useI18n();
</script>

<template>
  <component
    :is="asCell ? TableCell : 'div'"
    :class="[
      'min-w-0 text-sm',
      asCell ? 'mapping-sticky-cell mapping-title-cell' : '',
    ]"
    :title="getMappingTitleForDisplay(mapping)"
  >
    <div class="flex min-w-0 items-center gap-2">
      <Popover
        v-if="shouldShowProtocolHeadersWarning(mapping)"
        :open="isProtocolHeadersWarningOpen(mapping.host)"
        @update:open="
          (nextOpen) =>
            handleProtocolHeadersWarningOpenChange(mapping.host, nextOpen)
        "
      >
        <PopoverAnchor as-child>
          <button
            type="button"
            class="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-md text-destructive transition-colors hover:bg-destructive/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-destructive/30"
            :class="{
              'bg-destructive/10': isProtocolHeadersWarningOpen(mapping.host),
            }"
            :aria-label="
              t('admin.subdomainProxy.homeAssistantWarningAria', {
                host: formatHost(mapping.host),
              })
            "
            @mouseenter="openProtocolHeadersWarning(mapping.host)"
            @mouseleave="scheduleCloseProtocolHeadersWarning(mapping.host)"
            @focus="openProtocolHeadersWarning(mapping.host)"
            @blur="scheduleCloseProtocolHeadersWarning(mapping.host)"
            @click="toggleProtocolHeadersWarning(mapping.host)"
          >
            <CircleAlert class="h-3.5 w-3.5" />
          </button>
        </PopoverAnchor>
        <PopoverContent
          side="top"
          align="start"
          class="w-72 border-destructive/20 text-left"
          @mouseenter="openProtocolHeadersWarning(mapping.host)"
          @mouseleave="scheduleCloseProtocolHeadersWarning(mapping.host)"
          @focusin="openProtocolHeadersWarning(mapping.host)"
          @focusout="scheduleCloseProtocolHeadersWarning(mapping.host)"
        >
          <div class="space-y-3">
            <div class="space-y-1">
              <div class="flex items-center gap-2">
                <CircleAlert class="h-4 w-4 text-destructive" />
                <p class="text-sm font-medium">
                  {{ t("admin.subdomainProxy.homeAssistantWarningTitle") }}
                </p>
              </div>
              <p class="text-xs leading-5 text-muted-foreground">
                {{ t("admin.subdomainProxy.homeAssistantWarningDescription") }}
              </p>
            </div>
            <a
              href="#/system/gateway-proxy-headers"
              class="inline-flex rounded-md border border-destructive/20 bg-destructive/5 px-2.5 py-1.5 text-xs font-medium text-destructive transition hover:bg-destructive/10"
            >
              {{ t("admin.subdomainProxy.goDisableProtocolHeaders") }}
            </a>
          </div>
        </PopoverContent>
      </Popover>
      <span
        v-if="deepMonitorActive"
        class="inline-flex shrink-0 items-center rounded-full bg-primary/10 text-[11px] font-medium text-primary"
        :class="compact ? 'h-5 w-5 justify-center' : 'gap-1.5 px-2 py-0.5'"
        :title="t('admin.subdomainProxy.deepMonitorActive')"
      >
        <span class="relative flex h-1.5 w-1.5">
          <span
            class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary opacity-70"
          />
          <span
            class="relative inline-flex h-1.5 w-1.5 rounded-full bg-primary"
          />
        </span>
        <span :class="{ 'sr-only': compact }">
          {{ t("admin.subdomainProxy.deepMonitorActive") }}
        </span>
      </span>
      <button
        type="button"
        data-affordance="edit"
        class="group/edit inline-flex min-w-0 flex-1 items-center gap-1.5 rounded-sm text-left text-sm transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
        :title="t('admin.subdomainProxy.edit')"
        :aria-label="
          t('admin.subdomainProxy.editMappingAria', {
            host: formatHost(mapping.host),
          })
        "
        @click="emit('edit', mapping)"
      >
        <span class="block truncate">
          {{ getMappingDisplayTitle(mapping) }}
        </span>
        <Pencil
          class="size-3.5 shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover/edit:opacity-100 group-focus-visible/edit:opacity-100 [@media(hover:none)]:opacity-100"
          aria-hidden="true"
        />
      </button>
    </div>
  </component>
</template>
