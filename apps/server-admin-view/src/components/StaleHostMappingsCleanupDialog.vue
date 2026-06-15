<template>
  <Dialog :open="isOpen" @update:open="handleOpenChange">
    <DialogContent
      class="flex max-h-[88vh] flex-col overflow-hidden max-sm:max-h-[92dvh] max-sm:p-4 sm:max-w-[900px]"
    >
      <DialogHeader class="shrink-0">
        <div
          class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
        >
          <div class="space-y-1">
            <DialogTitle>{{
              t("admin.subdomainProxy.staleCleanupTitle")
            }}</DialogTitle>
            <DialogDescription>
              {{ t("admin.subdomainProxy.staleCleanupDescription") }}
            </DialogDescription>
          </div>
          <Button
            class="w-full sm:w-auto"
            variant="outline"
            :disabled="isProbing || isCleaning"
            @click="handleProbe"
          >
            <RefreshCw
              class="mr-2 h-4 w-4"
              :class="{ 'animate-spin': isProbing }"
            />
            {{
              isProbing
                ? t("admin.subdomainProxy.staleCleanupChecking")
                : t("admin.subdomainProxy.staleCleanupRefresh")
            }}
          </Button>
        </div>
      </DialogHeader>

      <div class="min-h-0 flex-1 overflow-x-hidden overflow-y-auto">
        <div class="py-2">
          <div
            v-if="isProbing"
            class="flex flex-col items-center justify-center space-y-4 py-16"
          >
            <RefreshCw class="h-8 w-8 animate-spin text-muted-foreground" />
            <p class="text-sm text-muted-foreground">
              {{ t("admin.subdomainProxy.staleCleanupProbing") }}
            </p>
          </div>

          <div
            v-else-if="probeableMappings.length === 0"
            class="py-16 text-center text-sm text-muted-foreground"
          >
            {{ t("admin.subdomainProxy.staleCleanupEmpty") }}
          </div>

          <div
            v-else-if="results.length === 0"
            class="py-16 text-center text-sm text-muted-foreground"
          >
            {{ t("admin.subdomainProxy.staleCleanupNoResults") }}
          </div>

          <div
            v-else-if="visibleResults.length === 0"
            class="py-16 text-center text-sm text-muted-foreground"
          >
            {{ t("admin.subdomainProxy.staleCleanupNoStale") }}
          </div>

          <div v-else>
            <div class="hidden rounded-md border bg-background sm:block">
              <Table
                class="w-full table-fixed"
                container-class="overflow-hidden"
              >
                <colgroup>
                  <col class="w-[6%]" />
                  <col class="w-[21%]" />
                  <col class="w-[29%]" />
                  <col class="w-[25%]" />
                  <col class="w-[19%]" />
                </colgroup>
                <TableHeader
                  class="sticky top-0 z-10 bg-background shadow-sm [&_th]:sticky [&_th]:top-0 [&_th]:z-10 [&_th]:bg-background"
                >
                  <TableRow class="h-12">
                    <TableHead class="px-4 text-center">
                      <input
                        type="checkbox"
                        class="h-4 w-4 cursor-pointer"
                        :checked="isAllStaleSelected"
                        :disabled="staleResults.length === 0"
                        @change="handleToggleAllStale"
                      />
                    </TableHead>
                    <TableHead class="px-4">{{
                      t("admin.subdomainProxy.staleCleanupColumns.title")
                    }}</TableHead>
                    <TableHead class="px-4">{{
                      t("admin.subdomainProxy.staleCleanupColumns.host")
                    }}</TableHead>
                    <TableHead class="px-4">{{
                      t("admin.subdomainProxy.staleCleanupColumns.target")
                    }}</TableHead>
                    <TableHead class="px-4">{{
                      t("admin.subdomainProxy.staleCleanupColumns.status")
                    }}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow
                    v-for="result in visibleResults"
                    :key="result.host"
                    class="h-16"
                  >
                    <TableCell class="px-4 py-3 text-center align-top">
                      <input
                        type="checkbox"
                        class="h-4 w-4 cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
                        :checked="isHostSelected(result.host)"
                        :disabled="result.status !== 'stale'"
                        @change="
                          (event) => handleToggleHost(result.host, event)
                        "
                      />
                    </TableCell>
                    <TableCell
                      class="whitespace-normal break-words px-4 py-4 text-sm font-medium leading-6 align-top"
                    >
                      {{ getMappingTitle(result.host) }}
                    </TableCell>
                    <TableCell
                      class="whitespace-normal break-all px-4 py-4 font-medium leading-6 align-top"
                    >
                      {{ result.host }}
                    </TableCell>
                    <TableCell
                      class="whitespace-normal break-all px-4 py-4 text-sm leading-6 align-top"
                    >
                      {{ result.target }}
                    </TableCell>
                    <TableCell class="whitespace-normal px-4 py-4 align-top">
                      <div class="space-y-1">
                        <Badge
                          :variant="getStatusBadgeVariant(result.status)"
                          :class="getStatusBadgeClass(result.status)"
                        >
                          {{ getStatusLabel(result) }}
                        </Badge>
                        <p
                          v-if="result.error"
                          class="whitespace-normal break-words text-xs leading-5 text-muted-foreground"
                        >
                          {{ result.error }}
                        </p>
                      </div>
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </div>

            <div class="space-y-3 sm:hidden">
              <label
                class="flex items-center gap-3 rounded-md border bg-background px-4 py-3 text-sm font-medium"
              >
                <input
                  type="checkbox"
                  class="h-4 w-4 cursor-pointer"
                  :checked="isAllStaleSelected"
                  :disabled="staleResults.length === 0"
                  @change="handleToggleAllStale"
                />
                {{ t("admin.subdomainProxy.staleCleanupSelectAll") }}
              </label>

              <div
                v-for="result in visibleResults"
                :key="`mobile-${result.host}`"
                class="rounded-md border bg-background p-4"
              >
                <div class="flex items-start gap-3">
                  <input
                    type="checkbox"
                    class="mt-1 h-4 w-4 shrink-0 cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
                    :checked="isHostSelected(result.host)"
                    :disabled="result.status !== 'stale'"
                    @change="(event) => handleToggleHost(result.host, event)"
                  />
                  <div class="min-w-0 flex-1 space-y-3">
                    <div class="min-w-0">
                      <p class="text-xs text-muted-foreground">
                        {{
                          t("admin.subdomainProxy.staleCleanupColumns.title")
                        }}
                      </p>
                      <p class="break-words text-sm font-medium leading-6">
                        {{ getMappingTitle(result.host) }}
                      </p>
                    </div>
                    <div class="min-w-0">
                      <p class="text-xs text-muted-foreground">
                        {{ t("admin.subdomainProxy.staleCleanupColumns.host") }}
                      </p>
                      <p class="break-all text-sm font-medium leading-6">
                        {{ result.host }}
                      </p>
                    </div>
                    <div class="min-w-0">
                      <p class="text-xs text-muted-foreground">
                        {{
                          t("admin.subdomainProxy.staleCleanupColumns.target")
                        }}
                      </p>
                      <p class="break-all text-sm leading-6">
                        {{ result.target }}
                      </p>
                    </div>
                    <div class="min-w-0 space-y-1">
                      <p class="text-xs text-muted-foreground">
                        {{
                          t("admin.subdomainProxy.staleCleanupColumns.status")
                        }}
                      </p>
                      <Badge
                        :variant="getStatusBadgeVariant(result.status)"
                        :class="getStatusBadgeClass(result.status)"
                      >
                        {{ getStatusLabel(result) }}
                      </Badge>
                      <p
                        v-if="result.error"
                        class="break-words text-xs leading-5 text-muted-foreground"
                      >
                        {{ result.error }}
                      </p>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <DialogFooter
        class="mt-2 shrink-0 items-stretch gap-3 sm:items-center sm:justify-between"
      >
        <span class="text-sm text-muted-foreground">
          <template v-if="visibleResults.length > 0">
            {{
              t("admin.subdomainProxy.staleCleanupSelected", {
                selected: selectedCount,
                total: staleResults.length,
              })
            }}
          </template>
        </span>
        <div class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button
            variant="outline"
            class="w-full sm:w-auto"
            :disabled="isCleaning"
            @click="closeDialog"
          >
            {{ t("admin.subdomainProxy.cancel") }}
          </Button>
          <Button
            class="w-full sm:w-auto"
            variant="destructive"
            :disabled="selectedCount === 0 || isProbing || isCleaning"
            @click="handleCleanSelected"
          >
            <Trash2 v-if="!isCleaning" class="mr-2 h-4 w-4" />
            <RefreshCw v-else class="mr-2 h-4 w-4 animate-spin" />
            {{
              isCleaning
                ? t("admin.subdomainProxy.staleCleanupCleaning")
                : t("admin.subdomainProxy.staleCleanupCleanSelected")
            }}
          </Button>
        </div>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw, Trash2 } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { toast } from "@admin-shared/utils/toast";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { useStaleHostMappingsCleanup } from "../composables/useStaleHostMappingsCleanup";
