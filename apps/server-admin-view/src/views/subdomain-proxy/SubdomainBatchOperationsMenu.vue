<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  CalendarClock,
  Ellipsis,
  Power,
  PowerOff,
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

defineProps<{
  saving: boolean;
  selectedCount: number;
}>();

const emit = defineEmits<{
  disable: [];
  enable: [];
  schedule: [];
  delete: [];
}>();

const { t } = useI18n();
</script>

<template>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <Button
        size="sm"
        variant="outline"
        :disabled="saving || selectedCount === 0"
        class="h-10 min-w-0 w-full gap-1 px-0 text-xs sm:h-8 sm:w-auto sm:px-3 sm:text-sm"
        data-testid="batch-more-trigger"
        :title="t('common.moreActions')"
        :aria-label="t('common.moreActions')"
      >
        <span class="hidden sm:inline">{{
          t("admin.subdomainProxy.moreActions")
        }}</span>
        <Ellipsis class="size-4 shrink-0" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end">
      <DropdownMenuItem :disabled="saving" @select="emit('enable')">
        <Power class="mr-2 h-4 w-4" />{{
          t("admin.subdomainProxy.enableMapping")
        }}
      </DropdownMenuItem>
      <DropdownMenuItem :disabled="saving" @select="emit('disable')">
        <PowerOff class="mr-2 h-4 w-4" />{{
          t("admin.subdomainProxy.disableMapping")
        }}
      </DropdownMenuItem>
      <DropdownMenuItem :disabled="saving" @select="emit('schedule')">
        <CalendarClock class="mr-2 h-4 w-4" />{{
          t("admin.subdomainProxy.scheduleAvailability")
        }}
      </DropdownMenuItem>
      <DropdownMenuSeparator />
      <DropdownMenuItem
        variant="destructive"
        :disabled="saving"
        @select="emit('delete')"
      >
        <Trash2 class="mr-2 h-4 w-4" />{{ t("admin.subdomainProxy.delete") }}
      </DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
