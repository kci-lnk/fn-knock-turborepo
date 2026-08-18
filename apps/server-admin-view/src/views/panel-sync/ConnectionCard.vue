<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { History, Pencil, Play, PlugZap, Trash2 } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import type { PanelConnection } from "@/lib/api/panel-sync-api";
import { composePanelEndpointUrl } from "./panel-sync-model";

const props = defineProps<{
  connection: PanelConnection;
  deleting: boolean;
  previewing: boolean;
  testing: boolean;
}>();
const emit = defineEmits<{
  delete: [];
  edit: [];
  history: [];
  preview: [];
  test: [];
  "toggle-auto": [value: boolean];
}>();
const { t, locale } = useI18n();
const verified = computed(() => Boolean(props.connection.verified_at));
const providerName = computed(
  () =>
    ({ sun_panel: "Sun-Panel", one_nav: "OneNav", van_nav: "Van Nav" })[
      props.connection.provider
    ],
);
const formatTime = (value?: string | null) =>
  value
    ? new Intl.DateTimeFormat(locale.value, {
        dateStyle: "medium",
        timeStyle: "short",
      }).format(new Date(value))
    : "--";
</script>

<template>
  <Card class="border-border/60 shadow-none">
    <CardHeader class="pb-3">
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <CardTitle class="truncate text-base">{{
            connection.name
          }}</CardTitle>
          <p class="mt-1 truncate text-sm text-muted-foreground">
            {{ providerName }} ·
            {{
              composePanelEndpointUrl(connection.base_url, connection.api_path)
            }}
          </p>
        </div>
        <Badge :variant="verified ? 'default' : 'secondary'">
          {{
            verified
              ? t("admin.panelSync.status.verified")
              : t("admin.panelSync.status.draft")
          }}
        </Badge>
      </div>
    </CardHeader>
    <CardContent class="space-y-4">
      <div class="grid gap-2 text-sm sm:grid-cols-2">
        <div>
          <span class="text-muted-foreground"
            >{{ t("admin.panelSync.lastRun") }}：</span
          >
          {{
            connection.last_run
              ? t(`admin.panelSync.runStatus.${connection.last_run.status}`)
              : "--"
          }}
        </div>
        <div>
          <span class="text-muted-foreground"
            >{{ t("admin.panelSync.nextRun") }}：</span
          >
          {{ formatTime(connection.next_sync_at) }}
        </div>
      </div>
      <div class="flex items-center justify-between rounded-lg border p-3">
        <div>
          <div class="text-sm font-medium">
            {{ t("admin.panelSync.autoSync") }}
          </div>
          <div class="text-xs text-muted-foreground">
            {{
              t("admin.panelSync.everyMinutes", {
                count: connection.auto_sync?.interval_minutes ?? 60,
              })
            }}
          </div>
        </div>
        <Switch
          :model-value="connection.auto_sync?.enabled ?? true"
          :disabled="!verified"
          :aria-label="t('admin.panelSync.autoSync')"
          @update:model-value="emit('toggle-auto', $event)"
        />
      </div>
      <div class="flex flex-wrap gap-2">
        <Button
          size="sm"
          variant="outline"
          :disabled="testing"
          @click="emit('test')"
        >
          <PlugZap class="mr-1.5 h-4 w-4" />
          {{
            testing ? t("admin.panelSync.testing") : t("admin.panelSync.test")
          }}
        </Button>
        <Button
          size="sm"
          :disabled="!verified || previewing"
          @click="emit('preview')"
        >
          <Play class="mr-1.5 h-4 w-4" />
          {{
            previewing
              ? t("admin.panelSync.previewing")
              : t("admin.panelSync.preview")
          }}
        </Button>
        <Button size="sm" variant="ghost" @click="emit('history')">
          <History class="mr-1.5 h-4 w-4" />{{ t("admin.panelSync.history") }}
        </Button>
        <Button size="sm" variant="ghost" @click="emit('edit')">
          <Pencil class="mr-1.5 h-4 w-4" />{{ t("common.edit") }}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          class="text-destructive hover:text-destructive"
          :disabled="deleting"
          @click="emit('delete')"
        >
          <Trash2 class="mr-1.5 h-4 w-4" />{{
            t("admin.panelSync.actions.delete")
          }}
        </Button>
      </div>
    </CardContent>
  </Card>
</template>
