<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { TabsContent } from "@/components/ui/tabs";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import {
  ChevronDown,
  Loader2,
  MonitorUp,
  Pencil,
  Plus,
  Radar,
  Trash2,
} from "lucide-vue-next";
import type { WolManagementPageController } from "./useWolManagementPage";

const props = defineProps<{ controller: WolManagementPageController }>();
const { t } = useI18n();
const {
  checkedAtLabel,
  deleteTarget,
  deletingTargetIds,
  openCreateTarget,
  openDiscovery,
  openEditTarget,
  relays,
  statusLabel,
  targets,
  wakeTarget,
  wakingTargetIds,
} = props.controller;
</script>

<template>
  <TabsContent value="targets" class="space-y-4 pt-2">
    <div class="flex items-center justify-between gap-3">
      <p class="text-sm text-muted-foreground">
        {{ t("admin.wol.targetsDescription") }}
      </p>
      <div class="flex items-center justify-end">
        <Button size="sm" class="rounded-r-none" @click="openDiscovery">
          <Radar class="mr-1.5 h-4 w-4" />
          {{ t("admin.wol.discoverDevices") }}
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button
              data-testid="wol-device-actions-menu-trigger"
              size="sm"
              class="w-8 rounded-l-none border-l border-primary-foreground/20 px-0"
              :aria-label="t('common.moreActions')"
            >
              <ChevronDown class="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem @select="openCreateTarget">
              <Plus class="mr-2 h-4 w-4" />
              {{ t("admin.wol.addTarget") }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>

    <div
      v-if="!targets.length"
      class="rounded-xl border border-dashed px-5 py-12 text-center text-sm text-muted-foreground"
    >
      {{ t("admin.wol.noTargets") }}
    </div>
    <div v-else class="grid gap-3 xl:grid-cols-2">
      <Card
        v-for="target in targets"
        :key="target.id"
        class="gap-0 overflow-hidden"
      >
        <CardHeader class="pb-4">
          <div class="flex items-start justify-between gap-4">
            <div data-testid="wol-target-primary" class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <span
                  class="h-2.5 w-2.5 shrink-0 rounded-full"
                  :class="
                    target.status.state === 'online'
                      ? 'bg-emerald-500'
                      : target.status.state === 'offline'
                        ? 'bg-zinc-400'
                        : 'bg-amber-400'
                  "
                  aria-hidden="true"
                />
                <CardTitle class="break-words text-lg leading-6">
                  {{ target.name }}
                </CardTitle>
                <span class="sr-only">{{ statusLabel(target) }}</span>
              </div>
            </div>
            <Badge
              class="shrink-0"
              :variant="target.enabled ? 'default' : 'secondary'"
            >
              {{ target.enabled ? t("admin.wol.active") : t("admin.wol.disabled") }}
            </Badge>
          </div>
        </CardHeader>
        <CardContent class="space-y-4">
          <div
            data-testid="wol-target-technical"
            class="grid gap-2 sm:grid-cols-2"
          >
            <div class="rounded-lg bg-muted/40 px-3 py-2.5">
              <p class="text-xs text-muted-foreground">
                {{ t("admin.wol.mac") }}
              </p>
              <p class="mt-1 break-all font-mono text-sm">{{ target.mac }}</p>
            </div>
            <div class="rounded-lg bg-muted/40 px-3 py-2.5 sm:col-span-2">
              <p class="text-xs text-muted-foreground">
                {{ t("admin.wol.status.label") }}
              </p>
              <div
                class="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm"
              >
                <span>{{ statusLabel(target) }}</span>
                <span class="text-xs text-muted-foreground">
                  {{ checkedAtLabel(target) }}
                </span>
                <span
                  v-if="target.status.observedIp || target.ipAddress"
                  class="font-mono text-xs"
                >
                  {{ target.status.observedIp || target.ipAddress }}
                </span>
              </div>
            </div>
            <div
              v-if="relays.length"
              class="rounded-lg bg-muted/40 px-3 py-2.5"
            >
              <p class="text-xs text-muted-foreground">
                {{ t("admin.wol.deliveryPath") }}
              </p>
              <div class="mt-1 text-sm">
                <template v-if="target.deliveryMode === 'local'">
                  <p>{{ t("admin.wol.localDelivery") }}</p>
                  <p
                    v-if="target.broadcastAddress"
                    class="mt-0.5 break-all font-mono text-xs text-muted-foreground"
                  >
                    {{ target.broadcastAddress }}:9
                  </p>
                </template>
                <p v-else-if="target.relay">{{ target.relay.name }}</p>
                <p v-else class="text-destructive">
                  {{ t("admin.wol.relayMissing") }}
                </p>
              </div>
            </div>
          </div>

          <div
            class="flex flex-wrap items-center justify-between gap-2 border-t pt-3"
          >
            <div class="flex flex-wrap gap-2">
              <Button variant="outline" size="sm" @click="openEditTarget(target)">
                <Pencil class="mr-1.5 h-3.5 w-3.5" />
                {{ t("admin.wol.edit") }}
              </Button>
              <ConfirmDangerPopover
                :title="t('admin.wol.deleteTargetTitle')"
                :description="t('admin.wol.deleteTargetDescription')"
                :loading="deletingTargetIds.has(target.id)"
                :on-confirm="() => deleteTarget(target)"
              >
                <template #trigger>
                  <Button
                    variant="outline"
                    size="sm"
                    :aria-label="t('admin.wol.deleteTargetTitle')"
                  >
                    <Trash2 class="h-3.5 w-3.5 text-destructive" />
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </div>
            <Button
              size="sm"
              :disabled="
                !target.enabled ||
                (target.deliveryMode === 'relay' && !target.relay?.enabled) ||
                wakingTargetIds.has(target.id)
              "
              @click="wakeTarget(target)"
            >
              <Loader2
                v-if="wakingTargetIds.has(target.id)"
                class="mr-1.5 h-3.5 w-3.5 animate-spin"
              />
              <MonitorUp v-else class="mr-1.5 h-3.5 w-3.5" />
              {{ t("admin.wol.wake") }}
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  </TabsContent>
</template>
