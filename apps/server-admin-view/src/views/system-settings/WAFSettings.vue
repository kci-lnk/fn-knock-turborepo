<script setup lang="ts">
import { useId } from "vue";
import { RefreshCw, TriangleAlert, Upload } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { TooltipProvider } from "@/components/ui/tooltip";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import WAFRuleList from "./waf-settings/WAFRuleList.vue";
import { useWAFSettings } from "./waf-settings/useWAFSettings";

const a11yId = useId();

const {
  activateRuleActions,
  activeRuleActionsKey,
  activeRulePreview,
  customRules,
  deleteCustomRule,
  details,
  downloadingRuleKey,
  downloadRuleFile,
  enableRecommendedSystemRules,
  form,
  formatCustomRuleMeta,
  formatCustomRuleName,
  formatDate,
  formatRuleSize,
  formatSize,
  formatSystemRuleMeta,
  formatSystemRuleName,
  handleAutoUpdateChange,
  handleCommonLocationExemptChange,
  handleEnabledChange,
  handleParanoiaLevelChange,
  handleUploadChange,
  isBusy,
  isChangingRules,
  isLoading,
  isRulePreviewOpen,
  isUpdatingSystemRules,
  levelOptions,
  loadingRuleKey,
  manifestLabel,
  openRulePreview,
  selectedCustomRules,
  selectedSystemRules,
  setAllSelected,
  setRuleSelected,
  showLoadingSkeleton,
  sourceLabel,
  syncedLabel,
  systemRules,
  t,
  toggleAllRules,
  toggleRule,
  triggerUpload,
  updateSelectedRules,
  updateSystemRules,
  uploadInputRef,
} = useWAFSettings();

// Vue assigns this string template ref at runtime.
void uploadInputRef;
</script>

