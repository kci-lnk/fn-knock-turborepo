<script setup lang="ts">
import { computed, defineAsyncComponent, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  Globe2,
  Languages,
  LogOut,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { ThemeModeToggle } from "@/components/ui/theme-toggle";
import {
  Tooltip,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { OFFICIAL_WEBSITE_URL } from "@/lib/update-presentation";
import LayoutScrollArea from "./LayoutScrollArea.vue";
import NavAlertDot from "./NavigationAlertDot.vue";
import { ConfirmDangerPopover } from "./asyncComponents";
import type { SidebarNavItem } from "./sidebarNavigation";
import { useSidebarCollapse } from "./useSidebarCollapse";

const TooltipContent = defineAsyncComponent(
  () => import("@/components/ui/tooltip/TooltipContent.vue"),
);

defineProps<{
  navItems: SidebarNavItem[];
  currentVersionLabel: string;
  isNavActive: (path: string) => boolean;
  isSidebarMenuOrderMode: boolean;
  shouldShowPanelLogout: boolean;
  isLogoutSubmitting: boolean;
  navigateTo: (path: string) => Promise<void>;
  onPanelLogout: () => Promise<void>;
  onOpenLocale: (event: MouseEvent) => void;
}>();

const { t } = useI18n();
const { collapsed, canToggle, isSaving, toggle } = useSidebarCollapse();
const toggleLabel = computed(() =>
  t(collapsed.value ? "admin.nav.expandSidebar" : "admin.nav.collapseSidebar"),
);
const openMenuTooltipPath = ref<string | null>(null);
const setMenuTooltipOpen = (path: string, open: boolean) => {
  if (open && collapsed.value) openMenuTooltipPath.value = path;
  else if (openMenuTooltipPath.value === path) openMenuTooltipPath.value = null;
};
watch(collapsed, (next) => {
  if (!next) openMenuTooltipPath.value = null;
});
</script>

<template>
  <aside
    :class="[
      'hidden shrink-0 sm:sticky sm:top-6 sm:block sm:h-[calc(100dvh-3rem)]',
      collapsed ? 'sm:w-16' : 'sm:w-36 md:w-[9.25rem] xl:w-[9.5rem]',
    ]"
    :data-collapsed="collapsed"
  >
    <TooltipProvider>
      <div class="flex h-full min-h-0 flex-col gap-3">
        <LayoutScrollArea
          id="desktop-sidebar-menu"
          :reserve-rail-gutter="!collapsed"
          class="min-h-0 flex-1"
          :content-class="
            collapsed
              ? 'flex min-h-full flex-col items-center gap-1.5'
              : 'flex min-h-full flex-col items-stretch gap-1.5'
          "
          :class="{ 'sidebar-menu-editing': isSidebarMenuOrderMode }"
        >
          <Tooltip
            v-for="item in navItems"
            :key="item.path"
            :disabled="!collapsed"
            :open="collapsed && openMenuTooltipPath === item.path"
            @update:open="setMenuTooltipOpen(item.path, $event)"
          >
            <TooltipTrigger as-child>
              <Button
                :variant="isNavActive(item.path) ? 'default' : 'ghost'"
                :aria-label="
                  item.alert ? `${item.name}; ${item.alert}` : item.name
                "
                :aria-current="isNavActive(item.path) ? 'page' : undefined"
                :class="[
                  'relative min-w-0 overflow-hidden select-none [-webkit-user-select:none] [-webkit-touch-callout:none] transition-[transform,box-shadow,background-color,color] duration-150',
                  collapsed
                    ? 'h-10 w-10 justify-center p-0'
                    : 'w-full justify-start gap-2 px-2.5',
                  isNavActive(item.path)
                    ? 'shadow-sm shadow-primary/15'
                    : 'hover:-translate-y-[1px]',
                ]"
                @click="navigateTo(item.path)"
              >
                <component :is="item.icon" class="h-4 w-4 shrink-0" />
                <span v-if="!collapsed" class="min-w-0 truncate">{{
                  item.name
                }}</span>
                <NavAlertDot
                  :label="item.alert"
                  :class="collapsed ? 'absolute right-1 top-1' : 'ml-auto'"
                />
              </Button>
            </TooltipTrigger>
            <TooltipContent
              v-if="collapsed && openMenuTooltipPath === item.path"
              side="right"
            >
              <p>{{ item.name }}</p>
              <p v-if="item.alert">{{ item.alert }}</p>
            </TooltipContent>
          </Tooltip>
        </LayoutScrollArea>
        <div class="shrink-0">
          <div
            class="mb-5 flex items-center justify-center gap-2"
            :class="{ 'flex-col': collapsed }"
          >
            <ThemeModeToggle />
            <Tooltip v-slot="{ open }">
              <TooltipTrigger as-child>
                <Button
                  variant="ghost"
                  size="sm"
                  class="h-8 w-8 shrink-0 rounded-md border border-border/60 bg-background/70 p-0 text-xs shadow-none hover:bg-muted"
                  :aria-label="t('locale.label')"
                  @click="onOpenLocale"
                >
                  <Languages class="h-3.5 w-3.5 shrink-0" />
                </Button>
              </TooltipTrigger>
              <TooltipContent v-if="open" side="right">{{
                t("locale.label")
              }}</TooltipContent>
            </Tooltip>
            <ConfirmDangerPopover
              v-if="shouldShowPanelLogout"
              :title="t('admin.dockerAdmin.logoutConfirmTitle')"
              :description="t('admin.dockerAdmin.logoutConfirmDescription')"
              :confirm-text="t('admin.dockerAdmin.logoutConfirm')"
              :loading="isLogoutSubmitting"
              :disabled="isLogoutSubmitting"
              :on-confirm="onPanelLogout"
              content-class="w-64 text-left"
            >
              <template #trigger>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  class="h-8 w-8 shrink-0 rounded-md border border-destructive/20 bg-destructive/5 p-0 text-destructive shadow-none hover:bg-destructive/10 hover:text-destructive"
                  :disabled="isLogoutSubmitting"
                  :title="t('admin.dockerAdmin.logout')"
                  :aria-label="t('admin.dockerAdmin.logout')"
                >
                  <LogOut class="h-3.5 w-3.5" />
                </Button>
              </template>
            </ConfirmDangerPopover>
          </div>
          <p class="min-w-0 text-center text-xs font-medium text-primary/70">
            <Tooltip v-slot="{ open }">
              <TooltipTrigger as-child>
                <a
                  :href="OFFICIAL_WEBSITE_URL"
                  target="_blank"
                  rel="noopener noreferrer"
                  :class="[
                    'inline-flex max-w-full items-center justify-center gap-1.5 rounded-full leading-none transition-colors hover:text-foreground hover:bg-background/70',
                    collapsed ? 'h-8 w-8' : 'px-2.5 py-1',
                  ]"
                  :aria-label="t('admin.nav.officialWebsite')"
                >
                  <Globe2 class="h-3.5 w-3.5" />
                  <span v-if="!collapsed">{{ currentVersionLabel }}</span>
                </a>
              </TooltipTrigger>
              <TooltipContent v-if="open" side="right">
                {{ t("admin.nav.officialWebsite") }} {{ currentVersionLabel }}
              </TooltipContent>
            </Tooltip>
          </p>
        </div>
        <div class="flex shrink-0 justify-center">
          <Tooltip v-slot="{ open }">
            <TooltipTrigger as-child>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                class="h-9 w-9"
                :aria-label="toggleLabel"
                :aria-expanded="!collapsed"
                aria-controls="desktop-sidebar-menu"
                :aria-busy="isSaving"
                :disabled="!canToggle"
                @click="toggle"
              >
                <PanelLeftOpen v-if="collapsed" class="h-4 w-4" />
                <PanelLeftClose v-else class="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent v-if="open" side="right">{{
              toggleLabel
            }}</TooltipContent>
          </Tooltip>
        </div>
      </div>
    </TooltipProvider>
  </aside>
</template>
