<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { TriangleAlert } from "lucide-vue-next";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import RequestAnalyticsActions from "./RequestAnalyticsActions.vue";
import RequestAnalyticsBreakdowns from "./RequestAnalyticsBreakdowns.vue";
import RequestAnalyticsOverview from "./RequestAnalyticsOverview.vue";
import { useRequestAnalyticsPage } from "./useRequestAnalyticsPage";

const { t } = useI18n();
const model = useRequestAnalyticsPage();
</script>

<template>
  <div class="space-y-3 sm:space-y-4">
    <RequestAnalyticsActions :model="model" />

    <div v-if="model.loading && !model.data" class="space-y-4">
      <div
        class="grid grid-cols-2 gap-2.5 sm:gap-3 lg:grid-cols-3 xl:grid-cols-5"
      >
        <Skeleton v-for="index in 5" :key="index" class="h-32 rounded-xl" />
      </div>
      <Skeleton class="h-[360px] rounded-xl" />
      <div class="grid gap-4 xl:grid-cols-2">
        <Skeleton class="h-80 rounded-xl" />
        <Skeleton class="h-80 rounded-xl" />
      </div>
    </div>

    <Alert
      v-else-if="model.loadFailed && !model.data"
      class="flex flex-col items-start gap-3 border-destructive/30 bg-destructive/5 text-foreground sm:flex-row sm:items-center sm:justify-between"
    >
      <div class="flex items-start gap-3">
        <TriangleAlert class="mt-0.5 h-4 w-4 shrink-0 text-destructive" />
        <div>
          <p class="text-sm font-medium">
            {{ t("admin.requestAnalysis.loadFailed") }}
          </p>
          <p class="mt-1 text-xs text-muted-foreground">
            {{ t("admin.requestAnalysis.loadFailedDescription") }}
          </p>
        </div>
      </div>
      <Button type="button" variant="outline" size="sm" @click="model.refresh">
        {{ t("admin.requestAnalysis.retry") }}
      </Button>
    </Alert>

    <template v-else>
      <RequestAnalyticsOverview :model="model" />
      <RequestAnalyticsBreakdowns :model="model" />
    </template>
  </div>
</template>