<template>
  <TooltipProvider>
    <Card>
      <CardHeader>
        <div class="space-y-1.5">
          <CardTitle class="text-md">
            {{ t("admin.wafSettings.title") }}
          </CardTitle>
          <CardDescription>
            {{ t("admin.wafSettings.description") }}
          </CardDescription>
        </div>
      </CardHeader>

      <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
        <div class="space-y-4 p-6">
          <Skeleton class="h-6 w-1/3" />
          <Skeleton class="h-4 w-2/3" />
          <Skeleton class="h-24 w-full" />
        </div>
      </CardContent>

      <CardContent v-else class="border-t p-0 divide-y">
        <section v-if="form.enabled" class="p-6">
          <Alert
            class="items-start rounded-xl border-amber-200 bg-amber-50/70 text-amber-950 [&>svg]:text-amber-600"
          >
            <TriangleAlert class="mt-0.5 h-4 w-4" />
            <AlertTitle>
              {{ t("admin.wafSettings.falsePositiveTitle") }}
            </AlertTitle>
            <AlertDescription class="text-sm leading-6 text-amber-900">
              {{ t("admin.wafSettings.falsePositiveDescription") }}
            </AlertDescription>
          </Alert>
        </section>

        <section
          class="flex flex-col gap-4 bg-muted/10 p-6 sm:flex-row sm:items-center sm:justify-between"
        >
          <div class="space-y-1 pr-6">
            <Label
              :for="`${a11yId}-wafsettings-1`"
              class="cursor-pointer text-base font-medium"
              @click="handleEnabledChange(!form.enabled)"
            >
              {{ t("admin.wafSettings.enableWaf") }}
            </Label>
            <div class="text-sm text-muted-foreground">
              {{ t("admin.wafSettings.enableWafDescription") }}
            </div>
          </div>
          <Switch
            :id="`${a11yId}-wafsettings-1`"
            :model-value="form.enabled"
            :disabled="isBusy"
            @update:model-value="(value) => handleEnabledChange(value === true)"
          />
        </section>

        <section
          v-if="form.enabled"
          class="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between"
        >
          <div class="space-y-1 pr-6">
            <Label
              :for="`${a11yId}-wafsettings-2`"
              class="cursor-pointer text-base font-medium"
              @click="
                handleAutoUpdateChange(!form.system_rules_auto_update_enabled)
              "
            >
              {{ t("admin.wafSettings.autoUpdate") }}
            </Label>
            <div class="text-sm text-muted-foreground">
              {{ t("admin.wafSettings.autoUpdateDescription") }}
            </div>
          </div>
          <Switch
            :id="`${a11yId}-wafsettings-2`"
            :model-value="form.system_rules_auto_update_enabled"
            :disabled="isBusy"
            @update:model-value="
              (value) => handleAutoUpdateChange(value === true)
            "
          />
        </section>

        <section
          v-if="form.enabled"
          class="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between"
        >
          <div class="space-y-1 pr-6">
            <Label
              :for="`${a11yId}-wafsettings-3`"
              class="cursor-pointer text-base font-medium"
              @click="
                handleCommonLocationExemptChange(
                  !form.common_location_exempt_enabled,
                )
              "
            >
              {{ t("admin.wafSettings.commonLocationExempt") }}
            </Label>
            <div class="text-sm text-muted-foreground">
              {{ t("admin.wafSettings.commonLocationExemptDescription") }}
            </div>
          </div>
          <Switch
            :id="`${a11yId}-wafsettings-3`"
            :model-value="form.common_location_exempt_enabled"
            :disabled="isBusy"
            @update:model-value="
              (value) => handleCommonLocationExemptChange(value === true)
            "
          />
        </section>

        <template v-if="form.enabled">
          <section
            class="grid gap-6 p-6 lg:grid-cols-[minmax(0,1fr)_minmax(360px,520px)]"
          >
            <div class="space-y-1 pr-6">
              <Label :for="`${a11yId}-wafsettings-4`" class="text-base">
                {{ t("admin.wafSettings.protectionLevel") }}
              </Label>
              <div class="text-sm text-muted-foreground">
                {{ t("admin.wafSettings.protectionLevelDescription") }}
              </div>
            </div>
            <div class="grid justify-items-end gap-5">
              <Select
                :model-value="String(form.paranoia_level)"
                :disabled="isBusy"
                @update:model-value="handleParanoiaLevelChange"
              >
                <SelectTrigger :id="`${a11yId}-wafsettings-4`">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="level in levelOptions"
                    :key="level.value"
                    :value="level.value"
                  >
                    {{ level.label }} · {{ level.description }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </section>

          <section class="space-y-5 p-6">
            <div
              class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
            >
              <div class="space-y-1">
                <div class="flex items-center gap-2">
                  <div class="text-base font-medium">
                    {{ t("admin.wafSettings.systemRules") }}
                  </div>
                  <Badge
                    v-if="details?.system.update_available"
                    variant="secondary"
                  >
                    {{ t("admin.wafSettings.updateAvailable") }}
                  </Badge>
                </div>
                <div class="text-sm text-muted-foreground">
                  {{
                    t("admin.wafSettings.manifestLocal", {
                      manifest: manifestLabel,
                      synced: syncedLabel,
                    })
                  }}
                </div>
                <div
                  v-if="details?.system.manifest_last_error"
                  class="text-sm text-destructive"
                >
                  {{ details.system.manifest_last_error }}
                </div>
              </div>
              <div class="flex flex-wrap gap-2">
                <Button size="sm" :disabled="isBusy" @click="updateSystemRules">
                  <RefreshCw
                    class="mr-2 h-4 w-4"
                    :class="isUpdatingSystemRules ? 'animate-spin' : ''"
                  />
                  {{ t("admin.wafSettings.updateRules") }}
                </Button>
              </div>
            </div>

            <WAFRuleList
              :active-rule-actions-key="activeRuleActionsKey"
              :downloading-rule-key="downloadingRuleKey"
              :empty-label="t('admin.wafSettings.notSyncedSystemRules')"
              :format-rule-aside="formatRuleSize"
              :format-rule-meta="formatSystemRuleMeta"
              :format-rule-name="formatSystemRuleName"
              :is-busy="isBusy"
              :is-changing-rules="isChangingRules"
              :loading-rule-key="loadingRuleKey"
              :rules="systemRules"
              :selected-filenames="selectedSystemRules"
              show-recommended-action
              :toggle-all-rules-action="() => toggleAllRules('system')"
              @activate-rule-actions="activateRuleActions"
              @apply-recommended="enableRecommendedSystemRules"
              @download-rule-file="downloadRuleFile"
              @open-rule-preview="openRulePreview"
              @set-all-selected="(checked) => setAllSelected('system', checked)"
              @set-rule-selected="
                (filename, checked) =>
                  setRuleSelected('system', filename, checked)
              "
              @toggle-rule="(rule, enabled) => toggleRule(rule, enabled)"
              @update-selected-rules="
                (enabled) => updateSelectedRules('system', enabled)
              "
            />
          </section>

          <section class="space-y-5 p-6">
            <div
              class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
            >
              <div class="space-y-1">
                <Label :for="`${a11yId}-wafsettings-5`" class="text-base">
                  {{ t("admin.wafSettings.customRules") }}
                </Label>
                <div class="text-sm text-muted-foreground">
                  {{ t("admin.wafSettings.customRulesDescription") }}
                </div>
              </div>
              <div>
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="isBusy"
                  @click="triggerUpload"
                >
                  <Upload class="mr-2 h-4 w-4" />
                  {{ t("admin.wafSettings.uploadRules") }}
                </Button>
                <input
                  :id="`${a11yId}-wafsettings-5`"
                  ref="uploadInputRef"
                  type="file"
                  class="hidden"
                  accept=".conf"
                  multiple
                  @change="handleUploadChange"
                />
              </div>
            </div>

            <WAFRuleList
              :active-rule-actions-key="activeRuleActionsKey"
              :delete-rule="deleteCustomRule"
              :downloading-rule-key="downloadingRuleKey"
              :empty-label="t('admin.wafSettings.noCustomRules')"
              :format-rule-meta="formatCustomRuleMeta"
              :format-rule-name="formatCustomRuleName"
              :is-busy="isBusy"
              :is-changing-rules="isChangingRules"
              :loading-rule-key="loadingRuleKey"
              :rules="customRules"
              :selected-filenames="selectedCustomRules"
              show-delete
              :toggle-all-rules-action="() => toggleAllRules('custom')"
              @activate-rule-actions="activateRuleActions"
              @download-rule-file="downloadRuleFile"
              @open-rule-preview="openRulePreview"
              @set-all-selected="(checked) => setAllSelected('custom', checked)"
              @set-rule-selected="
                (filename, checked) =>
                  setRuleSelected('custom', filename, checked)
              "
              @toggle-rule="(rule, enabled) => toggleRule(rule, enabled)"
              @update-selected-rules="
                (enabled) => updateSelectedRules('custom', enabled)
              "
            />
          </section>
        </template>
      </CardContent>
    </Card>
    <DetailDialog
      v-model:open="isRulePreviewOpen"
      :title="activeRulePreview?.filename || t('admin.wafSettings.ruleContent')"
      :description="
        activeRulePreview
          ? `${sourceLabel(activeRulePreview.source)} · ${formatSize(activeRulePreview.size_bytes)} · ${formatDate(activeRulePreview.updated_at)}`
          : ''
      "
      max-width-class="sm:max-w-[840px]"
      close-variant="default"
    >
      <div
        v-if="activeRulePreview"
        class="overflow-hidden rounded-md border bg-muted/20"
      >
        <pre
          class="max-h-[60vh] overflow-auto whitespace-pre-wrap break-words p-3 font-mono text-xs leading-5 text-foreground"
          >{{ activeRulePreview.content }}</pre
        >
      </div>
    </DetailDialog>
  </TooltipProvider>
</template>
