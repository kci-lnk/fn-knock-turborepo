<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { isWebSocketProxyTargetUrl } from "@admin-shared/utils/proxyTargetInput";
import { toast } from "@admin-shared/utils/toast";
import { Pencil, Trash2 } from "lucide-vue-next";
import { isAnySubdomainRoutingMode } from "../../lib/reverse-proxy-submode";
import { useConfigStore } from "../../store/config";
import type { HostMapping, HostLocation } from "../../types";
import GatewayLocationHostPickerDialog from "./gateway-locations/GatewayLocationHostPickerDialog.vue";
import GatewayLocationRuleDialog from "./gateway-locations/GatewayLocationRuleDialog.vue";
import {
  cloneLocation,
  DEFAULT_RESPONSE_CONTENT_TYPE,
} from "./gateway-locations/gatewayLocationModel";
import { useGatewayLocationEditor } from "./gateway-locations/useGatewayLocationEditor";

type HostMappingTitleInfo = Pick<HostMapping, "title" | "title_override">;

const route = useRoute();
const router = useRouter();
const configStore = useConfigStore();
const { t } = useI18n();
const selectedHost = ref("");
const isHostPickerOpen = ref(false);
const draftLocations = ref<HostLocation[]>([]);

const { isPending: isLoading, run: runLoad } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.gatewayLocationsSettings.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.gatewayLocationsSettings.loadDescription"),
      ),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.gatewayLocationsSettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.gatewayLocationsSettings.saveDescription"),
      ),
    });
  },
});

const availableMappings = computed(() =>
  (configStore.config?.host_mappings ?? []).filter(
    (mapping) => mapping.service_role !== "auth",
  ),
);
const selectedMapping = computed(
  () =>
    availableMappings.value.find(
      (mapping) => mapping.host === selectedHost.value,
    ) ?? null,
);
const isAvailable = computed(() =>
  isAnySubdomainRoutingMode(configStore.config),
);
const isDirty = computed(() => {
  const saved = selectedMapping.value?.locations ?? [];
  return JSON.stringify(saved) !== JSON.stringify(draftLocations.value);
});
const sortedDraftLocations = computed(() =>
  draftLocations.value.map((location, index) => ({ location, index })),
);
const canSave = computed(
  () => Boolean(selectedMapping.value) && isDirty.value && !isSaving.value,
);

const getMappingDisplayTitle = (mapping?: HostMappingTitleInfo | null) =>
  mapping?.title_override.trim() || mapping?.title.trim() || "";

const getMappingTitleForDisplay = (mapping?: HostMappingTitleInfo | null) =>
  getMappingDisplayTitle(mapping) || "-";

const resetDraftFromSelected = () => {
  draftLocations.value = (selectedMapping.value?.locations ?? []).map(
    cloneLocation,
  );
};

const selectHost = (host: string) => {
  selectedHost.value = host;
  void router.replace({
    path: "/system/gateway-locations",
    query: host ? { host } : {},
  });
  resetDraftFromSelected();
};

const openHostPicker = () => {
  if (!isAvailable.value || availableMappings.value.length === 0) return;
  isHostPickerOpen.value = true;
};

const selectHostFromDialog = (host: string) => {
  selectHost(host);
  isHostPickerOpen.value = false;
};

const handleHostPickerOpenChange = (open: boolean) => {
  isHostPickerOpen.value = open;
};

const ensureSelectedHost = () => {
  const requestedHost =
    typeof route.query.host === "string" ? route.query.host.trim() : "";
  const hostExists = availableMappings.value.some(
    (mapping) => mapping.host === requestedHost,
  );
  selectedHost.value = hostExists
    ? requestedHost
    : (availableMappings.value[0]?.host ?? "");
  resetDraftFromSelected();
};

const persistLocations = async (locations: HostLocation[]) => {
  const host = selectedHost.value;
  const mapping = selectedMapping.value;
  if (!host || !mapping) return false;

  const result = await runSave(
    () =>
      configStore.saveHostMappings(
        (configStore.config?.host_mappings ?? []).map((item) =>
          item.host === host
            ? {
                ...item,
                locations: locations.map(cloneLocation),
              }
            : item,
        ),
      ),
    {
      onSuccess: () => {
        resetDraftFromSelected();
        toast.success(t("admin.gatewayLocationsSettings.saved"));
      },
    },
  );
  return result !== undefined;
};

const {
  addHeaderRow,
  closeDialog,
  editingIndex,
  form,
  formError,
  isDialogOpen,
  isProxyLocationWebSocketTarget,
  openCreateDialog,
  openEditDialog,
  removeHeaderRow,
  removeLocation,
  saveDialogLocation,
  setAction,
} = useGatewayLocationEditor({ draftLocations, persistLocations });

const saveLocations = async () => {
  await persistLocations(draftLocations.value);
};

const formatAction = (location: HostLocation) =>
  location.action === "response"
    ? t("admin.gatewayLocationsSettings.fixedResponse")
    : t("admin.gatewayLocationsSettings.proxyAction");

