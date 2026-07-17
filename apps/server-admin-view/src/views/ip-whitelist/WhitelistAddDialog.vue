<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { AlertTriangle } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
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
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import CidrRegionSelector from "@/components/CidrRegionSelector.vue";
import type { GatewayVisibilitySelection } from "@/lib/api";
import type { WhitelistNewRecord } from "./useWhitelistAddRecord";

defineProps<{
  canSave: boolean;
  isRegionCidrMode: boolean;
  isSaving: boolean;
  newRecordPlaceholder: string;
  regionInputsDisabled: boolean;
}>();
const emit = defineEmits<{ add: [] }>();
const open = defineModel<boolean>("open", { required: true });
const cidrInputMode = defineModel<"manual" | "region">("cidrInputMode", {
  required: true,
});
const customHours = defineModel<number>("customHours", { required: true });
const durationSetting = defineModel<string>("durationSetting", {
  required: true,
});
const newRecord = defineModel<WhitelistNewRecord>("newRecord", {
  required: true,
});
const regionSelections = defineModel<GatewayVisibilitySelection[]>(
  "regionSelections",
  { required: true },
);
const { t } = useI18n();
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-[640px]">
      <DialogHeader>
        <DialogTitle>{{ t("admin.ipWhitelist.addDialogTitle") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.ipWhitelist.addDialogDescription") }}
        </DialogDescription>
      </DialogHeader>
      <div class="grid gap-4 py-4">
        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="targetType" class="text-right">
            {{ t("admin.ipWhitelist.type") }}
          </Label>
          <Select v-model="newRecord.targetType">
            <SelectTrigger class="col-span-3">
              <SelectValue :placeholder="t('admin.ipWhitelist.selectType')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="ip">
                {{ t("admin.ipWhitelist.typeIp") }}
              </SelectItem>
              <SelectItem value="cidr">
                {{ t("admin.ipWhitelist.typeCidr") }}
              </SelectItem>
              <SelectItem value="cname">
                {{ t("admin.ipWhitelist.typeCname") }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div
          v-if="newRecord.targetType === 'cidr'"
          class="grid grid-cols-4 items-center gap-4"
        >
          <Label class="text-right">
            {{ t("admin.ipWhitelist.cidrInputMode") }}
          </Label>
          <div
            class="col-span-3 inline-flex w-fit rounded-md border border-border bg-muted/20 p-1"
          >
            <Button
              type="button"
              size="sm"
              :variant="cidrInputMode === 'manual' ? 'default' : 'ghost'"
              class="h-8"
              @click="cidrInputMode = 'manual'"
            >
              {{ t("admin.ipWhitelist.cidrInputManual") }}
            </Button>
            <Button
              type="button"
              size="sm"
              :variant="cidrInputMode === 'region' ? 'default' : 'ghost'"
              class="h-8"
              @click="cidrInputMode = 'region'"
            >
              {{ t("admin.ipWhitelist.cidrInputRegion") }}
            </Button>
          </div>
        </div>

        <div
          v-if="!isRegionCidrMode"
          class="grid grid-cols-4 items-center gap-4"
        >
          <Label for="ip" class="text-right">
            {{ t("admin.ipWhitelist.target") }}
          </Label>
          <Input
            id="ip"
            v-model="newRecord.ip"
            :placeholder="newRecordPlaceholder"
            class="col-span-3"
          />
        </div>

        <div v-else class="grid grid-cols-4 items-start gap-4">
          <Label class="pt-2 text-right">
            {{ t("admin.ipWhitelist.regionScope") }}
          </Label>
          <div class="col-span-3 space-y-3">
            <Alert variant="destructive" class="items-start">
              <AlertTriangle class="h-4 w-4" />
              <AlertTitle>
                {{ t("admin.ipWhitelist.regionSecurityWarningTitle") }}
              </AlertTitle>
              <AlertDescription>
                {{ t("admin.ipWhitelist.regionSecurityWarningDescription") }}
              </AlertDescription>
            </Alert>
            <CidrRegionSelector
              v-model="regionSelections"
              :disabled="regionInputsDisabled"
              :description="t('admin.ipWhitelist.regionScopeDescription')"
              :text="{
                add: t('admin.gatewayVisibilitySettings.saveSelection'),
                addRegion: t('admin.gatewayVisibilitySettings.manageRegions'),
                cancel: t('common.cancel'),
                dialogDescription: t('admin.ipWhitelist.addRegionDescription'),
                loadFailed: t('admin.ipWhitelist.regionsLoadFailed'),
                loadFailedDescription: t(
                  'admin.ipWhitelist.regionsLoadDescription',
                ),
                loading: t('admin.ipWhitelist.loading'),
                noRegions: t('admin.ipWhitelist.noRegions'),
                province: t('admin.ipWhitelist.province'),
                retry: t('admin.subdomainProxy.retry'),
                selectedCount: (count) =>
                  t('admin.gatewayVisibilitySettings.selectedRegionCount', {
                    count,
                  }),
                scope: t('admin.ipWhitelist.scope'),
                selectCity: t('admin.ipWhitelist.selectCity'),
                selectProvince: t('admin.ipWhitelist.selectProvince'),
                selectProvinceFirst: t('admin.ipWhitelist.selectProvinceFirst'),
                unavailable: t(
                  'admin.gatewayVisibilitySettings.unavailableSelection',
                ),
              }"
            />
          </div>
        </div>

        <div
          v-if="newRecord.targetType === 'cname'"
          class="grid grid-cols-4 items-center gap-4"
        >
          <Label for="checkIntervalMinutes" class="text-right">
            {{ t("admin.ipWhitelist.checkIntervalLabel") }}
          </Label>
          <div class="col-span-3 flex items-center gap-2">
            <Input
              id="checkIntervalMinutes"
              v-model.number="newRecord.checkIntervalMinutes"
              type="number"
              min="1"
              :placeholder="t('admin.ipWhitelist.defaultFive')"
            />
            <span class="whitespace-nowrap text-sm text-muted-foreground">
              {{ t("admin.ipWhitelist.minutes") }}
            </span>
          </div>
        </div>

        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="duration" class="text-right">
            {{ t("admin.ipWhitelist.duration") }}
          </Label>
          <Select v-model="durationSetting">
            <SelectTrigger class="col-span-3">
              <SelectValue
                :placeholder="t('admin.ipWhitelist.selectDuration')"
              />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="permanent">
                {{ t("admin.ipWhitelist.permanent") }}
              </SelectItem>
              <SelectItem value="1h">
                {{ t("admin.ipWhitelist.oneHour") }}
              </SelectItem>
              <SelectItem value="24h">
                {{ t("admin.ipWhitelist.twentyFourHours") }}
              </SelectItem>
              <SelectItem value="7d">
                {{ t("admin.ipWhitelist.sevenDays") }}
              </SelectItem>
              <SelectItem value="custom">
                {{ t("admin.ipWhitelist.customHours") }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div
          v-if="durationSetting === 'custom'"
          class="grid grid-cols-4 items-center gap-4"
        >
          <Label for="customHours" class="text-right">
            {{ t("admin.ipWhitelist.customHours") }}
          </Label>
          <Input
            id="customHours"
            v-model.number="customHours"
            type="number"
            min="1"
            :placeholder="t('admin.ipWhitelist.customHoursPlaceholder')"
            class="col-span-3"
          />
        </div>

        <div class="grid grid-cols-4 items-center gap-4">
          <Label for="comment" class="text-right">
            {{ t("admin.ipWhitelist.commentOptional") }}
          </Label>
          <Input
            id="comment"
            v-model="newRecord.comment"
            :placeholder="t('admin.ipWhitelist.commentPlaceholder')"
            class="col-span-3"
            @keyup.enter="emit('add')"
          />
        </div>
      </div>
      <DialogFooter>
        <Button variant="outline" @click="open = false">
          {{ t("common.cancel") }}
        </Button>
        <Button :disabled="!canSave || isSaving" @click="emit('add')">
          {{ t("common.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
