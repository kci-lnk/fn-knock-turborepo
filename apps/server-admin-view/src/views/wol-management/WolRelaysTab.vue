<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { TabsContent } from "@/components/ui/tabs";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import { Cable, Link2, Loader2, Pencil, Plus, Trash2 } from "lucide-vue-next";
import WOLLocalRelaySettings from "./WOLLocalRelaySettings.vue";
import type { WolManagementPageController } from "./useWolManagementPage";

const props = defineProps<{ controller: WolManagementPageController }>();
const { t } = useI18n();
const {
  deleteRelay,
  deletingRelayIds,
  localRelay,
  localRelayForm,
  openCreateRelay,
  openEditRelay,
  pairLocalRelay,
  probeRelay,
  probingRelayIds,
  relays,
  rotateRelay,
  rotatingRelayIds,
  saveLocalRelay,
  savingLocalRelay,
} = props.controller;
</script>

<template>
  <TabsContent value="relays" class="space-y-4 pt-2">
    <div class="flex items-center justify-between gap-3">
      <p class="text-sm text-muted-foreground">
        {{ t("admin.wol.relaysDescription") }}
      </p>
      <Button size="sm" @click="openCreateRelay">
        <Plus class="mr-1.5 h-4 w-4" />
        {{ t("admin.wol.addRelay") }}
      </Button>
    </div>
    <div
      v-if="!relays.length"
      class="rounded-xl border border-dashed px-5 py-12 text-center text-sm text-muted-foreground"
    >
      {{ t("admin.wol.noRelays") }}
    </div>
    <div v-else class="grid gap-3 xl:grid-cols-2">
      <Card v-for="relay in relays" :key="relay.id" class="gap-3">
        <CardHeader class="pb-0">
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0">
              <CardTitle class="truncate text-base">{{ relay.name }}</CardTitle>
              <p class="mt-1 font-mono text-xs text-muted-foreground">
                {{ relay.address
                }}<span v-if="relay.port !== 40009">:{{ relay.port }}</span>
              </p>
            </div>
            <Badge :variant="relay.enabled ? 'default' : 'secondary'">
              {{ relay.enabled ? t("admin.wol.active") : t("admin.wol.disabled") }}
            </Badge>
          </div>
        </CardHeader>
        <CardContent class="space-y-4">
          <Badge
            class="w-fit"
            :variant="relay.pskConfigured ? 'outline' : 'secondary'"
          >
            <Link2 class="mr-1 h-3 w-3" />
            {{
              relay.pskConfigured
                ? t("admin.wol.relayPaired")
                : t("admin.wol.relayWaitingForPairing")
            }}
          </Badge>
          <div class="flex flex-wrap justify-end gap-2">
            <Button variant="outline" size="sm" @click="openEditRelay(relay)">
              <Pencil class="mr-1.5 h-3.5 w-3.5" />
              {{ t("admin.wol.edit") }}
            </Button>
            <Button
              variant="outline"
              size="sm"
              :disabled="
                !relay.enabled ||
                !relay.pskConfigured ||
                probingRelayIds.has(relay.id)
              "
              @click="probeRelay(relay)"
            >
              <Loader2
                v-if="probingRelayIds.has(relay.id)"
                class="mr-1.5 h-3.5 w-3.5 animate-spin"
              />
              <Cable v-else class="mr-1.5 h-3.5 w-3.5" />
              {{ t("admin.wol.probe") }}
            </Button>
            <ConfirmDangerPopover
              :title="t('admin.wol.rotateTitle')"
              :description="t('admin.wol.rotateDescription')"
              :confirm-text="t('admin.wol.repair')"
              :loading="rotatingRelayIds.has(relay.id)"
              :on-confirm="() => rotateRelay(relay)"
            >
              <template #trigger>
                <Button variant="outline" size="sm">
                  <Link2 class="mr-1.5 h-3.5 w-3.5" />
                  {{ t("admin.wol.repair") }}
                </Button>
              </template>
            </ConfirmDangerPopover>
            <ConfirmDangerPopover
              :title="t('admin.wol.deleteRelayTitle')"
              :description="t('admin.wol.deleteRelayDescription')"
              :loading="deletingRelayIds.has(relay.id)"
              :on-confirm="() => deleteRelay(relay)"
            >
              <template #trigger>
                <Button
                  variant="outline"
                  size="sm"
                  :aria-label="t('admin.wol.deleteRelayTitle')"
                >
                  <Trash2 class="h-3.5 w-3.5 text-destructive" />
                </Button>
              </template>
            </ConfirmDangerPopover>
          </div>
        </CardContent>
      </Card>
    </div>

    <div class="border-t pt-5">
      <WOLLocalRelaySettings
        :model="localRelayForm"
        :psk-configured="localRelay?.config.pskConfigured ?? false"
        :runtime="localRelay?.runtime ?? null"
        :saving="savingLocalRelay"
        @pair="pairLocalRelay"
        @save="saveLocalRelay"
      />
    </div>
  </TabsContent>
</template>
