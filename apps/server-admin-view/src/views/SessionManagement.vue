<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import { useConfigStore } from "../store/config";
import { useSyncedQueryTab } from "@admin-shared/composables/useSyncedQueryTab";
import { docsUrls } from "../lib/docs";

const SessionsTab = defineAsyncComponent(
  () => import("./session-management/SessionsTab.vue"),
);
const IpWhitelistTab = defineAsyncComponent(() => import("./IPWhitelist.vue"));
const LoginBackoffTab = defineAsyncComponent(
  () => import("./session-management/LoginBackoffTab.vue"),
);
const IpBlacklistTab = defineAsyncComponent(
  () => import("./session-management/IpBlacklistTab.vue"),
);
const GeneralBlacklistTab = defineAsyncComponent(
  () => import("./session-management/GeneralBlacklistTab.vue"),
);

const router = useRouter();
const route = useRoute();
const configStore = useConfigStore();
const { t } = useI18n();

const showSessionsTab = computed(
  () =>
    configStore.config?.run_type === 1 || configStore.config?.run_type === 3,
);
const defaultTab = computed(() =>
  showSessionsTab.value ? "sessions" : "ip-whitelist",
);
const allowedTabs = computed(() =>
  showSessionsTab.value
    ? [
        "sessions",
        "ip-whitelist",
        "login-backoff",
        "ip-blacklist",
        "general-blacklist",
      ]
    : ["ip-whitelist", "login-backoff", "ip-blacklist", "general-blacklist"],
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
    : currentTab.value === "ip-whitelist"
      ? docsUrls.guides.whitelist
      : docsUrls.guides.security,
);
</script>

<template>
  <div
    class="dynamic-white-page-card dynamic-white-settings-surface h-full flex flex-col gap-4"
  >
    <div class="flex items-start justify-between gap-3">
      <div class="space-y-1">
        <h2 class="text-lg font-semibold tracking-tight">
          {{ t("admin.sessions.page.title") }}
        </h2>
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
      <div class="overflow-x-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden pb-1">
        <TabsList>
          <TabsTrigger v-if="showSessionsTab" value="sessions">
            {{ t("admin.sessions.page.sessionsTab") }}
          </TabsTrigger>
          <TabsTrigger value="ip-whitelist">
            {{ t("admin.sessions.page.ipWhitelistTab") }}
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
      </div>
      <TabsContent v-if="showSessionsTab" value="sessions" class="pt-2">
        <SessionsTab />
      </TabsContent>
      <TabsContent value="ip-whitelist" class="pt-2">
        <IpWhitelistTab />
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
