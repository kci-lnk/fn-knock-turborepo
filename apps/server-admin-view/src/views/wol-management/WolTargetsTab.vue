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
  Power,
  Radar,
  Trash2,
} from "lucide-vue-next";
import type { WolManagementPageController } from "./useWolManagementPage";
import { canShutdownWolTarget } from "./wol-management-model";
import WolTargetTechnicalDetails from "./WolTargetTechnicalDetails.vue";

const props = defineProps<{ controller: WolManagementPageController }>();
const { t } = useI18n();
const {
  checkedAtLabel,
  deleteTarget,
  deletingTargetIds,
  openCreateTarget,
  openDiscovery,
  openEditTarget,
  openShutdownDialog,
  relays,
  statusLabel,
  targets,
  shuttingDownTargetIds,
  wakeTarget,
  wakingTargetIds,
} = props.controller;
</script>

<template>
  <TabsContent value="targets" class="space-y-4 pt-2">
    <div class="flex flex-col gap-3 sm:flex-row sm:justify-between">
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
              {{
                target.enabled ? t("admin.wol.active") : t("admin.wol.disabled")
              }}
            </Badge>
          </div>
        </CardHeader>
        <CardContent class="space-y-4">
          <WolTargetTechnicalDetails
            data-testid="wol-target-technical"
            :target="target"
            :has-relays="relays.length > 0"
            :status-label="statusLabel(target)"
            :checked-at-label="checkedAtLabel(target)"
          />
          <div
            class="grid gap-2 pt-1 sm:flex sm:items-center sm:justify-between"
          >
            <div
              class="order-2 grid grid-cols-[minmax(0,1fr)_2.75rem] gap-2 sm:order-1 sm:flex sm:flex-wrap"
            >
              <Button
                class="h-11 w-full sm:h-8 sm:w-auto"
                variant="outline"
                size="sm"
                @click="openEditTarget(target)"
              >
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
                    class="h-11 w-11 border-destructive/25 bg-destructive/5 p-0 text-destructive hover:bg-destructive/10 hover:text-destructive sm:h-8 sm:w-8"
                    :aria-label="t('admin.wol.deleteTargetTitle')"
                  >
                    <Trash2 class="h-3.5 w-3.5" />
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </div>
            <Button
              v-if="
                target.enabled &&
                target.status.state === 'online' &&
                canShutdownWolTarget(target)
              "
              variant="destructive"
              class="order-1 h-11 w-full sm:order-2 sm:h-8 sm:w-auto"
              size="sm"
              :disabled="
                wakingTargetIds.has(target.id) ||
                shuttingDownTargetIds.has(target.id)
              "
              @click="openShutdownDialog(target)"
            >
              <Loader2
                v-if="shuttingDownTargetIds.has(target.id)"
                class="mr-1.5 h-3.5 w-3.5 animate-spin"
              />
              <Power v-else class="mr-1.5 h-3.5 w-3.5" />
              {{ t("admin.wol.ssh.shutdown") }}
            </Button>
            <Button
              v-else-if="target.status.state !== 'online'"
              class="order-1 h-11 w-full sm:order-2 sm:h-8 sm:w-auto"
              size="sm"
              :disabled="
                !target.enabled ||
                (target.deliveryMode === 'relay' && !target.relay?.enabled) ||
                wakingTargetIds.has(target.id) ||
                shuttingDownTargetIds.has(target.id)
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
