<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { CheckCircle2, RefreshCw, XCircle } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import {
  DDNSAPI,
  type DDNSHttpTransport,
  type DDNSUpdateProxyTestResult,
} from "@/lib/api/ddns";
import {
  updateProxyValidationKey,
  type DDNSUpdateProxyDraft,
} from "./ddns-proxy-model";

const props = defineProps<{
  draft: DDNSUpdateProxyDraft;
  transport: DDNSHttpTransport;
  disabled: boolean;
}>();
const emit = defineEmits<{ busy: [value: boolean] }>();
const { t } = useI18n();
const pending = ref(false);
const result = ref<DDNSUpdateProxyTestResult | null>(null);
const error = ref("");
let active: AbortController | null = null;
const invalid = computed(() =>
  updateProxyValidationKey({ ...props.draft, enabled: true }),
);
watch(pending, (value) => emit("busy", value), { flush: "sync" });
const reset = () => {
  active?.abort();
  active = null;
  pending.value = false;
  result.value = null;
  error.value = "";
};
watch([() => props.draft, () => props.transport], reset, {
  deep: true,
  flush: "sync",
});
onBeforeUnmount(reset);

const test = async () => {
  if (pending.value || props.disabled || invalid.value) return;
  const controller = new AbortController();
  active = controller;
  result.value = null;
  error.value = "";
  pending.value = true;
  const { enabled, url, username, password, clearPassword } = props.draft;
  try {
    const response = await DDNSAPI.testUpdateProxy(
      {
        updateProxy: { enabled, url, username, password, clearPassword },
        httpTransport: props.transport,
      },
      controller.signal,
    );
    if (!controller.signal.aborted) {
      result.value = response;
      error.value = response.success
        ? ""
        : response.error || t("admin.ddns.updateProxyTestFailed");
    }
  } catch (cause) {
    if (!controller.signal.aborted)
      error.value = extractErrorMessage(
        cause,
        t("admin.ddns.updateProxyTestFailed"),
      );
  } finally {
    if (active === controller) {
      active = null;
      pending.value = false;
    }
  }
};
</script>

<template>
  <div class="min-w-0 space-y-2 border-t pt-3">
    <Button
      type="button"
      variant="outline"
      class="min-h-11 w-full sm:w-auto"
      :disabled="disabled || pending || Boolean(invalid)"
      @click="test"
    >
      <RefreshCw class="h-4 w-4" :class="{ 'animate-spin': pending }" />
      {{
        t(
          pending
            ? "admin.ddns.updateProxyTesting"
            : "admin.ddns.updateProxyTest",
        )
      }}
    </Button>
    <p class="break-words text-xs leading-5 text-muted-foreground">
      {{ t("admin.ddns.updateProxyTestHint") }}
    </p>
    <p
      v-if="error"
      role="alert"
      class="flex min-w-0 items-start gap-2 break-words text-sm text-destructive"
    >
      <XCircle class="mt-0.5 h-4 w-4 shrink-0" /><span class="min-w-0">{{
        error
      }}</span>
    </p>
    <p
      v-else-if="result?.success"
      role="status"
      class="flex items-start gap-2 text-sm text-emerald-700 dark:text-emerald-400"
    >
      <CheckCircle2 class="mt-0.5 h-4 w-4 shrink-0" />{{
        t("admin.ddns.updateProxyTestSuccess", { ms: result.elapsedMs })
      }}
    </p>
  </div>
</template>
