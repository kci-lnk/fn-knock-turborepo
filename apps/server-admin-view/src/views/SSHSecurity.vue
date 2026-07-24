<script setup lang="ts">
import { useId } from "vue";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { ChevronDown, Loader2, RefreshCw, Save, Trash2 } from "lucide-vue-next";
import CidrRegionSelector from "@/components/CidrRegionSelector.vue";
import SSHBlockListPanel from "./ssh-security/SSHBlockListPanel.vue";
import SSHLoginLogsPanel from "./ssh-security/SSHLoginLogsPanel.vue";
import { useSSHSecurityConfig } from "./ssh-security/useSSHSecurityConfig";

const a11yId = useId();

const {
  activeTab,
  clearFirewall,
  customCidrsState,
  details,
  form,
  invalidCustomCidrs,
  isClearFirewallDialogOpen,
  isLoading,
  isSaving,
  isSyncingFirewall,
  loadDetails,
  openClearFirewallDialog,
  regionInputsDisabled,
  saveBlockedReason,
  saveConfig,
  setBlockListPanel,
  summaryText,
  syncFirewall,
  t,
} = useSSHSecurityConfig();
</script>

<template>
  <div class="space-y-4">
    <ConfigCollapsibleCard
      :title="t('admin.sshSecurity.title')"
      :configured="details?.summary.configured === true"
      :ready="details !== null && !isLoading"
      :edit-label="t('admin.sshSecurity.editConfig')"
      summary-class="text-xs text-muted-foreground"
      expanded-content-class="p-0 sm:p-0"
      actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col-reverse items-stretch gap-2 rounded-b-lg sm:flex-row sm:items-center sm:justify-end"
    >
      <template #summary>{{ summaryText }}</template>

      <template #collapsed-actions>
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button
              variant="outline"
              class="w-24 gap-2"
              :disabled="
                isSaving ||
                isSyncingFirewall ||
                !details ||
                !details.summary.available
              "
            >
              <Loader2 v-if="isSyncingFirewall" class="h-4 w-4 animate-spin" />
              <span>{{ t("admin.sshSecurity.actions") }}</span>
              <ChevronDown class="h-4 w-4 text-muted-foreground" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-56">
            <DropdownMenuItem
              :disabled="
                isSaving ||
                isSyncingFirewall ||
                !details ||
                !details.summary.available
              "
              @select="syncFirewall"
            >
              <RefreshCw class="h-4 w-4" />
              {{ t("admin.sshSecurity.syncFirewall") }}
            </DropdownMenuItem>
            <DropdownMenuItem
              class="text-destructive focus:text-destructive"
              :disabled="
                isSaving ||
                isSyncingFirewall ||
                !details ||
                !details.summary.available
              "
              @select="openClearFirewallDialog"
            >
              <Trash2 class="h-4 w-4" />
              {{ t("admin.sshSecurity.clearSshFirewall") }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </template>

      <template #default>
        <div class="divide-y divide-border">
          <div v-if="details && !details.summary.available" class="p-4 sm:p-6">
            <div
              class="rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-900"
            >
              {{ details.summary.unavailable_reason }}
            </div>
          </div>

          <div
            class="grid gap-3 p-4 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
          >
            <div class="space-y-1">
              <Label
                :for="`${a11yId}-sshsecurity-1`"
                class="text-sm font-medium"
              >
                {{ t("admin.sshSecurity.enableSshSecurity") }}
              </Label>
              <p
                class="hidden pr-4 text-xs leading-5 text-muted-foreground sm:block"
              >
                {{ t("admin.sshSecurity.enableDescription") }}
              </p>
            </div>
            <div class="flex items-start justify-between gap-4">
              <p class="text-sm leading-6 text-muted-foreground sm:hidden">
                {{ t("admin.sshSecurity.enableDescription") }}
              </p>
              <Switch
                :id="`${a11yId}-sshsecurity-1`"
                v-model="form.enabled"
                class="mt-0.5 shrink-0"
                :disabled="
                  isSaving || (details !== null && !details.summary.available)
                "
              />
            </div>
          </div>

          <div
            class="grid gap-3 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
          >
            <div class="space-y-1">
              <Label for="ssh-window-minutes" class="text-sm font-medium">
                {{ t("admin.sshSecurity.windowTime") }}
              </Label>
              <p
                class="hidden pr-4 text-xs leading-5 text-muted-foreground sm:block"
              >
                {{ t("admin.sshSecurity.windowDescription") }}
              </p>
            </div>
            <div class="w-full max-w-xs space-y-2">
              <Input
                id="ssh-window-minutes"
                v-model.number="form.windowMinutes"
                type="number"
                min="1"
                max="1440"
                :disabled="isSaving"
              />
              <p class="text-[11px] text-muted-foreground">
                {{ t("admin.sshSecurity.unitMinutes") }}
              </p>
            </div>
          </div>

          <div
            class="grid gap-3 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
          >
            <div class="space-y-1">
              <Label for="ssh-failure-threshold" class="text-sm font-medium">
                {{ t("admin.sshSecurity.failureThreshold") }}
              </Label>
              <p
                class="hidden pr-4 text-xs leading-5 text-muted-foreground sm:block"
              >
                {{ t("admin.sshSecurity.failureThresholdDescription") }}
              </p>
            </div>
            <div class="w-full max-w-xs">
              <Input
                id="ssh-failure-threshold"
                v-model.number="form.failedLoginThreshold"
                type="number"
                min="1"
                max="1000"
                :disabled="isSaving"
              />
            </div>
          </div>

          <div
            class="grid gap-3 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
          >
            <div class="space-y-1">
              <Label for="ssh-block-duration" class="text-sm font-medium">
                {{ t("admin.sshSecurity.blockDuration") }}
              </Label>
              <p
                class="hidden pr-4 text-xs leading-5 text-muted-foreground sm:block"
              >
                {{ t("admin.sshSecurity.blockDurationDescription") }}
              </p>
            </div>
            <div
              class="grid w-full max-w-md grid-cols-[minmax(0,1fr)_140px] gap-2"
            >
              <Input
                id="ssh-block-duration"
                v-model.number="form.blockDurationValue"
                type="number"
                min="1"
                max="365"
                :disabled="isSaving"
              />
              <Select v-model="form.blockDurationUnit">
                <SelectTrigger
                  :aria-label="t('admin.sshSecurity.blockDuration')"
                  :disabled="isSaving"
                  ><SelectValue
                /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="minute">
                    {{ t("admin.sshSecurity.minute") }}
                  </SelectItem>
                  <SelectItem value="hour">
                    {{ t("admin.sshSecurity.hour") }}
                  </SelectItem>
                  <SelectItem value="day">
                    {{ t("admin.sshSecurity.day") }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <div
            class="grid gap-3 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
          >
            <div class="space-y-1">
              <div class="text-sm font-medium">
                {{ t("admin.sshSecurity.allowedRegions") }}
              </div>
            </div>
            <div class="w-full max-w-2xl space-y-3">
              <CidrRegionSelector
                v-model="form.allowedRegions"
                :disabled="regionInputsDisabled"
                :description="t('admin.sshSecurity.allowedRegionsDescription')"
                :text="{
                  add: t('admin.gatewayVisibilitySettings.saveSelection'),
                  addRegion: t('admin.gatewayVisibilitySettings.manageRegions'),
                  cancel: t('common.cancel'),
                  dialogDescription: t(
                    'admin.sshSecurity.addRegionDescription',
                  ),
                  loadFailed: t('admin.sshSecurity.regionsLoadFailed'),
                  loadFailedDescription: t(
                    'admin.sshSecurity.regionsLoadDescription',
                  ),
                  loading: t('admin.sshSecurity.loading'),
                  noRegions: t('admin.sshSecurity.noRegions'),
                  province: t('admin.sshSecurity.province'),
                  retry: t('admin.subdomainProxy.retry'),
                  selectedCount: (count) =>
                    t('admin.gatewayVisibilitySettings.selectedRegionCount', {
                      count,
                    }),
                  scope: t('admin.sshSecurity.scope'),
                  selectCity: t('admin.sshSecurity.selectCity'),
                  selectProvince: t('admin.sshSecurity.selectProvince'),
                  selectProvinceFirst: t(
                    'admin.sshSecurity.selectProvinceFirst',
                  ),
                  unavailable: t(
                    'admin.gatewayVisibilitySettings.unavailableSelection',
                  ),
                }"
              />
            </div>
          </div>

          <div
            class="grid gap-3 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[200px_1fr] sm:p-6 md:grid-cols-[240px_1fr]"
          >
            <div class="space-y-1">
              <Label for="ssh-custom-cidrs" class="text-sm font-medium">
                {{ t("admin.sshSecurity.customCidrs") }}
              </Label>
              <p
                class="hidden pr-4 text-xs leading-5 text-muted-foreground sm:block"
              >
                {{ t("admin.sshSecurity.customCidrsDescription") }}
              </p>
            </div>
            <div class="w-full max-w-2xl space-y-2">
              <Textarea
                id="ssh-custom-cidrs"
                v-model="form.customCidrsText"
                class="min-h-32 font-mono text-sm"
                placeholder="1.2.3.0/24"
                :disabled="isSaving"
              />
              <p
                class="text-sm"
                :class="
                  invalidCustomCidrs.length > 0
                    ? 'text-destructive'
                    : 'text-muted-foreground'
                "
              >
                {{
                  invalidCustomCidrs.length > 0
                    ? t("admin.sshSecurity.customCidrsInvalid", {
                        items: invalidCustomCidrs.join(
                          t("admin.sshSecurity.listSeparator"),
                        ),
                      })
                    : t("admin.sshSecurity.customCidrsRecognized", {
                        count: customCidrsState.cidrs.length,
                      })
                }}
              </p>
            </div>
          </div>
        </div>
      </template>

      <template #actions="{ collapse }">
        <Button variant="outline" @click="collapse">
          {{ t("admin.sshSecurity.collapse") }}
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button
              variant="outline"
              class="gap-2"
              :disabled="
                isSaving ||
                isSyncingFirewall ||
                !details ||
                !details.summary.available
              "
            >
              <Loader2 v-if="isSyncingFirewall" class="h-4 w-4 animate-spin" />
              <span>{{ t("admin.sshSecurity.actions") }}</span>
              <ChevronDown class="h-4 w-4 text-muted-foreground" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-56">
            <DropdownMenuItem
              :disabled="
                isSaving ||
                isSyncingFirewall ||
                !details ||
                !details.summary.available
              "
              @select="syncFirewall"
            >
              <RefreshCw class="h-4 w-4" />
              {{ t("admin.sshSecurity.syncFirewall") }}
            </DropdownMenuItem>
            <DropdownMenuItem
              class="text-destructive focus:text-destructive"
              :disabled="
                isSaving ||
                isSyncingFirewall ||
                !details ||
                !details.summary.available
              "
              @select="openClearFirewallDialog"
            >
              <Trash2 class="h-4 w-4" />
              {{ t("admin.sshSecurity.clearSshFirewall") }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <Button
          :disabled="
            isSaving || isSyncingFirewall || Boolean(saveBlockedReason)
          "
          @click="saveConfig"
        >
          <Save class="h-4 w-4" />
          {{
            isSaving
              ? t("admin.sshSecurity.saving")
              : t("admin.sshSecurity.saveConfig")
          }}
        </Button>
      </template>
    </ConfigCollapsibleCard>

    <Tabs v-model="activeTab" class="space-y-4">
      <TabsList>
        <TabsTrigger value="login-logs">
          {{ t("admin.sshSecurity.loginLogs") }}
        </TabsTrigger>
        <TabsTrigger value="blocks">
          {{ t("admin.sshSecurity.blockList") }}
        </TabsTrigger>
      </TabsList>

      <TabsContent value="login-logs"><SSHLoginLogsPanel /></TabsContent>
      <TabsContent value="blocks">
        <SSHBlockListPanel
          :ref="setBlockListPanel"
          :reload-details="loadDetails"
        />
      </TabsContent>
    </Tabs>

    <Dialog v-model:open="isClearFirewallDialogOpen">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {{ t("admin.sshSecurity.clearFirewallTitle") }}
          </DialogTitle>
          <DialogDescription>
            {{ t("admin.sshSecurity.clearFirewallDescription") }}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            variant="outline"
            :disabled="isSyncingFirewall"
            @click="isClearFirewallDialogOpen = false"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            variant="destructive"
            :disabled="isSyncingFirewall"
            @click="clearFirewall"
          >
            <Loader2 v-if="isSyncingFirewall" class="h-4 w-4 animate-spin" />
            {{ t("admin.sshSecurity.clear") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
