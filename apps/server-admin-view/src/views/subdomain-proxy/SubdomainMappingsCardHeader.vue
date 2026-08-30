<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  ChevronDown,
  Download,
  Folders,
  Image,
  ListTree,
  ListChecks,
  Plus,
  RefreshCw,
  Search,
  ShieldCheck,
  SlidersHorizontal,
  Trash2,
} from "lucide-vue-next";
import type { ButtonVariants } from "@/components/ui/button";
import { Button } from "@/components/ui/button";
import { CardDescription, CardTitle } from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import type { HostMapping } from "@/types";
import PanelSyncMenuItem from "./PanelSyncMenuItem.vue";
import SubdomainMappingsMaintenanceMenuItems from "./SubdomainMappingsMaintenanceMenuItems.vue";

defineProps<{
  authServiceMapping: HostMapping | null;
  canManageNewMappings: boolean;
  discoverButtonDividerClass: string;
  discoverButtonVariant: ButtonVariants["variant"];
  docsHref: string;
  groupedView: boolean;
  hasRegularHostMappings: boolean;
  isClearingAllSubdomainConfig: boolean;
  isConfigLoading: boolean;
  isDiscovering: boolean;
  isExportingBookmarks: boolean;
  isRefreshingTitles: boolean;
  isSavingMappings: boolean;
  proxyMappingsCount: number;
  selectionMode?: boolean;
  isSyncing: boolean;
  visibleMappingsCount: number;
}>();

const emit = defineEmits<{
  "add-auth-service": [];
  "export-bookmarks": [];
  "manage-groups": [];
  "open-clear-all-config": [];
  "open-create": [];
  "open-discover": [];
  "open-discover-settings": [];
  "open-stale-cleanup": [];
  "open-target-optimization": [];
  "refresh-all-titles": [];
  "sync-routes": [];
  "update-selection-mode": [value: boolean];
  "update-grouped-view": [value: boolean];
}>();

const { t } = useI18n();
</script>