import type {
  HostMappingProbeResult,
  HostMappingProbeStatus,
} from "../lib/api";
import type { HostMapping } from "../types";

const props = defineProps<{
  mappings: HostMapping[];
  saveMappings: (mappings: HostMapping[]) => Promise<unknown>;
  isAuthServiceTarget: (target: string) => boolean;
}>();

const emit = defineEmits<{
  cleaned: [count: number];
}>();

const { t } = useI18n();
const mappingsSource = computed(() => props.mappings);
const mappingTitleByHost = computed(() => {
  const titles = new Map<string, string>();
  for (const mapping of props.mappings) {
    titles.set(
      mapping.host.trim().toLowerCase(),
      mapping.title_override.trim() ||
        mapping.title.trim() ||
        t("admin.subdomainProxy.notFetched"),
    );
  }
  return titles;
});

const {
  open: isOpen,
  results,
  probeableMappings,
  staleResults,
  selectedCount,
  isAllStaleSelected,
  isProbing,
  isCleaning,
  openDialog,
  closeDialog,
  probe,
  cleanSelected,
  setHostSelected,
  isHostSelected,
  setAllStaleSelected,
} = useStaleHostMappingsCleanup({
  mappings: mappingsSource,
  saveMappings: (mappings) => props.saveMappings(mappings),
  isAuthServiceTarget: (target) => props.isAuthServiceTarget(target),
});

