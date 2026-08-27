<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowRight, RefreshCw } from "lucide-vue-next";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { SubdomainTargetOptimizationController } from "./useSubdomainTargetOptimization";

const props = defineProps<{
  model: SubdomainTargetOptimizationController;
}>();
const { t } = useI18n();
const {
  allSelected,
  candidateLoadFailed,
  closeDialog,
  destinationAddress,
  destinations,
  handleOpenChange,
  isLoadingCandidates,
  isMappingSelected,
  isOpen,
  isSavingMappings,
  partiallySelected,
  previews,
  retryCandidates,
  saveOptimizedTargets,
  selectedCount,
  setAllSelected,
  setDestinationAddress,
  setMappingSelected,
} = props.model;

const selectedDestination = computed(() =>
  destinations.value.find(
    (destination) => destination.address === destinationAddress.value,
  ),
);
const destinationLabel = (address: string, direction: string) =>
  direction === "lan_to_loopback"
    ? t("admin.subdomainProxy.targetOptimizationLoopbackOption", { address })
    : t("admin.subdomainProxy.targetOptimizationLanOption", { address });
const handleDestinationChange = (value: unknown) =>
  setDestinationAddress(typeof value === "string" ? value : "");
</script>

<template>
  <Dialog :open="isOpen" @update:open="handleOpenChange">
    <DialogContent
      class="flex max-h-[88vh] max-w-[calc(100vw-2rem)] flex-col overflow-hidden sm:max-w-[880px]"
    >
      <DialogHeader class="shrink-0">
        <DialogTitle>
          {{ t("admin.subdomainProxy.targetOptimizationTitle") }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.subdomainProxy.targetOptimizationDescription") }}
        </DialogDescription>
      </DialogHeader>

      <div class="min-h-0 flex-1 space-y-4 overflow-y-auto py-1">
        <div class="space-y-2">
          <label class="text-sm font-medium" for="target-optimization-destination">
            {{ t("admin.subdomainProxy.targetOptimizationDestination") }}
          </label>
          <Select
            :model-value="destinationAddress"
            :disabled="isLoadingCandidates || destinations.length === 0"
            @update:model-value="handleDestinationChange"
          >
            <SelectTrigger id="target-optimization-destination" class="w-full">
              <SelectValue
                :placeholder="
                  t('admin.subdomainProxy.targetOptimizationDestinationPlaceholder')
                "
              />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="destination in destinations"
                :key="destination.address"
                :value="destination.address"
              >
                {{
                  destinationLabel(
                    destination.address,
                    destination.direction,
                  )
                }}
              </SelectItem>
            </SelectContent>
          </Select>
          <p v-if="selectedDestination" class="text-xs text-muted-foreground">
            {{
              t(
                selectedDestination.direction === "loopback_to_lan"
                  ? "admin.subdomainProxy.targetOptimizationLoopbackToLanHint"
                  : "admin.subdomainProxy.targetOptimizationLanToLoopbackHint",
              )
            }}
          </p>
        </div>

        <div
          v-if="candidateLoadFailed"
          class="flex flex-col gap-3 rounded-md border border-amber-500/40 bg-amber-500/5 p-3 text-sm sm:flex-row sm:items-center sm:justify-between"
        >
          <span>{{ t("admin.subdomainProxy.targetOptimizationLoadFailed") }}</span>
          <Button
            size="sm"
            variant="outline"
            :disabled="isLoadingCandidates"
            @click="retryCandidates"
          >
            <RefreshCw
              class="mr-2 h-4 w-4"
              :class="{ 'animate-spin': isLoadingCandidates }"
            />
            {{ t("common.retry") }}
          </Button>
        </div>

        <div
          v-if="isLoadingCandidates"
          class="flex items-center justify-center gap-3 rounded-md border py-14 text-sm text-muted-foreground"
        >
          <RefreshCw class="h-5 w-5 animate-spin" />
          {{ t("admin.subdomainProxy.targetOptimizationLoading") }}
        </div>
        <div
          v-else-if="previews.length === 0"
          class="rounded-md border py-14 text-center text-sm text-muted-foreground"
        >
          {{
            t(
              destinations.length === 0
                ? "admin.subdomainProxy.targetOptimizationNoCandidates"
                : "admin.subdomainProxy.targetOptimizationNoMappings",
            )
          }}
        </div>
        <div v-else class="overflow-hidden rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead class="w-12 text-center">
                  <Checkbox
                    :model-value="
                      partiallySelected ? 'indeterminate' : allSelected
                    "
                    :aria-label="t('common.selectAll')"
                    @update:model-value="setAllSelected($event === true)"
                  />
                </TableHead>
                <TableHead class="w-[34%]">
                  {{ t("admin.subdomainProxy.columns.domain") }}
                </TableHead>
                <TableHead>
                  {{ t("admin.subdomainProxy.targetOptimizationPreview") }}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="preview in previews" :key="preview.host">
                <TableCell class="text-center">
                  <Checkbox
                    :model-value="isMappingSelected(preview.host)"
                    :aria-label="
                      t('common.selectItem', { item: preview.host })
                    "
                    @update:model-value="
                      setMappingSelected(preview.host, $event === true)
                    "
                  />
                </TableCell>
                <TableCell class="max-w-0">
                  <span class="block truncate font-medium" :title="preview.host">
                    {{ preview.host }}
                  </span>
                </TableCell>
                <TableCell>
                  <div class="flex min-w-0 items-center gap-2 text-xs">
                    <code
                      class="min-w-0 flex-1 truncate rounded bg-muted px-2 py-1"
                      :title="preview.target"
                    >{{ preview.target }}</code>
                    <ArrowRight class="h-4 w-4 shrink-0 text-muted-foreground" />
                    <code
                      class="min-w-0 flex-1 truncate rounded bg-primary/10 px-2 py-1 text-primary"
                      :title="preview.nextTarget"
                    >{{ preview.nextTarget }}</code>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </div>

      <DialogFooter
        class="shrink-0 items-center gap-2 sm:flex-row sm:justify-between"
      >
        <span class="text-sm text-muted-foreground">
          {{
            t("admin.subdomainProxy.targetOptimizationSelected", {
              selected: selectedCount,
              total: previews.length,
            })
          }}
        </span>
        <div class="flex w-full flex-col-reverse gap-2 sm:w-auto sm:flex-row">
          <Button variant="outline" :disabled="isSavingMappings" @click="closeDialog">
            {{ t("admin.subdomainProxy.cancel") }}
          </Button>
          <Button
            :disabled="
              selectedCount === 0 ||
              isLoadingCandidates ||
              isSavingMappings
            "
            @click="saveOptimizedTargets"
          >
            {{ t("admin.subdomainProxy.targetOptimizationApply") }}
          </Button>
        </div>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
