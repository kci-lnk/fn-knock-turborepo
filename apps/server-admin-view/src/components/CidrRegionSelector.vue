<script setup lang="ts">
import { onMounted, toRef } from "vue";
import { AlertTriangle, Loader2, Plus } from "lucide-vue-next";
import type { AcceptableValue } from "reka-ui";
import { useI18n } from "vue-i18n";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  TagsInput,
  TagsInputItem,
  TagsInputItemDelete,
  TagsInputItemText,
} from "@/components/ui/tags-input";
import { CidrAPI } from "@/lib/api";
import type { CidrOperator, GatewayVisibilitySelection } from "@/types";
import {
  getCidrRegionSelectionKey,
  getCidrRegionSelectionLabel,
} from "@/types/cidr";
import { createCidrRegionSelectorState } from "./cidr-region-selector-state";

interface CidrRegionSelectorText {
  add: string;
  addRegion: string;
  cancel: string;
  dialogDescription: string;
  loadFailed: string;
  loadFailedDescription: string;
  loading: string;
  noRegions: string;
  province: string;
  retry: string;
  selectedCount: (count: number) => string;
  scope: string;
  selectCity: string;
  selectProvince: string;
  selectProvinceFirst: string;
  unavailable: string;
}

const props = withDefaults(
  defineProps<{
    description?: string;
    disabled?: boolean;
    text: CidrRegionSelectorText;
  }>(),
  {
    description: "",
    disabled: false,
  },
);

const selections = defineModel<GatewayVisibilitySelection[]>({
  required: true,
});
const { t } = useI18n();
const {
  activeSelectionKeys,
  canSaveSelections,
  capabilities,
  capabilitiesLoading,
  capabilityLoadError,
  cityChoices,
  cityOptionsLoading,
  draft,
  handleDialogOpenChange,
  isDialogOpen,
  loadCapabilities,
  loadProvinces,
  openDialog,
  operatorFilteringSupported,
  operators,
  provinces,
  provincesLoadError,
  provincesLoading,
  removeRegion,
  saveProvinceSelections,
  selectOperator,
  selectProvince,
  selectedCityCount,
  toggleCity,
} = createCidrRegionSelectorState({
  disabled: toRef(props, "disabled"),
  formatLoadError: (error) =>
    extractErrorMessage(error, props.text.loadFailedDescription),
  loadCapabilities: () => CidrAPI.getCapabilities(),
  loadCities: (province) => CidrAPI.getCities(province),
  loadProvinces: () => CidrAPI.getProvinces(),
  onLoadError: (description) => {
    toast.error(props.text.loadFailed, { description });
  },
  selections,
});

const handleProvinceChange = (value: AcceptableValue) => {
  selectProvince(typeof value === "string" ? value : "");
};
const ALL_OPERATORS_VALUE = "__all_operators__";
const handleOperatorChange = (value: AcceptableValue) => {
  const normalized = typeof value === "string" ? value : ALL_OPERATORS_VALUE;
  selectOperator(
    normalized === ALL_OPERATORS_VALUE ? null : (normalized as CidrOperator),
  );
};
const isCitySelected = (key: string) => activeSelectionKeys.value.includes(key);
onMounted(() => {
  void Promise.all([loadProvinces(), loadCapabilities()]);
});
</script>

