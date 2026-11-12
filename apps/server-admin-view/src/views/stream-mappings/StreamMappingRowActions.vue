<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  BadgeCheck,
  ChevronDown,
  Power,
  PowerOff,
  RefreshCw,
  ShieldOff,
  Trash2,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { StreamMapping } from "../../types";
import { getMappingKey, type StreamRuleAction } from "./streamMappingModel";

const props = defineProps<{
  mapping: StreamMapping;
  probingMappingKey: string | null;
  saving: boolean;
}>();

const emit = defineEmits<{
  action: [action: StreamRuleAction, mapping: StreamMapping];
  edit: [mapping: StreamMapping];
  probe: [mapping: StreamMapping];
  policy: [mapping: StreamMapping];
  service: [mapping: StreamMapping];
}>();

const { t } = useI18n();
const mappingKey = computed(() => getMappingKey(props.mapping));
const isProbing = computed(() => props.probingMappingKey === mappingKey.value);
</script>

<template>
  <div class="flex justify-end">
    <Button
      variant="outline"
      size="sm"
      :disabled="saving"
      class="rounded-r-none"
      @click="emit('edit', mapping)"
    >
      {{ t("admin.streamMappings.edit") }}
    </Button>
    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <Button
          variant="outline"
          size="icon"
          :disabled="saving"
          :aria-label="t('common.moreActions')"
          class="h-8 w-8 rounded-l-none border-l-0"
        >
          <ChevronDown class="h-4 w-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" class="w-44">
        <DropdownMenuItem
          @select="
            emit(
              'action',
              mapping.manually_disabled ? 'enable' : 'disable',
              mapping,
            )
          "
        >
          <Power v-if="mapping.manually_disabled" class="mr-2 h-4 w-4" />
          <PowerOff v-else class="mr-2 h-4 w-4" />
          {{
            mapping.manually_disabled
              ? t("admin.streamMappings.actions.enable.label")
              : t("admin.streamMappings.actions.disable.label")
          }}
        </DropdownMenuItem>
        <DropdownMenuItem
          :disabled="isProbing"
          @select="emit('probe', mapping)"
        >
          <RefreshCw
            class="mr-2 h-4 w-4"
            :class="{ 'animate-spin': isProbing }"
          />
          {{
            isProbing
              ? t("admin.streamMappings.probing")
              : t("admin.streamMappings.probe")
          }}
        </DropdownMenuItem>
        <DropdownMenuItem @select="emit('policy', mapping)">
          <ShieldOff class="mr-2 h-4 w-4" />
          {{ t("admin.streamMappings.bypassPolicy") }}
        </DropdownMenuItem>
        <DropdownMenuItem @select="emit('service', mapping)">
          <BadgeCheck class="mr-2 h-4 w-4" />
          {{ t("admin.streamMappings.selectService") }}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          variant="destructive"
          @select="emit('action', 'delete', mapping)"
        >
          <Trash2 class="mr-2 h-4 w-4" />
          {{ t("admin.streamMappings.delete") }}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  </div>
</template>
