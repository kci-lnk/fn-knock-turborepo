<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import SessionsTab from "./session-management/SessionsTab.vue";
import LoginBackoffTab from "./session-management/LoginBackoffTab.vue";
import IpBlacklistTab from "./session-management/IpBlacklistTab.vue";
import { useConfigStore } from "../store/config";
import { useSyncedQueryTab } from "@admin-shared/composables/useSyncedQueryTab";
import { docsUrls } from "../lib/docs";

const router = useRouter();
const route = useRoute();
const configStore = useConfigStore();

const showSessionsTab = computed(
  () =>
    configStore.config?.run_type === 1 || configStore.config?.run_type === 3,
);
const defaultTab = computed(() =>
  showSessionsTab.value ? "sessions" : "login-backoff",
);
const allowedTabs = computed(() =>
  showSessionsTab.value
    ? ["sessions", "login-backoff", "ip-blacklist"]
    : ["login-backoff", "ip-blacklist"],
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
        <h1 class="text-lg font-semibold tracking-tight">会话与安全</h1>
        <p class="text-sm text-muted-foreground">
          查看在线会话、异常登录退避和扫描器黑名单。
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
        <TabsTrigger v-if="showSessionsTab" value="sessions"
          >会话管理</TabsTrigger
        >
        <TabsTrigger value="login-backoff">异常登录退避</TabsTrigger>
        <TabsTrigger value="ip-blacklist">扫描器黑名单</TabsTrigger>
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
    </Tabs>
  </div>
</template>
