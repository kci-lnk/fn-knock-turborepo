<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { LoaderCircle } from "lucide-vue-next";
import { terminalAccessErrorKey } from "@/lib/api/terminal-access";
import { useTerminalAccessStore } from "@/store/terminal-access";
import WebTerminalAuthorized from "./WebTerminalAuthorized.vue";

const { t } = useI18n();
const router = useRouter();
const access = useTerminalAccessStore();
const checked = ref(false);
const error = ref("");
const checking = ref(false);
let disposed = false;
let timer: ReturnType<typeof setInterval> | undefined;
const back = () => router.push({ path: "/system", query: { tab: "features" } });
async function refresh() {
  if (checking.value) return;
  checking.value = true;
  error.value = "";
  try {
    await access.refresh();
    if (!disposed) checked.value = true;
  } catch (cause) {
    if (!disposed) {
      checked.value = false;
      error.value = t(terminalAccessErrorKey(cause));
    }
  } finally {
    checking.value = false;
  }
}
watch(
  () => access.status?.enabled,
  (enabled) => {
    if (enabled === false) void back();
  },
  { immediate: true },
);
onMounted(() => {
  void refresh();
  timer = setInterval(() => void refresh(), 5000);
});
onUnmounted(() => {
  disposed = true;
  clearInterval(timer);
});
</script>

<template>
  <WebTerminalAuthorized
    v-if="checked && access.isCurrent && access.status?.enabled"
    :key="access.status.revision"
  />
  <div v-else class="flex min-h-48 flex-col items-center justify-center gap-4">
    <p v-if="error" role="alert" class="text-sm text-destructive">
      {{ error }}
    </p>
    <p
      v-else
      role="status"
      class="flex items-center gap-2 text-sm text-muted-foreground"
    >
      <LoaderCircle class="size-4 animate-spin" aria-hidden="true" />
      {{ t("admin.webTerminalSettings.loading") }}
    </p>
    <div v-if="error" class="flex gap-2">
      <Button type="button" variant="outline" @click="back">{{
        t("admin.webTerminalSettings.cancel")
      }}</Button>
      <Button type="button" :disabled="checking" @click="refresh">{{
        t("admin.webTerminalSettings.retry")
      }}</Button>
    </div>
  </div>
</template>
