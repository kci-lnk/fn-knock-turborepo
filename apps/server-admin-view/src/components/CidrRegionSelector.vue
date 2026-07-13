<script setup lang="ts">
import { computed, onMounted, toRef } from "vue";
import { Loader2, Plus } from "lucide-vue-next";
import type { AcceptableValue } from "reka-ui";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { Button } from "@/components/ui/button";
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
import type { GatewayVisibilitySelection } from "@/types";
import { getCidrRegionSelectionKey } from "@/types/cidr";
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
  scope: string;
  selectCity: string;
  selectCityOrProvince: string;
  selectProvince: string;
  selectProvinceFirst: string;
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
const {
  addRegion,
  canAddRegion,
  cityOptions,
  cityOptionsLoading,
  citySelectKey,
  draft,
  handleDialogOpenChange,
  isDialogOpen,
  loadProvinces,
  openDialog,
  provinces,
  provincesLoadError,
  provincesLoading,
  removeRegion,
  selectProvince,
} = createCidrRegionSelectorState({
  disabled: toRef(props, "disabled"),
  formatLoadError: (error) =>
    extractErrorMessage(error, props.text.loadFailedDescription),
  loadCities: (province) => CidrAPI.getCities(province),
  loadProvinces: () => CidrAPI.getProvinces(),
  onLoadError: (description) => {
    toast.error(props.text.loadFailed, { description });
  },
  selections,
});

const citySelectPlaceholder = computed(() => {
  if (cityOptionsLoading.value) return props.text.loading;
  if (!draft.province) return props.text.selectProvinceFirst;
  return cityOptions.value.some((option) => option.isProvinceWide)
    ? props.text.selectCityOrProvince
    : props.text.selectCity;
});
const handleProvinceChange = (value: AcceptableValue) => {
  selectProvince(typeof value === "string" ? value : "");
};
onMounted(() => {
  void loadProvinces();
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
              {{ selection.label }}
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
              <Label class="text-sm font-medium">{{ text.scope }}</Label>
              <Select :key="citySelectKey" v-model="draft.cityValue">
                <SelectTrigger
                  class="h-11 w-full rounded-lg border-border/70 bg-background px-3 shadow-none"
                  :disabled="
                    disabled ||
                    !draft.province ||
                    cityOptionsLoading ||
                    cityOptions.length === 0
                  "
                >
                  <Loader2
                    v-if="cityOptionsLoading"
                    class="h-4 w-4 animate-spin text-muted-foreground"
                  />
                  <SelectValue :placeholder="citySelectPlaceholder" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="city in cityOptions"
                    :key="city.value"
                    :value="city.value"
                  >
                    {{ city.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </div>

        <DialogFooter
          class="border-t border-border/60 px-6 py-4 sm:justify-end"
        >
          <Button variant="outline" @click="handleDialogOpenChange(false)">
            {{ text.cancel }}
          </Button>
          <Button :disabled="!canAddRegion" @click="addRegion">
            {{ text.add }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
