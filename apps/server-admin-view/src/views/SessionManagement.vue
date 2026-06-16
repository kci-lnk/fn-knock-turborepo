<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import SessionsTab from "./session-management/SessionsTab.vue";
import LoginBackoffTab from "./session-management/LoginBackoffTab.vue";
import IpBlacklistTab from "./session-management/IpBlacklistTab.vue";
import GeneralBlacklistTab from "./session-management/GeneralBlacklistTab.vue";
import { useConfigStore } from "../store/config";
import { useSyncedQueryTab } from "@admin-shared/composables/useSyncedQueryTab";
import { docsUrls } from "../lib/docs";

const router = useRouter();
const route = useRoute();
const configStore = useConfigStore();
const { t } = useI18n();

const showSessionsTab = computed(
  () =>
    configStore.config?.run_type === 1 || configStore.config?.run_type === 3,
);
const defaultTab = computed(() =>
  showSessionsTab.value ? "sessions" : "login-backoff",
);
const allowedTabs = computed(() =>
  showSessionsTab.value
    ? ["sessions", "login-backoff", "ip-blacklist", "general-blacklist"]
    : ["login-backoff", "ip-blacklist", "general-blacklist"],
);
const { currentTab, navigateTo } = useSyncedQueryTab({
  route,
  router,
  defaultTab,
  allowedTabs,
});

const currentDocsHref = computed(() =>
  currentTab.value === "sessions"
    ? docsUrls.guides.sessionManagement
    : docsUrls.guides.security,
);
</script>

<template>
  <div class="h-full flex flex-col gap-4">
    <div class="flex items-start justify-between gap-3">
      <div class="space-y-1">
        <h1 class="text-lg font-semibold tracking-tight">
          {{ t("admin.sessions.page.title") }}
        </h1>
        <p class="text-sm text-muted-foreground">
          {{ t("admin.sessions.page.description") }}
        </p>
      </div>
      <DocsLinkButton :href="currentDocsHref" />
    </div>
    <Tabs
      :model-value="currentTab"
      @update:model-value="navigateTo"
      class="w-full"
    >
      <TabsList>
        <TabsTrigger v-if="showSessionsTab" value="sessions">
          {{ t("admin.sessions.page.sessionsTab") }}
        </TabsTrigger>
        <TabsTrigger value="login-backoff">
          {{ t("admin.sessions.page.loginBackoffTab") }}
        </TabsTrigger>
        <TabsTrigger value="ip-blacklist">
          {{ t("admin.sessions.page.ipBlacklistTab") }}
        </TabsTrigger>
        <TabsTrigger value="general-blacklist">
          {{ t("admin.sessions.page.generalBlacklistTab") }}
        </TabsTrigger>
      </TabsList>
      <TabsContent v-if="showSessionsTab" value="sessions" class="pt-2">
        <SessionsTab />
      </TabsContent>
      <TabsContent value="login-backoff" class="pt-2">
        <LoginBackoffTab />
      </TabsContent>
      <TabsContent value="ip-blacklist" class="pt-2">
        <IpBlacklistTab />
      </TabsContent>
      <TabsContent value="general-blacklist" class="pt-2">
        <GeneralBlacklistTab />
      </TabsContent>
    </Tabs>
  </div>
</template>
