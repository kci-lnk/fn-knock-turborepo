<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { toast } from "@admin-shared/utils/toast";
import {
  resolveExplicitPublicAccessEntryPort,
  shouldOmitPublicAccessEntryPort,
} from "../../lib/reverse-proxy-submode";
import { useAccessEntryPort } from "../../composables/useAccessEntryPort";
import { useConfigStore } from "../../store/config";

type GatewayHostToggleField = "preserve_host" | "send_proxy_headers";
type GatewayHostConfigStoreKey =
  | "gateway_host_response"
  | "gateway_proxy_headers";

type GatewayHostToggleItem = {
  host: string;
  target: string;
  title: string;
} & Record<string, unknown>;

type GatewayHostToggleDetails = {
  availability: {
    available: boolean;
    reason: string;
  };
  config: {
    disabled_hosts: string[];
  };
  items: GatewayHostToggleItem[];
  summary: {
    disabled_count: number;
    total_count: number;
    updated_at: string | null;
  };
};

const props = defineProps<{
  configStoreKey: GatewayHostConfigStoreKey;
  descriptionCode: string;
  fetchDetails: () => Promise<GatewayHostToggleDetails>;
  messageKeyPrefix: string;
  saveDetails: (payload: {
    disabled_hosts: string[];
  }) => Promise<GatewayHostToggleDetails>;
  toggleColumnLabelKey: string;
  toggleField: GatewayHostToggleField;
}>();

const details = ref<GatewayHostToggleDetails | null>(null);
const formItems = ref<GatewayHostToggleItem[]>([]);
const loadError = ref("");
const { accessEntryPort, loadAccessEntryPort } = useAccessEntryPort();
const configStore = useConfigStore();
const { t } = useI18n();

const message = (key: string) => t(`${props.messageKeyPrefix}.${key}`);

const cloneItem = (item: GatewayHostToggleItem): GatewayHostToggleItem => ({
  ...item,
});

const getToggleValue = (item: GatewayHostToggleItem) =>
  item[props.toggleField] === true;

const applyDetails = (value: GatewayHostToggleDetails) => {
  details.value = {
    config: {
      disabled_hosts: [...value.config.disabled_hosts],
    },
    availability: {
      ...value.availability,
    },
    items: value.items.map(cloneItem),
    summary: {
      ...value.summary,
    },
  };
  formItems.value = value.items.map(cloneItem);

  if (configStore.config) {
    configStore.config = {
      ...configStore.config,
      [props.configStoreKey]: {
        disabled_hosts: [...value.config.disabled_hosts],
      },
    } as typeof configStore.config;
  }
};

const { isPending: isLoading, run: runLoad } = useAsyncAction({
  onError: (error) => {
    loadError.value = extractErrorMessage(error, message("loadDescription"));
  },
});

const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error(message("saveFailed"), {
      description: extractErrorMessage(error, message("saveDescription")),
    });
  },
});

const showLoadingSkeleton = useDelayedLoading(isLoading);
const isAvailable = computed(
  () => details.value?.availability.available === true,
);
const isDirty = computed(() => {
  const current = formItems.value.map((item) => ({
    host: item.host,
    value: getToggleValue(item),
  }));
  const saved = (details.value?.items ?? []).map((item) => ({
    host: item.host,
    value: getToggleValue(item),
  }));

  return JSON.stringify(current) !== JSON.stringify(saved);
});
const saveBlockedReason = computed(() => {
  if (isAvailable.value) return "";
  return details.value?.availability.reason || message("unavailable");
});
const disabledHosts = computed(() =>
  formItems.value
    .filter((item) => !getToggleValue(item))
    .map((item) => item.host),
);
const explicitAccessEntryPort = computed(() =>
  resolveExplicitPublicAccessEntryPort(configStore.config),
);
const displayAccessEntryPort = computed(() =>
  explicitAccessEntryPort.value
    ? String(explicitAccessEntryPort.value)
    : accessEntryPort.value.trim() || "7999",
);
const shouldOmitAccessEntryPort = computed(() => {
  if (
    shouldOmitPublicAccessEntryPort(configStore.config) &&
    !explicitAccessEntryPort.value
  ) {
    return true;
  }
  const parsedPort = Number.parseInt(displayAccessEntryPort.value, 10);
  return parsedPort === 80 || parsedPort === 443;
});
const formatHostWithAccessEntryPort = (host: string): string =>
  shouldOmitAccessEntryPort.value
    ? host
    : `${host}:${displayAccessEntryPort.value}`;

const fetchHostToggleDetails = async () => {
  await runLoad(async () => {
    const value = await props.fetchDetails();
    loadError.value = "";
    applyDetails(value);
  });
};

const resetForm = () => {
  if (!details.value) return;
  formItems.value = details.value.items.map(cloneItem);
};

const updateHostToggle = (host: string, nextValue: boolean) => {
  if (isSaving.value || !isAvailable.value) {
    return;
  }

  formItems.value = formItems.value.map((item) =>
    item.host === host
      ? {
          ...item,
          [props.toggleField]: nextValue,
        }
      : item,
  );
};

