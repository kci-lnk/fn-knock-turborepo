<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  Activity,
  CalendarClock,
  ChevronDown,
  FolderInput,
  MoreHorizontal,
  Pencil,
  Power,
  PowerOff,
  Route as RouteIcon,
  ShieldOff,
  Star,
  StarOff,
  Trash2,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { TableCell } from "@/components/ui/table";
import type { HostMapping, HostMappingGroup } from "@/types";
import { isProxyHostMapping } from "./model";

withDefaults(
  defineProps<{
    asCell?: boolean;
    canUseDeepMonitor: boolean;
    compact?: boolean;
    deepMonitorActive: boolean;
    groups: HostMappingGroup[];
    isAuthServiceTarget: (target: string) => boolean;
    isDefaultDomainAvailable: boolean;
    isSavingMappings: boolean;
    mapping: HostMapping;
    triggerAriaLabel?: string;
  }>(),
  {
    asCell: true,
    compact: false,
    triggerAriaLabel: "",
  },
);

const emit = defineEmits<{
  "clear-default": [mapping: HostMapping];
  delete: [host: string];
  edit: [mapping: HostMapping];
  move: [mapping: HostMapping, groupId: string | null];
  "open-advanced-auth": [host: string];
  "open-availability": [mapping: HostMapping];
  "open-deep-monitor": [host: string];
  "open-gateway-locations": [host: string];
  "set-default": [mapping: HostMapping];
  "toggle-enabled": [mapping: HostMapping];
}>();

const { t } = useI18n();
</script>

<template>
  <component :is="asCell ? TableCell : 'div'" class="text-right">
    <div class="flex justify-end">
      <Button
        v-if="!compact"
        variant="outline"
        size="sm"
        class="rounded-r-none"
        @click="emit('edit', mapping)"
      >
        {{ t("admin.subdomainProxy.edit") }}
      </Button>
      <DropdownMenu>
        <DropdownMenuTrigger as-child>
          <Button
            :variant="compact ? 'ghost' : 'outline'"
            size="icon"
            :aria-label="triggerAriaLabel || t('common.moreActions')"
            :class="[
              'h-8 w-8',
              compact ? 'shrink-0' : 'rounded-l-none border-l-0',
            ]"
          >
            <MoreHorizontal v-if="compact" class="h-4 w-4" />
            <ChevronDown v-else class="h-4 w-4" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" class="w-44">
          <DropdownMenuItem v-if="compact" @select="emit('edit', mapping)">
            <Pencil class="mr-2 h-4 w-4" />
            {{ t("admin.subdomainProxy.edit") }}
          </DropdownMenuItem>
          <DropdownMenuSeparator v-if="compact" />
          <DropdownMenuItem
            v-if="
              isProxyHostMapping(mapping) &&
              !isAuthServiceTarget(mapping.target)
            "
            @select="emit('open-gateway-locations', mapping.host)"
          >
            <RouteIcon class="mr-2 h-4 w-4" />
            {{ t("admin.subdomainProxy.paths") }}
          </DropdownMenuItem>
          <DropdownMenuItem
            v-if="canUseDeepMonitor && !isAuthServiceTarget(mapping.target)"
            @select="emit('open-deep-monitor', mapping.host)"
          >
            <Activity
              class="mr-2 h-4 w-4"
              :class="{ 'animate-pulse text-primary': deepMonitorActive }"
            />
            {{
              deepMonitorActive
                ? t("admin.subdomainProxy.deepMonitorActive")
                : t("admin.subdomainProxy.deepMonitor")
            }}
          </DropdownMenuItem>
          <DropdownMenuItem
            v-if="!isAuthServiceTarget(mapping.target) && mapping.use_auth"
            @select="emit('open-advanced-auth', mapping.host)"
          >
            <ShieldOff class="mr-2 h-4 w-4" />
            {{ t("admin.subdomainProxy.advancedAuthConfig") }}
          </DropdownMenuItem>
          <DropdownMenuItem
            v-if="
              !isAuthServiceTarget(mapping.target) && !isDefaultDomainAvailable
            "
            disabled
          >
            <StarOff class="mr-2 h-4 w-4" />
            {{ t("admin.subdomainProxy.defaultDomainUnavailable") }}
          </DropdownMenuItem>
          <DropdownMenuItem
            v-else-if="
              !isAuthServiceTarget(mapping.target) && mapping.is_default
            "
            :disabled="isSavingMappings"
            @select="emit('clear-default', mapping)"
          >
            <StarOff class="mr-2 h-4 w-4" />
            {{ t("admin.subdomainProxy.clearDefaultDomain") }}
          </DropdownMenuItem>
          <DropdownMenuItem
            v-else-if="!isAuthServiceTarget(mapping.target)"
            :disabled="isSavingMappings"
            @select="emit('set-default', mapping)"
          >
            <Star class="mr-2 h-4 w-4" />
            {{ t("admin.subdomainProxy.setDefaultDomain") }}
          </DropdownMenuItem>
          <DropdownMenuItem
            v-if="!isAuthServiceTarget(mapping.target)"
            :disabled="isSavingMappings"
            @select="emit('toggle-enabled', mapping)"
          >
            <Power v-if="mapping.disabled" class="mr-2 h-4 w-4" />
            <PowerOff v-else class="mr-2 h-4 w-4" />
            {{
              mapping.disabled
                ? t("admin.subdomainProxy.enableMapping")
                : t("admin.subdomainProxy.disableMapping")
            }}
          </DropdownMenuItem>
          <DropdownMenuSub
            v-if="groups.length > 0 && !isAuthServiceTarget(mapping.target)"
          >
            <DropdownMenuSubTrigger :disabled="isSavingMappings">
              <FolderInput class="mr-2 h-4 w-4" />
              {{ t("admin.subdomainProxy.moveToGroup") }}
            </DropdownMenuSubTrigger>
            <DropdownMenuSubContent class="w-48">
              <DropdownMenuItem
                v-for="group in groups"
                :key="group.id"
                :disabled="mapping.group_id === group.id"
                @select="emit('move', mapping, group.id)"
              >
                {{ group.name }}
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                :disabled="!mapping.group_id"
                @select="emit('move', mapping, null)"
              >
                {{ t("admin.subdomainProxy.ungrouped") }}
              </DropdownMenuItem>
            </DropdownMenuSubContent>
          </DropdownMenuSub>
          <DropdownMenuSub v-if="!isAuthServiceTarget(mapping.target)">
            <DropdownMenuSubTrigger :disabled="isSavingMappings">
              <MoreHorizontal class="mr-2 h-4 w-4" />
              {{ t("admin.subdomainProxy.moreActions") }}
            </DropdownMenuSubTrigger>
            <DropdownMenuSubContent class="w-48">
              <DropdownMenuItem
                :disabled="isSavingMappings"
                @select="emit('open-availability', mapping)"
              >
                <CalendarClock class="mr-2 h-4 w-4" />
                {{ t("admin.subdomainProxy.scheduleAvailability") }}
              </DropdownMenuItem>
            </DropdownMenuSubContent>
          </DropdownMenuSub>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            variant="destructive"
            :disabled="isSavingMappings"
            @select="emit('delete', mapping.host)"
          >
            <Trash2 class="mr-2 h-4 w-4" />
            {{ t("admin.subdomainProxy.delete") }}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  </component>
</template>
