<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import PasswordInput from "@/components/DockerAdminPasswordInput.vue";
import { LoaderCircle } from "lucide-vue-next";
import { Label } from "@/components/ui/label";
import {
  TerminalAccessAPI,
  terminalAccessErrorKey,
} from "@/lib/api/terminal-access";
import { useTerminalAccessStore } from "@/store/terminal-access";
import WebTerminalAuthorized from "./WebTerminalAuthorized.vue";

const { t } = useI18n();
const router = useRouter();
const access = useTerminalAccessStore();
const checked = ref(false);
const password = ref("");
const error = ref("");
const verifying = ref(false);
const checking = ref(false);
let disposed = false;
let refreshGeneration = 0;
let timer: ReturnType<typeof setInterval> | undefined;
const back = () => router.push({ path: "/system", query: { tab: "features" } });
async function refresh(force = false) {
  if ((checking.value || verifying.value) && !force) return;
  const request = ++refreshGeneration;
  checking.value = true;
  try {
    const status = await access.refresh();
    if (!disposed && request === refreshGeneration) {
      checked.value = true;
      if (force && status.enabled && !status.authorized) {
        error.value = t("admin.webTerminalSettings.sessionUnavailable");
      }
    }
    return status;
  } catch (cause) {
    if (!disposed && request === refreshGeneration) {
      checked.value = false;
      error.value = t(terminalAccessErrorKey(cause));
    }
  } finally {
    if (request === refreshGeneration) checking.value = false;
  }
}
async function verify() {
  if (verifying.value || !password.value) return;
  verifying.value = true;
  error.value = "";
  try {
    await TerminalAccessAPI.verify(password.value);
    if (disposed) return;
    const status = await refresh(true);
    if (status?.authorized) password.value = "";
  } catch (cause) {
    error.value = t(terminalAccessErrorKey(cause));
  } finally {
    verifying.value = false;
  }
}
watch(
  () => access.status?.enabled,
  (enabled) => {
    if (enabled === false) void back();
  },
);
onMounted(() => {
  void refresh();
  timer = setInterval(() => {
    void refresh();
  }, 5000);
});
onUnmounted(() => {
  disposed = true;
  clearInterval(timer);
  password.value = "";
});
</script>

<template>
  <WebTerminalAuthorized
    v-if="checked && access.status?.enabled && access.status.authorized"
    :key="access.status.revision"
  />
  <div
    v-else
    class="flex min-h-[calc(100svh-10rem)] w-full items-center justify-center py-6"
  >
    <section
      class="w-full max-w-sm rounded-xl border border-border/60 bg-card p-5 sm:p-6"
    >
      <h2 class="mb-5 text-lg font-semibold">
        {{ t("admin.nav.webTerminal") }}
      </h2>
      <form
        class="space-y-4"
        :aria-busy="verifying || !checked"
        @submit.prevent="verify"
      >
        <div v-if="checked">
          <Label for="terminal-access-password" class="sr-only">{{
            t("admin.webTerminalSettings.password")
          }}</Label>
          <PasswordInput
            id="terminal-access-password"
            v-model="password"
            autocomplete="current-password"
            input-class="h-10 shadow-none"
            :placeholder="t('admin.webTerminalSettings.verifyPlaceholder')"
            :disabled="verifying"
            :aria-invalid="Boolean(error)"
            :aria-describedby="error ? 'terminal-access-error' : undefined"
            @update:model-value="error = ''"
          />
        </div>
        <p
          v-if="error"
          id="terminal-access-error"
          role="alert"
          class="text-sm leading-relaxed text-destructive"
        >
          {{ error }}
        </p>
        <p
          v-if="!checked"
          role="status"
          class="flex items-center gap-2 text-sm text-muted-foreground"
        >
          <LoaderCircle
            v-if="checking"
            class="size-4 animate-spin"
            aria-hidden="true"
          />
          {{ t("admin.webTerminalSettings.loading") }}
        </p>
        <div class="flex justify-end gap-2">
          <Button type="button" variant="outline" @click="back">{{
            t("admin.webTerminalSettings.cancel")
          }}</Button>
          <Button
            v-if="!checked"
            type="button"
            :disabled="checking"
            @click="refresh()"
            >{{ t("admin.webTerminalSettings.retry") }}</Button
          >
          <Button v-else type="submit" :disabled="verifying || !password">
            <LoaderCircle
              v-if="verifying"
              class="size-4 animate-spin"
              aria-hidden="true"
            />
            {{
              t(
                verifying
                  ? "admin.webTerminalSettings.verifying"
                  : "admin.webTerminalSettings.verify",
              )
            }}
          </Button>
        </div>
      </form>
    </section>
  </div>
</template>