<template>
  <div
    class="flex flex-col items-stretch justify-between gap-4 sm:flex-row sm:items-center"
  >
    <CardTitle>{{ t("admin.subdomainProxy.mappingsTitle") }}</CardTitle>
    <div
      class="grid w-full grid-cols-3 items-center gap-2 sm:flex sm:w-auto sm:flex-wrap sm:justify-end"
    >
      <DocsLinkButton
        :href="docsHref"
        size="default"
        class="h-10 w-full justify-center gap-1.5 border bg-background px-2 shadow-sm sm:h-9 sm:w-auto sm:border-0 sm:bg-transparent sm:px-3 sm:shadow-none"
      />
      <Button
        :variant="groupedView ? 'secondary' : 'outline'"
        :disabled="isSavingMappings"
        :aria-pressed="groupedView"
        class="h-10 min-w-0 w-full justify-center px-2 sm:h-9 sm:w-auto sm:px-3"
        @click="emit('update-grouped-view', !groupedView)"
      >
        <ListTree class="h-4 w-4 shrink-0" />
        <span class="truncate">{{
          t("admin.subdomainProxy.groupedView")
        }}</span>
      </Button>
      <Button
        variant="outline"
        :disabled="isSavingMappings || !hasRegularHostMappings"
        :aria-pressed="selectionMode"
        class="h-10 min-w-0 w-full justify-center px-2 sm:h-9 sm:w-auto sm:px-3"
        @click="emit('update-selection-mode', !selectionMode)"
      >
        <ListChecks class="h-4 w-4 shrink-0" />
        <span class="truncate">{{
          t(
            selectionMode
              ? "admin.subdomainProxy.exitSelectionMode"
              : "admin.subdomainProxy.selectionMode",
          )
        }}</span>
      </Button>
      <Button
        v-if="groupedView"
        variant="outline"
        :disabled="isSavingMappings"
        class="hidden sm:inline-flex"
        @click="emit('manage-groups')"
      >
        <Folders class="mr-2 h-4 w-4" />
        {{ t("admin.subdomainProxy.manageGroups") }}
      </Button>
      <Button
        v-if="!authServiceMapping"
        :disabled="!canManageNewMappings || isSavingMappings"
        variant="default"
        class="col-span-3 w-full sm:w-auto"
        @click="emit('add-auth-service')"
      >
        <ShieldCheck class="mr-2 h-4 w-4" />
        {{ t("admin.subdomainProxy.addAuthService") }}
      </Button>
      <div
        v-if="authServiceMapping"
        class="col-span-3 flex min-w-0 w-full items-center sm:col-auto sm:w-auto"
      >
        <Button
          :variant="discoverButtonVariant"
          :disabled="!canManageNewMappings || isDiscovering || isSavingMappings"
          class="h-10 min-w-0 flex-1 rounded-r-none px-2 text-xs sm:h-9 sm:flex-none sm:px-3 sm:text-sm"
          @click="emit('open-discover')"
        >
          <Search class="h-4 w-4 shrink-0" />
          <span class="truncate">
            {{
              isDiscovering
                ? t("admin.subdomainProxy.discovering")
                : t("admin.subdomainProxy.discover")
            }}
          </span>
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button
              data-testid="subdomain-discover-menu-trigger"
              :variant="discoverButtonVariant"
              size="icon"
              :aria-label="t('common.moreActions')"
              :disabled="isSavingMappings"
              :class="[
                'h-10 w-10 rounded-l-none border-l px-1 sm:h-9 sm:w-9 sm:px-2',
                discoverButtonDividerClass,
              ]"
            >
              <ChevronDown class="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem
              v-if="groupedView"
              data-testid="mobile-manage-groups-menu-item"
              class="sm:hidden"
              :disabled="isSavingMappings"
              @select="emit('manage-groups')"
            >
              <Folders class="mr-2 h-4 w-4" />
              {{ t("admin.subdomainProxy.manageGroups") }}
            </DropdownMenuItem>
            <DropdownMenuSeparator v-if="groupedView" class="sm:hidden" />
            <DropdownMenuItem
              :disabled="isDiscovering"
              @select="emit('open-discover-settings')"
            >
              <SlidersHorizontal class="mr-2 h-4 w-4" />
              {{ t("admin.scanIntensity.title") }}
            </DropdownMenuItem>
            <DropdownMenuItem
              variant="destructive"
              :disabled="isSavingMappings || isClearingAllSubdomainConfig"
              @select="emit('open-clear-all-config')"
            >
              <Trash2 class="mr-2 h-4 w-4" />
              {{ t("admin.subdomainProxy.clearAllConfig") }}
            </DropdownMenuItem>
            <SubdomainMappingsMaintenanceMenuItems
              :clearing="isClearingAllSubdomainConfig"
              :has-mappings="proxyMappingsCount > 0"
              :saving="isSavingMappings"
              @cleanup="emit('open-stale-cleanup')"
              @optimize="emit('open-target-optimization')"
            />
            <DropdownMenuItem
              :disabled="isConfigLoading"
              @click="emit('open-create')"
            >
              <Plus class="mr-2 h-4 w-4" />
              {{ t("admin.subdomainProxy.addMapping") }}
            </DropdownMenuItem>
            <DropdownMenuItem
              :disabled="isSyncing"
              @click="emit('sync-routes')"
            >
              <RefreshCw
                class="mr-2 h-4 w-4"
                :class="{ 'animate-spin': isSyncing }"
              />
              {{
                isSyncing
                  ? t("admin.subdomainProxy.syncing")
                  : t("admin.subdomainProxy.syncRoutes")
              }}
            </DropdownMenuItem>
            <PanelSyncMenuItem />
            <DropdownMenuSeparator />
            <DropdownMenuItem
              :disabled="isRefreshingTitles || proxyMappingsCount === 0"
              @select="emit('refresh-all-titles')"
            >
              <Image
                class="mr-2 h-4 w-4"
                :class="{ 'animate-pulse': isRefreshingTitles }"
              />
              {{
                isRefreshingTitles
                  ? t("admin.subdomainProxy.refreshing")
                  : t("admin.subdomainProxy.refreshIconsTitles")
              }}
            </DropdownMenuItem>
            <DropdownMenuItem
              :disabled="isExportingBookmarks || visibleMappingsCount === 0"
              @select="emit('export-bookmarks')"
            >
              <Download
                class="mr-2 h-4 w-4"
                :class="{ 'animate-pulse': isExportingBookmarks }"
              />
              {{
                isExportingBookmarks
                  ? t("admin.subdomainProxy.exporting")
                  : t("admin.subdomainProxy.exportBookmarks")
              }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  </div>
  <CardDescription>
    {{ t("admin.subdomainProxy.mappingsDescription") }}
  </CardDescription>
</template>