const visibleResults = computed(() =>
  results.value.filter((result) => result.status !== "online"),
);

const openCleanupDialog = async () => {
  openDialog();
  await nextTick();
  await handleProbe();
};

const handleOpenChange = (nextOpen: boolean) => {
  if (!nextOpen) {
    closeDialog();
  }
};

const handleProbe = async () => {
  if (probeableMappings.value.length === 0) {
    results.value = [];
    return;
  }

  try {
    await probe();
  } catch (error) {
    toast.error(t("admin.subdomainProxy.staleCleanupProbeFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.subdomainProxy.staleCleanupProbeFailedDescription"),
      ),
    });
  }
};

const handleCleanSelected = async () => {
  try {
    const cleanedCount = await cleanSelected();
    if (cleanedCount > 0) {
      toast.success(t("admin.subdomainProxy.staleCleanupCleaned"), {
        description: t("admin.subdomainProxy.staleCleanupCleanedDescription", {
          count: cleanedCount,
        }),
      });
      emit("cleaned", cleanedCount);
      closeDialog();
    }
  } catch (error) {
    toast.error(t("admin.subdomainProxy.staleCleanupCleanFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.subdomainProxy.staleCleanupCleanFailedDescription"),
      ),
    });
  }
};

const handleToggleAllStale = (event: Event) => {
  setAllStaleSelected((event.target as HTMLInputElement).checked);
};

const handleToggleHost = (host: string, event: Event) => {
  setHostSelected(host, (event.target as HTMLInputElement).checked);
};

const getMappingTitle = (host: string): string =>
  mappingTitleByHost.value.get(host.trim().toLowerCase()) ||
  t("admin.subdomainProxy.notFetched");

const getStatusLabel = (result: HostMappingProbeResult): string => {
  if (result.status === "online" && result.httpStatus) {
    return `${t("admin.subdomainProxy.staleCleanupStatus.online")} ${
      result.httpStatus
    }`;
  }
  return t(`admin.subdomainProxy.staleCleanupStatus.${result.status}`);
};

const getStatusBadgeVariant = (status: HostMappingProbeStatus) =>
  status === "stale" ? "destructive" : "secondary";

const getStatusBadgeClass = (status: HostMappingProbeStatus): string => {
  if (status === "online") {
    return "bg-emerald-500/10 text-emerald-700";
  }
  if (status === "unsupported") {
    return "bg-muted text-muted-foreground";
  }
  return "";
};

defineExpose({
  open: openCleanupDialog,
});
</script>
