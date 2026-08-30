<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { PanelLeftClose, PanelLeftOpen } from "lucide-vue-next";
import TerminalTargetList from "./TerminalTargetList.vue";
import type { WebTerminalPageController } from "./useWebTerminalPage";

const props = defineProps<{ controller: WebTerminalPageController }>();
const {
  deleteTarget,
  handleSessionTabChange,
  openTargetCreate,
  openTargetEdit,
  openLocalSettings,
  selectedTargetId,
  selectedSessionId,
  selectTarget,
  sessions,
  sidebarCollapsed,
  targetDrawerOpen,
  targets,
  targetsLoading,
  toggleSidebar,
} = props.controller;
const { t } = useI18n();

const addTarget = () => {
  targetDrawerOpen.value = false;
  openTargetCreate();
};

const editTarget = (target: Parameters<typeof openTargetEdit>[0]) => {
  targetDrawerOpen.value = false;
  openTargetEdit(target);
};

const removeTarget = async (target: Parameters<typeof deleteTarget>[0]) => {
  targetDrawerOpen.value = false;
  await deleteTarget(target);
};

const selectSession = async (sessionId: string) => {
  targetDrawerOpen.value = false;
  await handleSessionTabChange(sessionId);
};
</script>

<template>
  <aside
    :class="[
      'relative hidden min-h-0 shrink-0 border-r border-border/70 transition-[width] duration-200 md:block',
      sidebarCollapsed ? 'w-[68px]' : 'w-[280px]',
    ]"
  >
    <TerminalTargetList
      :collapsed="sidebarCollapsed"
      :loading="targetsLoading"
      :selected-session-id="selectedSessionId"
      :selected-target-id="selectedTargetId"
      :sessions="sessions"
      :targets="targets"
      @add="addTarget"
      @configure-local="openLocalSettings"
      @delete="removeTarget"
      @edit="editTarget"
      @select="selectTarget"
      @select-session="selectSession"
    />
    <Button
      size="icon-sm"
      variant="outline"
      class="absolute -right-3 bottom-3 z-10 h-6 w-6 rounded-full bg-background shadow-sm"
      :aria-label="
        sidebarCollapsed
          ? t('admin.webTerminal.expandTargets', 'Expand targets')
          : t('admin.webTerminal.collapseTargets', 'Collapse targets')
      "
      @click="toggleSidebar"
    >
      <PanelLeftOpen v-if="sidebarCollapsed" class="h-3 w-3" />
      <PanelLeftClose v-else class="h-3 w-3" />
    </Button>
  </aside>

  <Sheet v-model:open="targetDrawerOpen">
    <SheetContent side="left" class="w-[88vw] max-w-[340px] p-0">
      <SheetHeader class="sr-only">
        <SheetTitle>{{
          t("admin.webTerminal.targets", "Terminal targets")
        }}</SheetTitle>
        <SheetDescription>
          {{
            t(
              "admin.webTerminal.targetsDescription",
              "Manage local and SSH targets and choose a terminal session.",
            )
          }}
        </SheetDescription>
      </SheetHeader>
      <TerminalTargetList
        class="h-full"
        drawer
        :loading="targetsLoading"
        :selected-session-id="selectedSessionId"
        :selected-target-id="selectedTargetId"
        :sessions="sessions"
        :targets="targets"
        @add="addTarget"
        @configure-local="openLocalSettings"
        @delete="removeTarget"
        @edit="editTarget"
        @select="selectTarget"
        @select-session="selectSession"
      />
    </SheetContent>
  </Sheet>
</template>
