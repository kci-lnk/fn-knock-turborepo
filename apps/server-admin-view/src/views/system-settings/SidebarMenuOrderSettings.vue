<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { VueDraggable } from "vue-draggable-plus";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { GripVertical, RotateCcw } from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { useConfigStore } from "@/store/config";
import type { SidebarNavItemId } from "@/types";
import { useLayoutNavigation } from "../layout/useLayoutNavigation";
import {
  DEFAULT_SIDEBAR_MENU_ORDER,
  hasSameSidebarMenuOrder,
  mergeVisibleSidebarMenuOrder,
  normalizeSidebarMenuOrder,
  orderSidebarNavItems,
  type SidebarNavItem,
} from "../layout/sidebarNavigation";

const { t } = useI18n();
const configStore = useConfigStore();
const { navItems } = useLayoutNavigation();
const draggableItems = ref<SidebarNavItem[]>([]);
const savedOrder = ref<SidebarNavItemId[]>(
  normalizeSidebarMenuOrder(
    configStore.config?.dashboard_display?.sidebar_menu_order,
  ),
);
const isSaving = ref(false);
const isConfigReady = computed(
  () =>
    Boolean(configStore.config) &&
    !configStore.isLoading &&
    !configStore.isError,
);

const isDefaultOrder = computed(() =>
  hasSameSidebarMenuOrder(savedOrder.value, [...DEFAULT_SIDEBAR_MENU_ORDER]),
);

const syncVisibleItems = () => {
  draggableItems.value = orderSidebarNavItems(navItems.value, savedOrder.value);
};

watch(
  [
    () => configStore.config?.dashboard_display?.sidebar_menu_order,
    navItems,
    isConfigReady,
  ],
  ([order, , ready]) => {
    if (isSaving.value) return;
    if (!ready) {
      draggableItems.value = [];
      return;
    }
    savedOrder.value = normalizeSidebarMenuOrder(order);
    syncVisibleItems();
  },
  { immediate: true },
);

const persistOrder = async (
  nextOrder: SidebarNavItemId[],
  successKey: string,
) => {
  if (isSaving.value || !isConfigReady.value) return;
  const previousOrder = [...savedOrder.value];
  isSaving.value = true;

  try {
    const result = await configStore.saveDashboardDisplayConfig({
      sidebar_menu_order: nextOrder,
    });
    savedOrder.value = normalizeSidebarMenuOrder(result.sidebar_menu_order);
    syncVisibleItems();
    toast.success(t(successKey));
  } catch (error) {
    savedOrder.value = previousOrder;
    syncVisibleItems();
    toast.error(t("admin.sidebarMenuOrder.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sidebarMenuOrder.saveFailedDescription"),
      ),
    });
  } finally {
    isSaving.value = false;
  }
};

const saveVisibleOrder = async () => {
  if (!isConfigReady.value) return;
  const nextOrder = mergeVisibleSidebarMenuOrder({
    fullOrder: savedOrder.value,
    nextVisibleOrder: draggableItems.value.map((item) => item.id),
  });
  if (hasSameSidebarMenuOrder(nextOrder, savedOrder.value)) {
    syncVisibleItems();
    return;
  }
  await persistOrder(nextOrder, "admin.sidebarMenuOrder.saved");
};

const restoreDefaultOrder = async () => {
  if (isDefaultOrder.value || isSaving.value || !isConfigReady.value) return;
  await persistOrder(
    [...DEFAULT_SIDEBAR_MENU_ORDER],
    "admin.sidebarMenuOrder.defaultRestored",
  );
};
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">{{
            t("admin.sidebarMenuOrder.systemSettings")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=features">{{
            t("admin.sidebarMenuOrder.features")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{
            t("admin.sidebarMenuOrder.title")
          }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/50 shadow-none">
      <CardHeader>
        <div
          class="flex w-full flex-col gap-4 sm:flex-row sm:items-start sm:justify-between"
        >
          <div class="space-y-1.5">
            <CardTitle class="text-xl tracking-tight">{{
              t("admin.sidebarMenuOrder.title")
            }}</CardTitle>
            <CardDescription class="max-w-2xl leading-6">
              {{ t("admin.sidebarMenuOrder.description") }}
            </CardDescription>
          </div>
          <Button
            type="button"
            variant="outline"
            class="shrink-0"
            :disabled="isSaving || isDefaultOrder || !isConfigReady"
            @click="restoreDefaultOrder"
          >
            <RotateCcw class="mr-2 h-4 w-4" />
            {{ t("admin.sidebarMenuOrder.restoreDefault") }}
          </Button>
        </div>
      </CardHeader>

      <CardContent class="border-t pt-6">
        <div class="w-full space-y-4">
          <p class="text-sm leading-6 text-muted-foreground">
            {{ t("admin.sidebarMenuOrder.visibleOnlyHint") }}
          </p>

          <div
            v-if="!isConfigReady"
            class="rounded-xl border border-dashed px-5 py-10 text-center text-sm text-muted-foreground"
            aria-live="polite"
          >
            {{
              configStore.isError
                ? t("common.loadConfigFailed")
                : t("common.loadingConfig")
            }}
          </div>

          <VueDraggable
            v-else-if="draggableItems.length"
            v-model="draggableItems"
            class="divide-y overflow-hidden rounded-xl border bg-background"
            ghost-class="bg-muted/60"
            chosen-class="bg-muted/80"
            :animation="180"
            :disabled="isSaving || draggableItems.length < 2"
            @end="saveVisibleOrder"
          >
            <div
              v-for="item in draggableItems"
              :key="item.id"
              class="group flex cursor-grab select-none items-center gap-3 px-5 py-4 transition-colors hover:bg-muted/40 active:cursor-grabbing"
              :aria-label="
                t('admin.sidebarMenuOrder.dragAria', { name: item.name })
              "
            >
              <component
                :is="item.icon"
                class="h-4 w-4 shrink-0 text-muted-foreground"
              />
              <span class="min-w-0 flex-1 truncate text-sm font-medium">{{
                item.name
              }}</span>
              <GripVertical
                class="h-4 w-4 shrink-0 text-muted-foreground/60 transition-colors group-hover:text-muted-foreground"
              />
            </div>
          </VueDraggable>

          <div
            v-else
            class="rounded-xl border border-dashed px-5 py-10 text-center text-sm text-muted-foreground"
          >
            {{ t("admin.sidebarMenuOrder.empty") }}
          </div>

          <p class="min-h-5 text-xs text-muted-foreground" aria-live="polite">
            {{
              isSaving
                ? t("admin.sidebarMenuOrder.saving")
                : t("admin.sidebarMenuOrder.autoSaveHint")
            }}
          </p>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
