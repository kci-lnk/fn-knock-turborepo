<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import LogViewer from "@admin-shared/components/LogViewer.vue";

defineProps<{
  canClear: boolean;
  clearLogs: () => void | Promise<void>;
  isClearing: boolean;
  logLines: string[];
}>();

const { t } = useI18n();
</script>

<template>
  <Card class="gap-2">
    <CardHeader>
      <div class="flex items-center justify-between">
        <CardTitle class="text-base">
          {{ t("admin.ddns.logsTitle") }}
        </CardTitle>
        <div class="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            :disabled="isClearing || !canClear"
            @click="clearLogs"
          >
            <Trash2 class="h-3.5 w-3.5 mr-1" />
            {{ t("admin.ddns.clear") }}
          </Button>
        </div>
      </div>
    </CardHeader>
    <CardContent>
      <LogViewer
        :logs="logLines"
        reversed
        height-class="max-h-[400px]"
        :show-header="false"
        theme="light"
        wrap
      />
    </CardContent>
  </Card>
</template>
