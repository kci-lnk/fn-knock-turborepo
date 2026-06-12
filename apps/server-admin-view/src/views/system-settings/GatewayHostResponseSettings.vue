<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
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
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI, SystemAPI } from "../../lib/api";
import {
  resolveExplicitPublicAccessEntryPort,
  shouldOmitPublicAccessEntryPort,
} from "../../lib/reverse-proxy-submode";
import { useConfigStore } from "../../store/config";
import type {
  GatewayHostResponseDetails,
  GatewayHostResponseItem,
} from "../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";

const details = ref<GatewayHostResponseDetails | null>(null);
const formItems = ref<GatewayHostResponseItem[]>([]);
const loadError = ref("");
const accessEntryPort = ref("7999");
const configStore = useConfigStore();

const cloneItem = (item: GatewayHostResponseItem): GatewayHostResponseItem => ({
  ...item,
});

const applyDetails = (value: GatewayHostResponseDetails) => {
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
      gateway_host_response: {
        disabled_hosts: [...value.config.disabled_hosts],
      },
    };
  }
};

const { isPending: isLoading, run: runLoad } = useAsyncAction({
  onError: (error) => {
    loadError.value = extractErrorMessage(error, "加载 Host 响应配置失败");
  },
});

const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "保存 Host 响应配置失败"),
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
    preserve_host: item.preserve_host,
  }));
  const saved = (details.value?.items ?? []).map((item) => ({
    host: item.host,
    preserve_host: item.preserve_host,
  }));

  return JSON.stringify(current) !== JSON.stringify(saved);
});
const saveBlockedReason = computed(() => {
  if (isAvailable.value) return "";
  return details.value?.availability.reason || "当前模式暂不可用";
});
const disabledHosts = computed(() =>
  formItems.value
    .filter((item) => item.preserve_host === false)
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

const fetchDetails = async () => {
  await runLoad(async () => {
    const value = await ConfigAPI.getGatewayHostResponse();
    loadError.value = "";
    applyDetails(value);
  });
};

const loadAccessEntryPort = async () => {
  try {
    const info = await SystemAPI.getAccessEntry();
    accessEntryPort.value = info.port.trim() || "7999";
  } catch (error) {
    console.warn("load access entry port failed:", error);
  }
};

const resetForm = () => {
  if (!details.value) return;
  formItems.value = details.value.items.map(cloneItem);
};

const updatePreserveHost = (host: string, nextValue: boolean) => {
  if (isSaving.value || !isAvailable.value) {
    return;
  }

  formItems.value = formItems.value.map((item) =>
    item.host === host
      ? {
          ...item,
          preserve_host: nextValue,
        }
      : item,
  );
};

const saveSettings = async () => {
  if (saveBlockedReason.value) {
    toast.error("暂时无法保存", {
      description: saveBlockedReason.value,
    });
    return;
  }

  await runSave(
    () =>
      ConfigAPI.updateGatewayHostResponse({
        disabled_hosts: disabledHosts.value,
      }),
    {
      onSuccess: (value) => {
        applyDetails(value);
        toast.success("Host 响应配置已更新并完成同步");
      },
    },
  );
};

onMounted(() => {
  void fetchDetails();
  void loadAccessEntryPort();
});
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">系统设置</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=gateway">网关</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>Host响应</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/60 shadow-none">
      <CardHeader class="space-y-3">
        <div class="space-y-1.5">
          <CardTitle class="text-xl">Host响应</CardTitle>
          <CardDescription class="leading-6">
            默认会向上游保留用户访问时的
            <code>Host</code>
            ，便于应用按外部域名生成跳转、Cookie
            或绝对链接。对于必须使用上游原始 Host 的目标，可以在这里单独关闭。
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
            <AlertTitle>当前模式暂不可用</AlertTitle>
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
                暂无可配置的子域映射，请先到“子域映射”添加应用映射。
              </div>

              <div v-else class="rounded-xl bg-muted/10">
                <Table>
                  <TableHeader>
                    <TableRow class="hover:bg-transparent">
                      <TableHead class="px-4 py-3">子域</TableHead>
                      <TableHead class="w-32 px-4 py-3 text-center">
                        Host
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
                            :model-value="item.preserve_host"
                            :disabled="isSaving || !isAvailable"
                            @update:model-value="
                              updatePreserveHost(item.host, $event === true)
                            "
                          />
                        </div>
                      </TableCell>
                    </TableRow>
                  </TableBody>
                </Table>
              </div>
            </section>

            <section class="space-y-4 border-t border-border/60 p-5">
              <div
                class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
              >
                <p class="text-sm leading-6 text-muted-foreground">
                  {{
                    saveBlockedReason || "保存后会立即同步到 Go 网关运行态。"
                  }}
                </p>
                <div class="flex gap-2 self-end">
                  <Button
                    variant="outline"
                    :disabled="isSaving || !isDirty"
                    @click="resetForm"
                  >
                    重置
                  </Button>
                  <Button
                    :disabled="isSaving || !isDirty || !!saveBlockedReason"
                    @click="saveSettings"
                  >
                    {{ isSaving ? "保存中..." : "保存并同步" }}
                  </Button>
                </div>
              </div>
            </section>
          </div>
        </template>
      </CardContent>
    </Card>
  </div>
</template>
