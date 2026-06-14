<template>
  <div class="h-full flex flex-col gap-4">
    <div class="flex items-start justify-between gap-3">
      <div class="grid gap-1">
        <h1 class="text-lg font-semibold tracking-tight">
          {{ t("admin.sslSettings.title") }}
        </h1>
        <p class="text-sm text-muted-foreground">
          {{ t("admin.sslSettings.description") }}
        </p>
      </div>
      <DocsLinkButton :href="docsUrls.guides.ssl" />
    </div>
    <Tabs
      :model-value="currentTab"
      @update:model-value="navigateTo"
      class="w-full"
    >
      <TabsList>
        <TabsTrigger value="cert-config">{{
          t("admin.sslSettings.certConfig")
        }}</TabsTrigger>
        <TabsTrigger value="self-signed">{{
          t("admin.sslSettings.selfSigned")
        }}</TabsTrigger>
        <TabsTrigger value="acme-cert">{{
          t("admin.sslSettings.acme")
        }}</TabsTrigger>
      </TabsList>
      <TabsContent value="cert-config" class="pt-2">
        <CertConfig />
      </TabsContent>
      <TabsContent value="self-signed" class="pt-2">
        <SelfSignedCA />
      </TabsContent>
      <TabsContent value="acme-cert" class="pt-2">
        <AcmeCert />
      </TabsContent>
    </Tabs>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import CertConfig from "./ssl-settings/CertConfig.vue";
import SelfSignedCA from "./ssl-settings/SelfSignedCA.vue";
import AcmeCert from "./ssl-settings/AcmeCert.vue";
import { useSyncedQueryTab } from "@admin-shared/composables/useSyncedQueryTab";
import { docsUrls } from "../lib/docs";

const router = useRouter();
const route = useRoute();
const { t } = useI18n();

const defaultTab = "cert-config";
const allowedTabs = new Set([defaultTab, "self-signed", "acme-cert"]);
const { currentTab, navigateTo } = useSyncedQueryTab({
  route,
  router,
  defaultTab,
  allowedTabs,
});
</script>