<template>
  <div class="space-y-3">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <p v-if="description" class="text-sm leading-6 text-muted-foreground">
        {{ description }}
      </p>
      <span v-else></span>
      <Button
        type="button"
        variant="outline"
        size="sm"
        :disabled="disabled || provincesLoading || provinces.length === 0"
        @click="openDialog"
      >
        <Loader2 v-if="provincesLoading" class="h-4 w-4 animate-spin" />
        <Plus v-else class="h-4 w-4" />
        {{ text.addRegion }}
      </Button>
    </div>

    <div
      v-if="provincesLoadError"
      class="flex items-center justify-between gap-3 rounded-md bg-destructive/5 px-3 py-2 text-xs text-destructive"
    >
      <span>{{ provincesLoadError }}</span>
      <Button type="button" variant="ghost" size="sm" @click="loadProvinces">
        {{ text.retry }}
      </Button>
    </div>

    <div
      v-if="
        capabilityLoadError || (capabilities && !operatorFilteringSupported)
      "
      class="flex items-start gap-3 rounded-md border border-amber-500/30 bg-amber-500/5 px-3 py-2 text-xs text-amber-700 dark:text-amber-300"
    >
      <AlertTriangle class="mt-0.5 h-4 w-4 shrink-0" />
      <div class="min-w-0 flex-1 space-y-1">
        <p class="font-medium">
          {{ t("admin.cidrSelector.operatorUnavailable") }}
        </p>
        <p>
          {{
            capabilityLoadError
              ? t("admin.cidrSelector.capabilityCheckFailed")
              : t("admin.cidrSelector.operatorUpgradeRequired", {
                  version:
                    capabilities?.operatorFiltering.minimumContainerVersion ??
                    "0.1.3",
                })
          }}
        </p>
      </div>
      <Button
        v-if="capabilityLoadError"
        type="button"
        variant="ghost"
        size="sm"
        :disabled="capabilitiesLoading"
        @click="loadCapabilities"
      >
        {{ text.retry }}
      </Button>
    </div>

    <div class="rounded-xl bg-muted/20 px-4 py-4">
      <TagsInput
        :model-value="selections.map((item) => getCidrRegionSelectionKey(item))"
        class="min-h-0 items-start gap-2 border-none bg-transparent px-0 py-0 shadow-none"
      >
        <template v-if="selections.length > 0">
          <TagsInputItem
            v-for="selection in selections"
            :key="getCidrRegionSelectionKey(selection)"
            :value="getCidrRegionSelectionKey(selection)"
            class="h-auto rounded-full border border-border/70 bg-background pr-1"
          >
            <TagsInputItemText class="px-3 py-1.5">
              {{ getCidrRegionSelectionLabel(selection) }}
            </TagsInputItemText>
            <TagsInputItemDelete
              class="mr-1 rounded-full hover:bg-muted"
              :disabled="disabled"
              @click.prevent="removeRegion(selection)"
            />
          </TagsInputItem>
        </template>
        <span v-else class="px-1 py-1 text-sm text-muted-foreground">
          {{ text.noRegions }}
        </span>
      </TagsInput>
    </div>

    <Dialog :open="isDialogOpen" @update:open="handleDialogOpenChange">
      <DialogContent
        class="overflow-hidden border-border/70 bg-background p-0 shadow-xl sm:max-w-[560px]"
      >
        <div class="px-6 pt-6 pb-2">
          <DialogHeader class="space-y-2 text-left">
            <DialogTitle class="text-xl font-semibold tracking-tight">
              {{ text.addRegion }}
            </DialogTitle>
            <DialogDescription class="text-sm leading-6 text-muted-foreground">
              {{ text.dialogDescription }}
            </DialogDescription>
          </DialogHeader>
        </div>

        <div class="space-y-4 border-t border-border/60 px-6 py-5">
          <div class="grid gap-4 sm:grid-cols-2">
            <div class="space-y-2">
              <Label class="text-sm font-medium">{{ text.province }}</Label>
              <Select
                :model-value="draft.province"
                @update:model-value="handleProvinceChange"
              >
                <SelectTrigger
                  class="h-11 w-full rounded-lg border-border/70 bg-background px-3 shadow-none"
                  :disabled="disabled || provinces.length === 0"
                >
                  <SelectValue :placeholder="text.selectProvince" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="province in provinces"
                    :key="province.value"
                    :value="province.value"
                  >
                    {{ province.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div class="space-y-2">
              <Label class="text-sm font-medium">
                {{ t("admin.cidrSelector.operator") }}
              </Label>
              <Select
                :model-value="draft.operator ?? ALL_OPERATORS_VALUE"
                @update:model-value="handleOperatorChange"
              >
                <SelectTrigger
                  class="h-11 w-full rounded-lg border-border/70 bg-background px-3 shadow-none"
                  :disabled="disabled || capabilitiesLoading"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem :value="ALL_OPERATORS_VALUE">
                    {{ t("admin.cidrSelector.allOperators") }}
                  </SelectItem>
                  <SelectItem
                    v-for="operator in operators"
                    :key="operator"
                    :value="operator"
                    :disabled="!operatorFilteringSupported"
                  >
                    {{ operator }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div class="space-y-2 sm:col-span-2">
              <div class="flex min-h-5 items-center justify-between gap-3">
                <Label class="text-sm font-medium">{{ text.scope }}</Label>
                <span
                  v-if="draft.province && !cityOptionsLoading"
                  class="text-xs text-muted-foreground"
                >
                  {{ text.selectedCount(selectedCityCount) }}
                </span>
              </div>

              <div
                class="min-h-44 overflow-hidden rounded-lg border border-border/70 bg-background"
              >
                <div
                  v-if="cityOptionsLoading"
                  class="flex min-h-44 items-center justify-center gap-2 text-sm text-muted-foreground"
                >
                  <Loader2 class="h-4 w-4 animate-spin" />
                  {{ text.loading }}
                </div>
                <div
                  v-else-if="!draft.province"
                  class="flex min-h-44 items-center justify-center px-4 text-center text-sm text-muted-foreground"
                >
                  {{ text.selectProvinceFirst }}
                </div>
                <div
                  v-else-if="cityChoices.length === 0"
                  class="flex min-h-44 items-center justify-center px-4 text-center text-sm text-muted-foreground"
                >
                  {{ text.selectCity }}
                </div>
                <div v-else class="max-h-64 overflow-y-auto p-2">
                  <label
                    v-for="choice in cityChoices"
                    :key="choice.key"
                    class="flex cursor-pointer items-center gap-3 rounded-md px-3 py-2.5 transition-colors hover:bg-muted/50"
                    :class="{
                      'bg-primary/5': isCitySelected(choice.key),
                      'cursor-not-allowed opacity-60': disabled,
                    }"
                  >
                    <Checkbox
                      :model-value="isCitySelected(choice.key)"
                      :disabled="disabled"
                      @update:model-value="
                        (value) => toggleCity(choice.key, value === true)
                      "
                    />
                    <span class="min-w-0 flex-1 text-sm">
                      {{ choice.label }}
                    </span>
                    <span
                      v-if="choice.unavailable"
                      class="shrink-0 rounded-full bg-muted px-2 py-0.5 text-[11px] text-muted-foreground"
                    >
                      {{ text.unavailable }}
                    </span>
                  </label>
                </div>
              </div>
            </div>
          </div>
        </div>

        <DialogFooter
          class="border-t border-border/60 px-6 py-4 sm:justify-end"
        >
          <Button variant="outline" @click="handleDialogOpenChange(false)">
            {{ text.cancel }}
          </Button>
          <Button
            :disabled="!canSaveSelections"
            @click="saveProvinceSelections"
          >
            {{ text.add }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
