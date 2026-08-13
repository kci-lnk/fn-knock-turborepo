<script setup lang="ts">
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import SSHBlockListPanel from "./SSHBlockListPanel.vue";
import SSHLoginLogsPanel from "./SSHLoginLogsPanel.vue";
import type { SSHSecurityController } from "./ssh-security-contract";

const props = defineProps<{ controller: SSHSecurityController }>();
const { activeTab, loadDetails, setBlockListPanel, t } = props.controller;
</script>

<template>
<Tabs v-model="activeTab" class="space-y-4">
  <TabsList>
    <TabsTrigger value="login-logs">
      {{ t("admin.sshSecurity.loginLogs") }}
    </TabsTrigger>
    <TabsTrigger value="blocks">
      {{ t("admin.sshSecurity.blockList") }}
    </TabsTrigger>
  </TabsList>

  <TabsContent value="login-logs"><SSHLoginLogsPanel /></TabsContent>
  <TabsContent value="blocks">
    <SSHBlockListPanel
      :ref="setBlockListPanel"
      :reload-details="loadDetails"
    />
  </TabsContent>
</Tabs>
</template>
