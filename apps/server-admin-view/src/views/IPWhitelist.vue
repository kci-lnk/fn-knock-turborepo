<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import WhitelistAddDialog from "./ip-whitelist/WhitelistAddDialog.vue";
import WhitelistRecordsPanel from "./ip-whitelist/WhitelistRecordsPanel.vue";
import WhitelistRegionGroups from "./ip-whitelist/WhitelistRegionGroups.vue";
import { useIpWhitelistPage } from "./ip-whitelist/useIpWhitelistPage";

const { t } = useI18n();
const controller = useIpWhitelistPage();
const {
  addRecord,
  canSaveNewRecord,
  cidrInputMode,
  customHours,
  durationSetting,
  isRegionCidrMode,
  isSaving,
  newRecord,
  newRecordPlaceholder,
  regionInputsDisabled,
  showAddDialog,
  whitelistRegionSelections,
} = controller;
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle class="flex items-center justify-between">
        <span>{{ t("admin.ipWhitelist.title") }}</span>
        <Button @click="showAddDialog = true">
          {{ t("admin.ipWhitelist.addTarget") }}
        </Button>
      </CardTitle>
      <CardDescription>
        {{ t("admin.ipWhitelist.pageDescription") }}
      </CardDescription>
    </CardHeader>
    <CardContent>
      <WhitelistRecordsPanel :controller="controller" />
      <WhitelistRegionGroups :controller="controller" />
    </CardContent>
  </Card>

  <WhitelistAddDialog
    v-model:cidr-input-mode="cidrInputMode"
    v-model:custom-hours="customHours"
    v-model:duration-setting="durationSetting"
    v-model:new-record="newRecord"
    v-model:open="showAddDialog"
    v-model:region-selections="whitelistRegionSelections"
    :can-save="canSaveNewRecord"
    :is-region-cidr-mode="isRegionCidrMode"
    :is-saving="isSaving"
    :new-record-placeholder="newRecordPlaceholder"
    :region-inputs-disabled="regionInputsDisabled"
    @add="addRecord"
  />
</template>
