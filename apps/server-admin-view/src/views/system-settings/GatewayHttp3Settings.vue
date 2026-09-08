<script setup lang="ts">
import { computed, onMounted, reactive, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { RefreshCw } from "lucide-vue-next";
import {
  gatewayHttp3Api,
  type GatewayHttp3Status,
} from "@/lib/api/gateway-http3";

const { t } = useI18n();
const id = useId();
const status = ref<GatewayHttp3Status | null>(null);
const busy = ref(false);
const error = ref("");
const form = reactive({ enabled: false, advertised_port: 0 });
const valid = computed(
  () =>
    Number.isInteger(Number(form.advertised_port)) &&
    Number(form.advertised_port) >= 0 &&
    Number(form.advertised_port) <= 65535,
);
const dirty = computed(
  () =>
    status.value !== null &&
    (form.enabled !== status.value.enabled ||
      Number(form.advertised_port) !== status.value.advertised_port),
);
const stateLabel = computed(() => {
  const state = status.value?.state ?? "unknown";
  const supported = [
    "running",
    "disabled",
    "suspended_frp",
    "waiting_certificate",
    "waiting_bridge",
    "error",
  ];
  return t(
    `admin.gatewaySettings.http3.states.${supported.includes(state) ? state : "unknown"}`,
  );
});
function resetForm() {
  if (!status.value) return;
  form.enabled = status.value.enabled;
  form.advertised_port = status.value.advertised_port;
  error.value = "";
}
async function run(save: boolean) {
  if (busy.value || (save && (!dirty.value || !valid.value))) return;
  const preserveEdits = !save && dirty.value;
  busy.value = true;
  error.value = "";
  try {
    const next = save
      ? await gatewayHttp3Api.set({
          enabled: form.enabled,
          advertised_port: Number(form.advertised_port),
        })
      : await gatewayHttp3Api.get();
    status.value = next;
    if (!preserveEdits) resetForm();
    if (save) toast.success(t("admin.gatewaySettings.http3.saved"));
  } catch (cause) {
    error.value = extractErrorMessage(cause);
  } finally {
    busy.value = false;
  }
}
onMounted(() => {
  void run(false);
});
</script>

<template>
  <div class="space-y-6">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">{{
            t("admin.gatewayPortalSettings.systemSettings")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=gateway">{{
            t("admin.gatewayPortalSettings.gateway")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem
          ><BreadcrumbPage>{{
            t("admin.gatewaySettings.http3.title")
          }}</BreadcrumbPage></BreadcrumbItem
        >
      </BreadcrumbList>
    </Breadcrumb>
    <Card class="w-full border-border/60 shadow-none">
      <CardHeader
        class="flex flex-row items-start justify-between gap-6 space-y-0"
      >
        <div class="space-y-2">
          <CardTitle :id="`${id}-title`" class="text-xl">HTTP/3</CardTitle>
          <CardDescription>{{
            t("admin.gatewaySettings.http3.description")
          }}</CardDescription>
        </div>
        <Switch
          v-model="form.enabled"
          :aria-labelledby="`${id}-title`"
          :disabled="busy || !status"
          class="mt-1 shrink-0"
        />
      </CardHeader>
      <CardContent class="space-y-5 border-t pt-5">
        <div class="space-y-2">
          <div class="flex flex-wrap items-center gap-3">
            <Label :for="id">{{ t("admin.gatewaySettings.http3.port") }}</Label>
            <Input
              :id="id"
              v-model="form.advertised_port"
              type="number"
              min="0"
              max="65535"
              step="1"
              class="w-28"
              :disabled="busy || !status"
              :aria-describedby="`${id}-hint`"
            />
          </div>
          <p :id="`${id}-hint`" class="text-xs text-muted-foreground">
            {{ t("admin.gatewaySettings.http3.portHint") }}
          </p>
        </div>
        <p
          v-if="error || status?.error"
          role="alert"
          class="text-sm text-destructive"
        >
          {{ error || status?.error }}
        </p>
        <div class="space-y-2 border-t pt-4">
          <div class="flex flex-wrap items-center gap-2">
            <Badge variant="secondary">{{ stateLabel }}</Badge>
            <span
              v-if="status?.listen_addresses.length"
              class="break-all font-mono text-xs text-muted-foreground"
              >{{ status.listen_addresses.join(", ") }}</span
            >
            <Button
              size="icon"
              variant="ghost"
              class="h-7 w-7"
              :disabled="busy"
              :aria-label="t('admin.gatewaySettings.http3.refresh')"
              :title="t('admin.gatewaySettings.http3.refresh')"
              @click="run(false)"
              ><RefreshCw
                class="h-3.5 w-3.5"
                :class="{ 'animate-spin': busy }"
                aria-hidden="true"
            /></Button>
          </div>
          <p v-if="status" class="text-xs text-muted-foreground">
            {{
              t("admin.gatewaySettings.http3.metrics", {
                active: status.active_connections,
                failed: status.handshake_failures,
              })
            }}
          </p>
          <p class="text-xs text-muted-foreground">
            {{ t("admin.gatewaySettings.http3.reachability") }}
          </p>
        </div>
        <div class="flex justify-end gap-2 border-t pt-4">
          <Button
            v-if="dirty"
            variant="ghost"
            :disabled="busy"
            @click="resetForm"
            >{{ t("admin.gatewaySettings.http3.reset") }}</Button
          >
          <Button :disabled="busy || !dirty || !valid" @click="run(true)">{{
            t("admin.gatewaySettings.http3.save")
          }}</Button>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
