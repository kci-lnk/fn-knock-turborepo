<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import { Trash2 } from "lucide-vue-next";
import type { IpWhitelistPageController } from "./useIpWhitelistPage";

const props = defineProps<{ controller: IpWhitelistPageController }>();
const { t } = useI18n();
const {
  formatRegionInput,
  formatRemaining,
  isInitializing,
  regionGroupLabel,
  regionGroups,
  removeRegionGroup,
  removingRegionGroupId,
} = props.controller;
</script>

<template>
  <div
    v-if="!isInitializing && regionGroups.length > 0"
    class="mt-6 rounded-md border"
  >
    <div
      class="flex flex-wrap items-start justify-between gap-3 border-b px-4 py-3"
    >
      <div class="min-w-0 space-y-1">
        <h3 class="text-sm font-medium">
          {{ t("admin.ipWhitelist.regionGroupsTitle") }}
        </h3>
        <p class="text-sm text-muted-foreground">
          {{ t("admin.ipWhitelist.regionGroupsDescription") }}
        </p>
      </div>
      <Badge variant="secondary">
        {{
          t("admin.ipWhitelist.regionGroupsCount", {
            count: regionGroups.length,
          })
        }}
      </Badge>
    </div>

    <div class="divide-y">
      <div
        v-for="group in regionGroups"
        :key="group.id"
        class="flex flex-wrap items-start justify-between gap-4 px-4 py-4"
      >
        <div class="min-w-0 flex-1 space-y-2">
          <div class="flex flex-wrap gap-2">
            <Badge
              v-for="region in group.regions"
              :key="`${group.id}:${formatRegionInput(region)}`"
              variant="outline"
              class="max-w-full whitespace-normal text-left font-normal"
            >
              {{ formatRegionInput(region) }}
            </Badge>
          </div>
          <div
            class="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground"
          >
            <span>
              {{
                t("admin.ipWhitelist.regionGroupCidrCount", {
                  count: group.cidrCount,
                })
              }}
            </span>
            <span v-if="group.expireAt">
              {{ formatRemaining(group.expireAt) }}
            </span>
            <span v-else class="text-green-600">
              {{ t("admin.ipWhitelist.permanent") }}
            </span>
            <span>
              {{ t("admin.ipWhitelist.createdAt") }}
              <HumanFriendlyTime :value="group.createdAt * 1000" />
            </span>
          </div>
          <p v-if="group.comment" class="text-sm text-muted-foreground">
            {{ group.comment }}
          </p>
        </div>

        <ConfirmDangerPopover
          :title="t('admin.ipWhitelist.regionGroupDeleteTitle')"
          :description="
            t('admin.ipWhitelist.regionGroupDeleteDescription', {
              target: regionGroupLabel(group),
            })
          "
          :loading="removingRegionGroupId === group.id"
          :disabled="removingRegionGroupId === group.id"
          :on-confirm="() => removeRegionGroup(group.id)"
          content-class="w-64 text-left"
        >
          <template #trigger>
            <Button
              variant="ghost"
              size="icon"
              :aria-label="t('common.confirmDelete')"
              class="h-8 w-8 text-destructive hover:bg-destructive/10 hover:text-destructive"
              :disabled="removingRegionGroupId === group.id"
            >
              <Trash2 class="h-4 w-4" />
            </Button>
          </template>
        </ConfirmDangerPopover>
      </div>
    </div>
  </div>
</template>
