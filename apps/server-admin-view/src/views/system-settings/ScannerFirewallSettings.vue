<script setup lang="ts">
import { useId } from "vue";
import { useI18n } from "vue-i18n";
import { Shield } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import ScannerFirewallExemptions from "./scanner-firewall/ScannerFirewallExemptions.vue";
import ScannerPathWhitelistEntry from "./scanner-firewall/ScannerPathWhitelistEntry.vue";
import ScannerFirewallThresholds from "./scanner-firewall/ScannerFirewallThresholds.vue";
import { useScannerFirewallSettings } from "./scanner-firewall/useScannerFirewallSettings";

const { t } = useI18n();
const a11yId = useId();
const model = useScannerFirewallSettings();
</script>

<template>
  <Card>
    <CardHeader>
      <div class="flex flex-col justify-between gap-4 sm:flex-row sm:items-center">
        <div>
          <CardTitle class="text-md">
            {{ t("admin.scannerFirewallSettings.title") }}
          </CardTitle>
          <CardDescription class="mt-1.5">
            {{ t("admin.scannerFirewallSettings.description") }}
          </CardDescription>
        </div>
        <Button
          variant="secondary"
          size="sm"
          class="shrink-0"
          @click="model.goToBlacklist"
        >
          <Shield class="mr-2 h-4 w-4" />
          {{ t("admin.scannerFirewallSettings.viewBlacklist") }}
        </Button>
      </div>
    </CardHeader>

    <CardContent
      v-if="model.isLoading && model.showLoadingSkeleton"
      class="border-t p-0"
    >
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!model.isLoading" class="divide-y border-t p-0">
      <div class="flex items-center justify-between bg-muted/10 p-6">
        <div class="space-y-1 pr-6">
          <Label
            :for="`${a11yId}-enabled`"
            class="cursor-pointer text-base font-medium"
            @click="model.form.enabled = !model.form.enabled"
          >
            {{ t("admin.scannerFirewallSettings.enableTitle") }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.scannerFirewallSettings.enableDescription") }}
          </div>
        </div>
        <Switch :id="`${a11yId}-enabled`" v-model="model.form.enabled" />
      </div>

      <ScannerPathWhitelistEntry @open="model.goToPathWhitelist" />

      <div
        v-show="model.form.enabled"
        class="divide-y duration-300 animate-in fade-in slide-in-from-top-2"
      >
        <div class="flex items-center justify-between gap-4 p-6">
          <div class="space-y-1 pr-6">
            <Label
              :for="`${a11yId}-common-location`"
              class="cursor-pointer text-base font-medium"
              @click="
                model.form.commonLocationExemptEnabled =
                  !model.form.commonLocationExemptEnabled
              "
            >
              {{ t("admin.scannerFirewallSettings.commonLocationExemptTitle") }}
            </Label>
            <div class="text-sm text-muted-foreground">
              {{
                t(
                  "admin.scannerFirewallSettings.commonLocationExemptDescription",
                )
              }}
            </div>
          </div>
          <Switch
            :id="`${a11yId}-common-location`"
            v-model="model.form.commonLocationExemptEnabled"
          />
        </div>

        <ScannerFirewallExemptions :model="model" />
        <ScannerFirewallThresholds :model="model" :id-prefix="a11yId" />
      </div>
    </CardContent>
    <CardContent v-else class="min-h-[200px]" aria-hidden="true" />

    <FloatingActionDock
      :active="model.isDirty"
      inline-class="flex items-center justify-between p-6 border-t bg-muted/20 rounded-b-xl"
    >
      <template #inline>
        <div class="text-sm text-muted-foreground">
          {{
            t(
              model.isDirty
                ? "admin.scannerFirewallSettings.dirty"
                : "admin.scannerFirewallSettings.clean",
            )
          }}
        </div>
        <div class="flex gap-3">
          <Button
            variant="outline"
            :disabled="!model.isDirty || model.isSaving"
            @click="model.resetForm"
          >
            {{ t("admin.scannerFirewallSettings.discard") }}
          </Button>
          <Button
            :disabled="
              !model.isDirty || model.isSaving || Boolean(model.saveBlockedReason)
            "
            @click="model.saveSettings"
          >
            <span
              v-if="model.isSaving"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            />
            {{ t("admin.scannerFirewallSettings.saveChanges") }}
          </Button>
        </div>
      </template>
      <template #floating>
        <Button
          variant="outline"
          :disabled="!model.isDirty || model.isSaving"
          @click="model.resetForm"
        >
          {{ t("admin.scannerFirewallSettings.discard") }}
        </Button>
        <Button
          :disabled="
            !model.isDirty || model.isSaving || Boolean(model.saveBlockedReason)
          "
          @click="model.saveSettings"
        >
          <span
            v-if="model.isSaving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          />
          {{ t("admin.scannerFirewallSettings.saveChanges") }}
        </Button>
      </template>
    </FloatingActionDock>
  </Card>
</template>
