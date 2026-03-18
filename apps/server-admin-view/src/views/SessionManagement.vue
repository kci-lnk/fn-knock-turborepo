<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import SessionsTab from "./session-management/SessionsTab.vue";
import LoginBackoffTab from "./session-management/LoginBackoffTab.vue";
import IpBlacklistTab from "./session-management/IpBlacklistTab.vue";
import { useConfigStore } from "../store/config";
import { useSyncedQueryTab } from "@admin-shared/composables/useSyncedQueryTab";

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
</script>

<template>
  <div class="h-full flex flex-col gap-4">
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
