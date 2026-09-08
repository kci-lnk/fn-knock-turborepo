<script setup lang="ts">
import { computed, onMounted, reactive, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import FeatureSwitchRow from "./FeatureSwitchRow.vue";
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
async function run(save: boolean) {
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
    form.enabled = next.enabled;
    form.advertised_port = next.advertised_port;
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
  <section
    class="space-y-4 py-4"
    :aria-label="t('admin.gatewaySettings.http3.title')"
  >
    <FeatureSwitchRow
      :model-value="form.enabled"
      :title="t('admin.gatewaySettings.http3.title')"
      :description="t('admin.gatewaySettings.http3.description')"
      :disabled="busy || !status"
      @change="form.enabled = $event"
    />
    <div class="space-y-3 px-6">
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
        />
      </div>
      <p class="text-sm text-muted-foreground">
        {{ t("admin.gatewaySettings.http3.portHint") }}
      </p>
      <p v-if="status" class="text-sm">
        {{ stateLabel }}
        <span class="break-all">{{ status.listen_addresses.join(", ") }}</span>
      </p>
      <p class="text-sm text-muted-foreground">
        {{ t("admin.gatewaySettings.http3.reachability") }}
      </p>
      <p v-if="status" class="text-sm text-muted-foreground">
        {{
          t("admin.gatewaySettings.http3.metrics", {
            active: status.active_connections,
            failed: status.handshake_failures,
          })
        }}
      </p>
      <p
        v-if="error || status?.error"
        role="alert"
        class="text-sm text-destructive"
      >
        {{ error || status?.error }}
      </p>
      <div class="flex gap-2">
        <Button
          size="sm"
          :disabled="busy || !dirty || !valid"
          @click="run(true)"
          >{{ t("admin.gatewaySettings.http3.save") }}</Button
        >
        <Button
          size="sm"
          variant="outline"
          :disabled="busy"
          @click="run(false)"
          >{{ t("admin.gatewaySettings.http3.refresh") }}</Button
        >
      </div>
    </div>
  </section>
</template>
