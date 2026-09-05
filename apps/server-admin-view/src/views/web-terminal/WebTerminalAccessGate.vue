<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
  if (checking.value && !force) return;
  const request = ++refreshGeneration;
  checking.value = true;
  try {
    await access.refresh();
    if (!disposed && request === refreshGeneration) checked.value = true;
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
    password.value = "";
    await refresh(true);
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
  <Card v-else class="mx-auto w-full max-w-lg">
    <CardHeader>
      <CardTitle>{{ t("admin.nav.webTerminal") }}</CardTitle>
      <CardDescription>{{
        t("admin.webTerminalSettings.verifyHint")
      }}</CardDescription>
    </CardHeader>
    <CardContent>
      <form class="space-y-4" @submit.prevent="verify">
        <p v-if="error" role="alert" class="text-sm text-destructive">
          {{ error }}
        </p>
        <p v-if="!checked" role="status" class="text-sm text-muted-foreground">
          {{ t("admin.webTerminalSettings.loading") }}
        </p>
        <template v-else>
          <Label for="terminal-access-password">{{
            t("admin.webTerminalSettings.password")
          }}</Label>
          <Input
            id="terminal-access-password"
            v-model="password"
            type="password"
            autocomplete="current-password"
            :disabled="verifying"
          />
        </template>
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
          <Button v-else type="submit" :disabled="verifying || !password">{{
            t("admin.webTerminalSettings.verify")
          }}</Button>
        </div>
      </form>
    </CardContent>
  </Card>
</template>
