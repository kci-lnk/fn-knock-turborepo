<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import { isWebSocketProxyTargetUrl } from "@admin-shared/utils/proxyTargetInput";
import { Pencil, Trash2 } from "lucide-vue-next";
import type { GatewayLocationsPageController } from "./useGatewayLocationsPage";

const props = defineProps<{ controller: GatewayLocationsPageController }>();
const { t } = useI18n();
const {
  availableMappings,
  canSave,
  draftLocations,
  formatAction,
  formatTarget,
  indexedDraftLocations,
  isDirty,
  isSaving,
  openEditDialog,
  removeLocation,
  resetDraftFromSelected,
  saveLocations,
} = props.controller;
</script>

<template>
  <div
    v-if="availableMappings.length === 0"
    class="rounded-md border px-5 py-8 text-center text-sm text-muted-foreground"
  >
    {{ t("admin.gatewayLocationsSettings.noMappings") }}
  </div>

  <div v-else class="overflow-hidden rounded-md border">
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{{ t("admin.gatewayLocationsSettings.match") }}</TableHead>
          <TableHead>{{ t("admin.gatewayLocationsSettings.path") }}</TableHead>
          <TableHead>{{ t("admin.gatewayLocationsSettings.action") }}</TableHead>
          <TableHead>
            {{ t("admin.gatewayLocationsSettings.targetResponse") }}
          </TableHead>
          <TableHead>
            {{ t("admin.gatewayLocationsSettings.processing") }}
          </TableHead>
          <TableHead class="text-right">
            {{ t("admin.gatewayLocationsSettings.actions") }}
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-if="draftLocations.length === 0">
          <TableCell colspan="6" class="py-8 text-center text-muted-foreground">
            {{ t("admin.gatewayLocationsSettings.noRules") }}
          </TableCell>
        </TableRow>
        <TableRow
          v-for="{ location, index } in indexedDraftLocations"
          :key="`${location.match}:${location.path}:${index}`"
        >
          <TableCell class="text-sm font-medium">
            {{
              location.match === "exact"
                ? t("admin.gatewayLocationsSettings.exactMatch")
                : t("admin.gatewayLocationsSettings.prefixMatch")
            }}
          </TableCell>
          <TableCell class="font-medium">{{ location.path }}</TableCell>
          <TableCell>{{ formatAction(location) }}</TableCell>
          <TableCell class="max-w-[22rem] truncate">
            {{ formatTarget(location) }}
          </TableCell>
          <TableCell class="text-xs text-muted-foreground">
            <template v-if="location.action === 'proxy'">
              {{
                location.strip_path
                  ? t("admin.gatewayLocationsSettings.stripPath")
                  : t("admin.gatewayLocationsSettings.keepPath")
              }}
              <template v-if="!isWebSocketProxyTargetUrl(location.target)">
                ·
                {{
                  location.rewrite_html
                    ? t("admin.gatewayLocationsSettings.rewriteHtml")
                    : t("admin.gatewayLocationsSettings.noRewriteHtml")
                }}
              </template>
            </template>
            <template v-else>
              {{
                t("admin.gatewayLocationsSettings.responseHeadersCount", {
                  count: Object.keys(location.response.headers || {}).length,
                })
              }}
            </template>
          </TableCell>
          <TableCell class="text-right">
            <div class="flex justify-end gap-2">
              <Button variant="ghost" size="icon" @click="openEditDialog(index)">
                <Pencil class="h-4 w-4" />
                <span class="sr-only">
                  {{ t("admin.gatewayLocationsSettings.editRuleSr") }}
                </span>
              </Button>
              <ConfirmDangerPopover
                :title="t('admin.gatewayLocationsSettings.deleteRuleTitle')"
                :description="
                  t('admin.gatewayLocationsSettings.deleteRuleDescription', {
                    path: location.path,
                  })
                "
                :confirm-text="t('admin.gatewayLocationsSettings.confirmDelete')"
                :on-confirm="() => removeLocation(index)"
                content-class="w-64 text-left"
              >
                <template #trigger>
                  <Button
                    variant="ghost"
                    size="icon"
                    class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                  >
                    <Trash2 class="h-4 w-4" />
                    <span class="sr-only">
                      {{ t("admin.gatewayLocationsSettings.deleteRuleSr") }}
                    </span>
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </div>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>

  <FloatingActionDock
    :active="isDirty"
    inline-class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end"
  >
    <template #inline>
      <Button
        variant="outline"
        :disabled="!isDirty || isSaving"
        @click="resetDraftFromSelected"
      >
        {{ t("admin.gatewayLocationsSettings.discardChanges") }}
      </Button>
      <Button :disabled="!canSave" @click="saveLocations">
        {{ t("admin.gatewayLocationsSettings.saveLocations") }}
      </Button>
    </template>
  </FloatingActionDock>
</template>
