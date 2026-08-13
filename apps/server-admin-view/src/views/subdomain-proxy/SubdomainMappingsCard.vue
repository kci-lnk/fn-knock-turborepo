<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import type { HostMappingGroup } from "@/types";
import SubdomainGroupManagerDialog from "./SubdomainGroupManagerDialog.vue";
import SubdomainMappingNotices from "./SubdomainMappingNotices.vue";
import SubdomainMappingsCardHeader from "./SubdomainMappingsCardHeader.vue";
import SubdomainMappingsTable from "./SubdomainMappingsTable.vue";
import type {
  SubdomainMappingsCardEmits,
  SubdomainMappingsCardProps,
  SubdomainMappingsTableActions,
} from "./subdomain-mappings-card-contract";

const props = defineProps<SubdomainMappingsCardProps>();
const emit = defineEmits<SubdomainMappingsCardEmits>();
const { t } = useI18n();
const isGroupManagerOpen = ref(false);
const mappingsTable = ref<{ clearSelection: () => void } | null>(null);
const searchModel = computed({
  get: () => props.searchQuery,
  set: (value: string) => emit("update:searchQuery", value),
});
const showGroupedView = computed(
  () => props.groups.length > 0 && props.groupedView,
);
const updateGroupedView = (value: boolean) => {
  emit("update-grouped-view", value);
  mappingsTable.value?.clearSelection();
};
const saveGroupsAndCloseOnSuccess = (nextGroups: HostMappingGroup[]) => {
  emit("save-groups", nextGroups, (saved) => {
    if (saved) isGroupManagerOpen.value = false;
  });
};
const tableActions: SubdomainMappingsTableActions = {
  clearDefault: (mapping) => emit("clear-default", mapping),
  copyHost: (mapping) => emit("copy-host", mapping),
  deleteMapping: (host) => emit("delete", host),
  edit: (mapping) => emit("edit", mapping),
  manageGroups: () => {
    isGroupManagerOpen.value = true;
  },
  moveMappings: (hosts, groupId) => emit("move-mappings", hosts, groupId),
  openAdvancedAuth: (host) => emit("open-advanced-auth", host),
  openAvailability: (mapping) => emit("open-availability", mapping),
  openCreate: (groupId) => emit("open-create", groupId),
  openDeepMonitor: (host) => emit("open-deep-monitor", host),
  openGatewayLocations: (host) => emit("open-gateway-locations", host),
  saveFlatOrder: (mappings) => {
    emit("update:draggableMappings", mappings);
    emit("save-order");
  },
  saveGroupedOrder: (sections) => emit("save-grouped-order", sections),
  setDefault: (mapping) => emit("set-default", mapping),
  toggleEnabled: (mapping) => emit("toggle-enabled", mapping),
};
</script>

<template>
  <Card>
    <CardHeader>
      <SubdomainMappingsCardHeader
        :all-mappings-count="allMappingsCount"
        :auth-service-mapping="authServiceMapping"
        :can-manage-new-mappings="canManageNewMappings"
        :discover-button-divider-class="discoverButtonDividerClass"
        :discover-button-variant="discoverButtonVariant"
        :docs-href="docsHref"
        :grouped-view="groupedView"
        :has-regular-host-mappings="hasRegularHostMappings"
        :is-clearing-all-subdomain-config="isClearingAllSubdomainConfig"
        :is-config-loading="isConfigLoading"
        :is-discovering="isDiscovering"
        :is-exporting-bookmarks="isExportingBookmarks"
        :is-refreshing-titles="isRefreshingTitles"
        :is-saving-mappings="isSavingMappings"
        :is-syncing="isSyncing"
        :visible-mappings-count="visibleMappingsCount"
        @add-auth-service="emit('add-auth-service')"
        @export-bookmarks="emit('export-bookmarks')"
        @manage-groups="isGroupManagerOpen = true"
        @open-clear-all-config="emit('open-clear-all-config')"
        @open-create="emit('open-create')"
        @open-discover="emit('open-discover')"
        @open-discover-settings="emit('open-discover-settings')"
        @open-stale-cleanup="emit('open-stale-cleanup')"
        @refresh-all-titles="emit('refresh-all-titles')"
        @sync-routes="emit('sync-routes')"
        @update-grouped-view="updateGroupedView"
      />
    </CardHeader>
    <CardContent class="space-y-4">
      <SearchInput
        v-model="searchModel"
        :placeholder="t('admin.subdomainProxy.searchPlaceholder')"
        class="max-w-xs"
      />
      <SubdomainMappingsTable
        ref="mappingsTable"
        :actions="tableActions"
        :model="props"
        :show-grouped-view="showGroupedView"
      >
        <template #notices>
          <SubdomainMappingNotices
            :visible-mappings-count="visibleMappingsCount"
            :root-domain-validation-message="rootDomainValidationMessage"
            :saved-root-domain="savedRootDomain"
            :root-domain-pending-save="isRootDomainPendingSave"
          />
        </template>
      </SubdomainMappingsTable>
    </CardContent>
  </Card>
  <SubdomainGroupManagerDialog
    v-model:open="isGroupManagerOpen"
    :groups="groups"
    :mappings="allRegularMappings"
    :saving="isSavingMappings"
    @save="saveGroupsAndCloseOnSuccess"
  />
</template>