const formatTarget = (location: HostLocation) => {
  if (location.action === "response") {
    return `${location.response.status || 200} ${location.response.content_type || DEFAULT_RESPONSE_CONTENT_TYPE}`;
  }
  return location.target;
};

watch(
  () => route.query.host,
  () => {
    ensureSelectedHost();
  },
);

onMounted(async () => {
  if (!configStore.config) {
    await runLoad(() => configStore.loadConfig());
  }
  ensureSelectedHost();
});
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">
            {{ t("admin.gatewayLocationsSettings.systemSettings") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=gateway">
            {{ t("admin.gatewayLocationsSettings.gateway") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>
            {{ t("admin.gatewayLocationsSettings.title") }}
          </BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <div
      class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between"
    >
      <div class="max-w-3xl space-y-1.5">
        <h2 class="text-2xl font-semibold tracking-normal">
          {{ t("admin.gatewayLocationsSettings.title") }}
        </h2>
        <p class="text-sm leading-6 text-muted-foreground">
          {{ t("admin.gatewayLocationsSettings.description") }}
        </p>
      </div>
      <Button
        class="w-full sm:w-auto"
        :disabled="!selectedMapping || !isAvailable"
        @click="openCreateDialog"
      >
        {{ t("admin.gatewayLocationsSettings.addRule") }}
      </Button>
    </div>

    <Card class="border-border/60 shadow-none">
      <CardContent class="space-y-5 pt-6">
        <div
          v-if="isLoading && showLoadingSkeleton"
          class="space-y-4 rounded-md border border-border/60 bg-muted/20 p-5"
        >
          <Skeleton class="h-10 w-full rounded-md" />
          <Skeleton class="h-24 w-full rounded-md" />
        </div>

        <template v-else>
          <Alert v-if="!isAvailable" class="border-zinc-200 bg-zinc-50">
            <AlertTitle>
              {{ t("admin.gatewayLocationsSettings.unavailableTitle") }}
            </AlertTitle>
            <AlertDescription class="text-sm leading-6 text-zinc-700">
              {{ t("admin.gatewayLocationsSettings.unavailableDescription") }}
            </AlertDescription>
          </Alert>

          <button
            type="button"
            class="grid w-full gap-4 rounded-md border border-border/60 bg-background px-5 py-4 text-left transition-colors hover:border-primary/30 hover:bg-muted/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60 sm:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)_minmax(0,1fr)_5rem] sm:items-center"
            :disabled="!isAvailable || availableMappings.length === 0"
            :aria-label="
              t('admin.gatewayLocationsSettings.switchHostAria', {
                host:
                  selectedMapping?.host ||
                  t('admin.gatewayLocationsSettings.noHost'),
                title: getMappingTitleForDisplay(selectedMapping),
              })
            "
            @click="openHostPicker"
          >
            <span class="min-w-0 space-y-1">
              <span class="block text-xs font-medium text-muted-foreground">
                {{ t("admin.gatewayLocationsSettings.currentHost") }}
              </span>
              <span class="block truncate text-base font-semibold leading-6">
                {{
                  selectedMapping?.host ||
                  t("admin.gatewayLocationsSettings.noHost")
                }}
              </span>
              <span class="block truncate text-sm text-muted-foreground">
                {{
                  availableMappings.length > 0
                    ? t("admin.gatewayLocationsSettings.switchObject")
                    : t("admin.gatewayLocationsSettings.createHostHint")
                }}
              </span>
            </span>

            <span
              class="min-w-0 space-y-1 border-t border-border/60 pt-3 sm:border-l sm:border-t-0 sm:pl-5 sm:pt-0"
            >
              <span class="block text-xs font-medium text-muted-foreground">
                {{ t("admin.gatewayLocationsSettings.siteTitle") }}
              </span>
              <span class="flex min-w-0 items-center gap-2">
                <span class="truncate text-sm font-medium">
                  {{ getMappingTitleForDisplay(selectedMapping) }}
                </span>
              </span>
            </span>

            <span
              class="min-w-0 space-y-1 border-t border-border/60 pt-3 sm:border-l sm:border-t-0 sm:pl-5 sm:pt-0"
            >
              <span class="block text-xs font-medium text-muted-foreground">
                {{ t("admin.gatewayLocationsSettings.target") }}
              </span>
              <span class="block truncate text-sm font-medium">
                {{
                  selectedMapping?.target ||
                  t("admin.gatewayLocationsSettings.notSelected")
                }}
              </span>
            </span>

            <span
              class="space-y-1 border-t border-border/60 pt-3 sm:border-l sm:border-t-0 sm:pl-5 sm:pt-0 sm:text-right"
            >
              <span class="block text-xs font-medium text-muted-foreground">
                {{ t("admin.gatewayLocationsSettings.ruleCount") }}
              </span>
              <span class="block text-sm font-medium">
                {{ draftLocations.length }}
              </span>
            </span>
          </button>

          <div
            v-if="availableMappings.length === 0"
            class="rounded-md border px-5 py-8 text-center text-sm text-muted-foreground"
          >
            {{ t("admin.gatewayLocationsSettings.noMappings") }}
          </div>

          <div v-else class="overflow-hidden rounded-md border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>
                    {{ t("admin.gatewayLocationsSettings.match") }}
                  </TableHead>
                  <TableHead>
                    {{ t("admin.gatewayLocationsSettings.path") }}
                  </TableHead>
                  <TableHead>
                    {{ t("admin.gatewayLocationsSettings.action") }}
                  </TableHead>
                  <TableHead>
                    {{ t("admin.gatewayLocationsSettings.targetResponse") }}
                  </TableHead>
                  <TableHead>
                    {{ t("admin.gatewayLocationsSettings.processing") }}
                  </TableHead>
                  <TableHead class="text-right">
                    {{ t("admin.gatewayLocationsSettings.actions") }}
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow v-if="draftLocations.length === 0">
                  <TableCell
                    colspan="6"
                    class="py-8 text-center text-muted-foreground"
                  >
                    {{ t("admin.gatewayLocationsSettings.noRules") }}
                  </TableCell>
                </TableRow>
                <TableRow
                  v-for="{ location, index } in sortedDraftLocations"
                  :key="`${location.match}:${location.path}:${index}`"
                >
                  <TableCell class="text-sm font-medium">
                    {{
                      location.match === "exact"
                        ? t("admin.gatewayLocationsSettings.exactMatch")
                        : t("admin.gatewayLocationsSettings.prefixMatch")
                    }}
                  </TableCell>
                  <TableCell class="font-medium">{{ location.path }}</TableCell>
                  <TableCell>{{ formatAction(location) }}</TableCell>
                  <TableCell class="max-w-[22rem] truncate">
                    {{ formatTarget(location) }}
                  </TableCell>
                  <TableCell class="text-xs text-muted-foreground">
                    <template v-if="location.action === 'proxy'">
                      {{
                        location.strip_path
                          ? t("admin.gatewayLocationsSettings.stripPath")
                          : t("admin.gatewayLocationsSettings.keepPath")
                      }}
                      <template
                        v-if="!isWebSocketProxyTargetUrl(location.target)"
                      >
                        ·
                        {{
                          location.rewrite_html
                            ? t("admin.gatewayLocationsSettings.rewriteHtml")
                            : t("admin.gatewayLocationsSettings.noRewriteHtml")
                        }}
                      </template>
                    </template>
                    <template v-else>
                      {{
                        t(
                          "admin.gatewayLocationsSettings.responseHeadersCount",
                          {
                            count: Object.keys(location.response.headers || {})
                              .length,
                          },
                        )
                      }}
                    </template>
                  </TableCell>
                  <TableCell class="text-right">
                    <div class="flex justify-end gap-2">
                      <Button
                        variant="ghost"
                        size="icon"
                        @click="openEditDialog(index)"
                      >
                        <Pencil class="h-4 w-4" />
                        <span class="sr-only">
                          {{ t("admin.gatewayLocationsSettings.editRuleSr") }}
                        </span>
                      </Button>
                      <ConfirmDangerPopover
                        :title="
                          t('admin.gatewayLocationsSettings.deleteRuleTitle')
                        "
                        :description="
                          t(
                            'admin.gatewayLocationsSettings.deleteRuleDescription',
                            { path: location.path },
                          )
                        "
                        :confirm-text="
                          t('admin.gatewayLocationsSettings.confirmDelete')
                        "
                        :on-confirm="() => removeLocation(index)"
                        content-class="w-64 text-left"
                      >
                        <template #trigger>
                          <Button
                            variant="ghost"
                            size="icon"
                            class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                          >
                            <Trash2 class="h-4 w-4" />
                            <span class="sr-only">
                              {{
                                t("admin.gatewayLocationsSettings.deleteRuleSr")
                              }}
                            </span>
                          </Button>
                        </template>
                      </ConfirmDangerPopover>
                    </div>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>

          <FloatingActionDock
            :active="isDirty"
            inline-class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end"
          >
            <template #inline>
              <Button
                variant="outline"
                :disabled="!isDirty || isSaving"
                @click="resetDraftFromSelected"
              >
                {{ t("admin.gatewayLocationsSettings.discardChanges") }}
              </Button>
              <Button :disabled="!canSave" @click="saveLocations">
                {{ t("admin.gatewayLocationsSettings.saveLocations") }}
              </Button>
            </template>
          </FloatingActionDock>
        </template>
      </CardContent>
    </Card>

    <GatewayLocationHostPickerDialog
      :mappings="availableMappings"
      :open="isHostPickerOpen"
      :selected-host="selectedHost"
      :selected-mapping="selectedMapping"
      @select="selectHostFromDialog"
      @update:open="handleHostPickerOpenChange"
    />

    <GatewayLocationRuleDialog
      :open="isDialogOpen"
      :editing-index="editingIndex"
      :form="form"
      :form-error="formError"
      :is-proxy-location-web-socket-target="isProxyLocationWebSocketTarget"
      :is-saving="isSaving"
      @update:open="(open) => !open && closeDialog()"
      @add-header="addHeaderRow"
      @close="closeDialog"
      @remove-header="removeHeaderRow"
      @save="saveDialogLocation"
      @set-action="setAction"
    />
  </div>
</template>
