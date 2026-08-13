<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Loader2,
  Power,
  RadioTower,
  RefreshCw,
  Settings2,
} from "lucide-vue-next";
import WolManagementDialogs from "./wol-management/WolManagementDialogs.vue";
import WolRelaysTab from "./wol-management/WolRelaysTab.vue";
import WolTargetsTab from "./wol-management/WolTargetsTab.vue";
import { useWolManagementPage } from "./wol-management/useWolManagementPage";

const { t } = useI18n();
const controller = useWolManagementPage();
const { load, loadError, loading, openSettings } = controller;
</script>

<template>
  <div
    class="dynamic-white-page-card dynamic-white-settings-surface flex h-full flex-col gap-4"
  >
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="space-y-1">
        <h1 class="text-xl font-semibold tracking-tight">
          {{ t("admin.wol.title") }}
        </h1>
        <p class="text-sm leading-6 text-muted-foreground">
          {{ t("admin.wol.description") }}
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Button
          variant="outline"
          size="icon"
          :aria-label="t('admin.wol.portal.settings')"
          @click="openSettings"
        >
          <Settings2 class="h-4 w-4" />
        </Button>
        <Button variant="outline" :disabled="loading" @click="load">
          <RefreshCw :class="['mr-2 h-4 w-4', loading && 'animate-spin']" />
          {{ t("admin.wol.refresh") }}
        </Button>
      </div>
    </div>

    <div
      v-if="loading"
      class="flex flex-1 items-center justify-center py-16 text-sm text-muted-foreground"
    >
      <Loader2 class="mr-2 h-5 w-5 animate-spin" />
      {{ t("admin.wol.loading") }}
    </div>
    <div
      v-else-if="loadError"
      class="rounded-xl border border-destructive/40 bg-destructive/5 p-5"
    >
      <p class="text-sm text-destructive">{{ loadError }}</p>
      <Button class="mt-3" size="sm" variant="outline" @click="load">
        {{ t("admin.wol.retry") }}
      </Button>
    </div>

    <Tabs v-else default-value="targets" class="flex min-h-0 flex-1 flex-col">
      <TabsList class="w-fit">
        <TabsTrigger value="targets">
          <Power class="mr-1.5 h-4 w-4" />
          {{ t("admin.wol.tabs.targets") }}
        </TabsTrigger>
        <TabsTrigger value="relays">
          <RadioTower class="mr-1.5 h-4 w-4" />
          {{ t("admin.wol.tabs.relays") }}
        </TabsTrigger>
      </TabsList>
      <WolTargetsTab :controller="controller" />
      <WolRelaysTab :controller="controller" />
    </Tabs>

    <WolManagementDialogs :controller="controller" />
  </div>
</template>
