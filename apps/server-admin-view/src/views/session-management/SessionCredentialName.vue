<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useMediaQueryMatch } from "@admin-shared/composables/useMediaQueryMatch";
import type { SessionRecord } from "../../types";
import {
  formatSessionCredentialLoginDetail,
  getSessionCredentialDisplayName,
} from "./sessionCredentialPresentation";

const props = defineProps<{
  session: Pick<SessionRecord, "method" | "credentialName" | "linkedTotpName">;
}>();

const { t } = useI18n();
const open = ref(false);
const isTouchInteraction = useMediaQueryMatch(
  "(hover: none), (pointer: coarse)",
);
const translate = (key: string, params?: Record<string, string>) =>
  params ? t(key, params) : t(key);

const displayName = computed(() =>
  getSessionCredentialDisplayName(props.session),
);
const loginDetail = computed(() =>
  formatSessionCredentialLoginDetail(props.session, translate),
);
const showTooltip = computed(
  () => Boolean(loginDetail.value) && loginDetail.value !== displayName.value,
);

const handleOpenChange = (nextOpen: boolean) => {
  open.value = nextOpen;
};

const handleTriggerClick = () => {
  if (!showTooltip.value || !isTouchInteraction.value) return;
  open.value = !open.value;
};

watch(showTooltip, (visible) => {
  if (!visible) open.value = false;
});
</script>

<template>
  <span v-if="!showTooltip">{{ displayName }}</span>
  <TooltipProvider v-else>
    <Tooltip :open="open" @update:open="handleOpenChange">
      <TooltipTrigger as-child>
        <button
          type="button"
          class="inline-flex cursor-help items-center justify-center rounded-sm border-0 bg-transparent p-0 text-left font-inherit text-inherit [line-height:inherit] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
          @click="handleTriggerClick"
        >
          {{ displayName }}
        </button>
      </TooltipTrigger>
      <TooltipContent class="max-w-[min(32rem,calc(100vw-2rem))] text-left">
        <p class="break-words">{{ loginDetail }}</p>
      </TooltipContent>
    </Tooltip>
  </TooltipProvider>
</template>