const saveSettings = async () => {
  if (saveBlockedReason.value) {
    toast.error(message("saveBlockedTitle"), {
      description: saveBlockedReason.value,
    });
    return;
  }

  await runSave(
    () =>
      props.saveDetails({
        disabled_hosts: disabledHosts.value,
      }),
    {
      onSuccess: (value) => {
        applyDetails(value);
        toast.success(message("updated"));
      },
    },
  );
};

onMounted(() => {
  void fetchHostToggleDetails();
  void loadAccessEntryPort();
});
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">{{
            t("admin.nav.systemSettings")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=gateway">{{
            t("admin.systemSettingsTabs.gateway")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{ message("title") }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/60 shadow-none">
      <CardHeader class="space-y-3">
        <div class="space-y-1.5">
          <CardTitle class="text-xl">
            {{ message("title") }}
          </CardTitle>
          <CardDescription class="leading-6">
            {{ message("descriptionPrefix") }}
            <code>{{ descriptionCode }}</code>
            {{ message("descriptionSuffix") }}
          </CardDescription>
        </div>
      </CardHeader>

      <CardContent class="space-y-6">
        <div
          v-if="isLoading && showLoadingSkeleton"
          class="space-y-4 rounded-xl border border-border/60 bg-muted/20 p-5"
        >
          <Skeleton class="h-16 w-full rounded-xl" />
          <Skeleton class="h-16 w-full rounded-xl" />
          <Skeleton class="h-16 w-full rounded-xl" />
        </div>

        <div
          v-else-if="loadError && !details"
          class="rounded-xl border border-destructive/25 bg-destructive/5 px-5 py-4 text-sm text-destructive"
        >
          {{ loadError }}
        </div>

        <template v-else-if="details">
          <Alert v-if="!isAvailable" class="border-zinc-200 bg-zinc-50">
            <AlertTitle>{{ message("unavailable") }}</AlertTitle>
            <AlertDescription class="text-sm leading-6 text-zinc-700">
              {{ details.availability.reason }}
            </AlertDescription>
          </Alert>

          <div class="overflow-hidden rounded-xl border border-border/60">
            <section class="space-y-4 p-5">
              <div
                v-if="formItems.length === 0"
                class="rounded-xl bg-muted/20 px-4 py-4 text-sm leading-6 text-muted-foreground"
              >
                {{ message("emptyMapping") }}
              </div>

              <div v-else class="rounded-xl bg-muted/10">
                <Table>
                  <TableHeader>
                    <TableRow class="hover:bg-transparent">
                      <TableHead class="px-4 py-3">
                        {{ message("subdomain") }}
                      </TableHead>
                      <TableHead class="w-32 px-4 py-3 text-center">
                        {{ message(toggleColumnLabelKey) }}
                      </TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    <TableRow
                      v-for="item in formItems"
                      :key="item.host"
                      class="hover:bg-muted/20"
                    >
                      <TableCell class="px-4 py-4 align-top">
                        <div class="min-w-0 space-y-1.5">
                          <div class="flex flex-wrap items-center gap-2">
                            <div class="break-all font-medium">
                              {{ formatHostWithAccessEntryPort(item.host) }}
                            </div>
                            <Badge
                              v-if="item.title"
                              variant="secondary"
                              class="max-w-full"
                            >
                              {{ item.title }}
                            </Badge>
                          </div>
                          <div class="break-all text-xs text-muted-foreground">
                            {{ item.target }}
                          </div>
                        </div>
                      </TableCell>
                      <TableCell class="px-4 py-4 text-center">
                        <div class="flex justify-center">
                          <Switch
                            :model-value="getToggleValue(item)"
                            :disabled="isSaving || !isAvailable"
                            @update:model-value="
                              updateHostToggle(item.host, $event === true)
                            "
                          />
                        </div>
                      </TableCell>
                    </TableRow>
                  </TableBody>
                </Table>
              </div>
            </section>

            <FloatingActionDock
              :active="isDirty"
              inline-class="space-y-4 border-t border-border/60 p-5"
            >
              <template #inline>
                <div
                  class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
                >
                  <p class="text-sm leading-6 text-muted-foreground">
                    {{ saveBlockedReason || message("saveHint") }}
                  </p>

                  <div class="flex flex-wrap items-center justify-end gap-3">
                    <Button
                      variant="outline"
                      :disabled="!isDirty || isSaving"
                      @click="resetForm"
                    >
                      {{ message("reset") }}
                    </Button>
                    <Button
                      :disabled="
                        !isDirty || isSaving || Boolean(saveBlockedReason)
                      "
                      @click="saveSettings"
                    >
                      <span
                        v-if="isSaving"
                        class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
                      ></span>
                      {{
                        isSaving ? message("saving") : message("saveAndSync")
                      }}
                    </Button>
                  </div>
                </div>
              </template>

              <template #floating>
                <Button
                  variant="outline"
                  :disabled="!isDirty || isSaving"
                  @click="resetForm"
                >
                  {{ message("reset") }}
                </Button>
                <Button
                  :disabled="!isDirty || isSaving || Boolean(saveBlockedReason)"
                  @click="saveSettings"
                >
                  <span
                    v-if="isSaving"
                    class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
                  ></span>
                  {{ isSaving ? message("saving") : message("saveAndSync") }}
                </Button>
              </template>
            </FloatingActionDock>
          </div>
        </template>
      </CardContent>
    </Card>
  </div>
</template>
