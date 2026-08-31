<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronLeft } from "lucide-vue-next";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import SubdomainMappingBasicForm from "./SubdomainMappingBasicForm.vue";
import SubdomainMappingDialogFooter from "./SubdomainMappingDialogFooter.vue";
import SubdomainMappingIconPanel from "./SubdomainMappingIconPanel.vue";
import SubdomainMappingStaticPathBrowser from "./SubdomainMappingStaticPathBrowser.vue";
import SubdomainMappingVisibilityPanel from "./SubdomainMappingVisibilityPanel.vue";
import type {
  SubdomainMappingDialogEmits,
  SubdomainMappingDialogProps,
} from "./subdomain-mapping-dialog-contract";

const props = defineProps<SubdomainMappingDialogProps>();
const emit = defineEmits<SubdomainMappingDialogEmits>();
const { t } = useI18n();

const DIALOG_COPY_KEYS = {
  basic: ["mappingDialogTitle", "mappingDialogDescription"],
  icon: ["iconTitle", "iconPreviewDescription"],
  "path-browser": ["staticServe.browser.title", "staticServe.browser.hint"],
  visibility: ["visibilityTitle", "visibilityBehaviorDescription"],
} as const;
const dialogTitle = computed(() =>
  t(
    `admin.subdomainProxy.${DIALOG_COPY_KEYS[props.visibilityEditor.mappingDialogView][0]}`,
  ),
);
const dialogDescription = computed(() =>
  t(
    `admin.subdomainProxy.${DIALOG_COPY_KEYS[props.visibilityEditor.mappingDialogView][1]}`,
  ),
);
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="flex max-h-[85vh] flex-col gap-0 overflow-hidden overscroll-contain p-0 max-sm:!inset-x-0 max-sm:!max-w-none max-sm:!translate-x-0 max-sm:!translate-y-0"
      :class="[
        visibilityEditor.mappingDialogView === 'path-browser'
          ? 'sm:!max-w-[760px]'
          : 'sm:max-w-[520px]',
        isMappingDialogKeyboardActive
          ? 'max-sm:!bottom-auto max-sm:!top-[var(--mapping-dialog-viewport-top)] max-sm:!h-[var(--mapping-dialog-viewport-height)] max-sm:!max-h-[var(--mapping-dialog-viewport-height)] max-sm:rounded-none max-sm:border-0'
          : 'max-sm:!bottom-0 max-sm:!top-auto max-sm:!h-auto max-sm:!max-h-[var(--mapping-dialog-mobile-max-height)] max-sm:rounded-b-none max-sm:border-b-0',
      ]"
      :style="contentStyle"
      :show-close-button="false"
    >
      <DialogTitle class="sr-only">{{ dialogTitle }}</DialogTitle>
      <DialogDescription class="sr-only">{{ dialogDescription }}</DialogDescription>

      <div
        v-if="visibilityEditor.mappingDialogView !== 'basic'"
        class="shrink-0 border-b bg-background px-6 pb-3 pt-8"
      >
        <button
          type="button"
          class="-mx-2 inline-flex w-[calc(100%+1rem)] items-center gap-3 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          :aria-label="t('admin.subdomainProxy.backToBasicAria')"
          @click="
            visibilityEditor.mappingDialogView === 'path-browser'
              ? pathBrowserEditor.cancel()
              : visibilityEditor.returnBasicView()
          "
        >
          <ChevronLeft class="h-4 w-4 shrink-0" />
          <span class="text-sm font-semibold">
            {{ dialogTitle }}
          </span>
        </button>
      </div>

      <div
        :ref="setScrollElement"
        class="relative min-h-0 flex-1 overflow-x-hidden overflow-y-auto overscroll-contain px-6 [overflow-anchor:none]"
        :style="scrollStyle"
        @focusin="handleFocusIn"
        @focusout="handleFocusOut"
      >
        <Transition
          enter-active-class="motion-safe:transition-[opacity,transform] motion-safe:duration-200 motion-safe:ease-out motion-safe:will-change-transform motion-reduce:transition-none"
          leave-active-class="absolute inset-x-6 top-0 motion-safe:transition-[opacity,transform] motion-safe:duration-200 motion-safe:ease-out motion-safe:will-change-transform motion-reduce:hidden"
          :enter-from-class="visibilityEditor.transitionEnterFromClass"
          enter-to-class="translate-x-0 opacity-100"
          leave-from-class="translate-x-0 opacity-100"
          :leave-to-class="visibilityEditor.transitionLeaveToClass"
        >
          <SubdomainMappingBasicForm
            v-if="visibilityEditor.mappingDialogView === 'basic'"
            key="mapping-basic"
            :dialog="props"
          />
          <SubdomainMappingIconPanel
            v-else-if="visibilityEditor.mappingDialogView === 'icon'"
            key="mapping-icon"
            :icon-editor="iconEditor"
            :is-saving-mappings="isSavingMappings"
          />
          <SubdomainMappingVisibilityPanel
            v-else-if="visibilityEditor.mappingDialogView === 'visibility'"
            key="mapping-visibility"
            :composed-preview-host="composedPreviewHost"
            :mapping-form="mappingForm"
            :visibility-editor="visibilityEditor"
          />
          <SubdomainMappingStaticPathBrowser
            v-else
            key="mapping-path-browser"
            :editor="pathBrowserEditor"
          />
        </Transition>
      </div>

      <SubdomainMappingDialogFooter
        :dialog="props"
        @close="emit('close')"
        @save="emit('save')"
      />
    </DialogContent>
  </Dialog>
</template>
