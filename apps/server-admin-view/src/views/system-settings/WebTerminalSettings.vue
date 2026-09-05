<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, useId } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
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
import RefreshButton from "@/components/RefreshButton.vue";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import { toast } from "@admin-shared/utils/toast";
import {
  TerminalAccessAPI,
  terminalAccessErrorKey,
  type WebTerminalSettings,
} from "@/lib/api/terminal-access";
import { useTerminalAccessStore } from "@/store/terminal-access";

const { t } = useI18n();
const router = useRouter();
const access = useTerminalAccessStore();
const id = useId();
const settings = ref<WebTerminalSettings | null>(null);
const enabled = ref(true);
const password = ref("");
const clearPassword = ref(false);
const loading = ref(false);
const saving = ref(false);
const error = ref("");
let disposed = false;
const dirty = computed(
  () =>
    settings.value &&
    (enabled.value !== settings.value.enabled ||
      (enabled.value && (password.value !== "" || clearPassword.value))),
);
const back = () => router.push({ path: "/system", query: { tab: "features" } });
function apply(value: WebTerminalSettings) {
  settings.value = value;
  enabled.value = value.enabled;
  password.value = "";
  clearPassword.value = false;
  access.applySettings(value);
}
async function load() {
  if (loading.value || saving.value) return;
  loading.value = true;
  error.value = "";
  try {
    const value = await TerminalAccessAPI.settings();
    if (!disposed) apply(value);
  } catch (cause) {
    error.value = t(terminalAccessErrorKey(cause));
  } finally {
    loading.value = false;
  }
}
async function save() {
  if (!settings.value || loading.value || saving.value || !dirty.value) return;
  if (enabled.value && new TextEncoder().encode(password.value).length > 128) {
    error.value = t("admin.webTerminalSettings.passwordTooLong");
    return;
  }
  saving.value = true;
  error.value = "";
  try {
    const value = await TerminalAccessAPI.update({
      enabled: enabled.value,
      revision: settings.value.revision,
      password: enabled.value && password.value ? password.value : undefined,
      clearPassword: enabled.value && clearPassword.value,
    });
    if (!disposed) {
      apply(value);
      toast.success(t("admin.webTerminalSettings.saved"));
    }
  } catch (cause) {
    error.value = t(terminalAccessErrorKey(cause));
  } finally {
    saving.value = false;
  }
}
function clear() {
  password.value = "";
  clearPassword.value = true;
}
onMounted(load);
onUnmounted(() => {
  disposed = true;
  password.value = "";
});
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb
      ><BreadcrumbList>
        <BreadcrumbItem
          ><BreadcrumbLink href="#/system">{{
            t("admin.smartConnectSettings.systemSettings")
          }}</BreadcrumbLink></BreadcrumbItem
        >
        <BreadcrumbSeparator />
        <BreadcrumbItem
          ><BreadcrumbLink href="#/system?tab=features">{{
            t("admin.smartConnectSettings.features")
          }}</BreadcrumbLink></BreadcrumbItem
        >
        <BreadcrumbSeparator />
        <BreadcrumbItem
          ><BreadcrumbPage>{{
            t("admin.nav.webTerminal")
          }}</BreadcrumbPage></BreadcrumbItem
        >
      </BreadcrumbList></Breadcrumb
    >
    <Card class="border-border/50 shadow-none">
      <CardHeader class="flex flex-row items-start justify-between gap-4">
        <div class="space-y-1.5">
          <CardTitle class="text-xl tracking-tight">{{
            t("admin.nav.webTerminal")
          }}</CardTitle>
          <CardDescription>{{
            t("admin.webTerminalSettings.description")
          }}</CardDescription>
        </div>
        <RefreshButton
          :loading="loading"
          :disabled="saving || loading"
          @click="load"
        />
      </CardHeader>
      <CardContent class="space-y-5">
        <p v-if="error" role="alert" class="text-sm text-destructive">
          {{ error }}
        </p>
        <p
          v-if="loading && !settings"
          role="status"
          class="text-sm text-muted-foreground"
        >
          {{ t("admin.webTerminalSettings.loading") }}
        </p>
        <form v-if="settings" class="space-y-5" @submit.prevent="save">
          <div class="rounded-2xl border border-border/60 bg-muted/10 p-4">
            <div class="flex items-start justify-between gap-4">
              <div class="space-y-2">
                <Label :for="`${id}-enabled`" class="text-base font-medium">{{
                  t("admin.webTerminalSettings.enabled")
                }}</Label>
                <p class="text-sm text-muted-foreground">
                  {{ t("admin.webTerminalSettings.enabledHint") }}
                </p>
              </div>
              <Switch
                :id="`${id}-enabled`"
                v-model="enabled"
                :disabled="saving || loading"
              />
            </div>
          </div>
          <section
            v-if="enabled"
            class="space-y-4 rounded-xl border border-border/60 p-5"
          >
            <div class="space-y-1">
              <Label :for="`${id}-password`">{{
                t("admin.webTerminalSettings.password")
              }}</Label>
              <p class="text-sm text-muted-foreground">
                {{ t("admin.webTerminalSettings.passwordHint") }}
              </p>
              <p class="text-sm" role="status">
                {{
                  t(
                    clearPassword
                      ? "admin.webTerminalSettings.clearPending"
                      : settings.passwordConfigured
                        ? "admin.webTerminalSettings.configured"
                        : "admin.webTerminalSettings.notConfigured",
                  )
                }}
              </p>
            </div>
            <Input
              :id="`${id}-password`"
              v-model="password"
              type="password"
              autocomplete="new-password"
              :disabled="saving || loading"
              :placeholder="t('admin.webTerminalSettings.passwordPlaceholder')"
              @update:model-value="clearPassword = false"
            />
            <Button
              v-if="settings.passwordConfigured"
              type="button"
              variant="outline"
              :disabled="saving || loading || clearPassword"
              @click="clear"
              >{{ t("admin.webTerminalSettings.clear") }}</Button
            >
          </section>
          <FloatingActionDock :active="Boolean(dirty)">
            <template #inline
              ><div class="flex justify-end gap-3">
                <Button
                  type="button"
                  variant="outline"
                  :disabled="saving"
                  @click="back"
                  >{{ t("admin.webTerminalSettings.cancel") }}</Button
                >
                <Button type="submit" :disabled="!dirty || saving || loading">{{
                  t(
                    saving
                      ? "admin.webTerminalSettings.saving"
                      : "admin.webTerminalSettings.save",
                  )
                }}</Button>
              </div></template
            >
            <template #floating>
              <Button
                type="button"
                variant="outline"
                :disabled="saving"
                @click="back"
                >{{ t("admin.webTerminalSettings.cancel") }}</Button
              >
              <Button
                type="button"
                :disabled="!dirty || saving || loading"
                @click="save"
                >{{
                  t(
                    saving
                      ? "admin.webTerminalSettings.saving"
                      : "admin.webTerminalSettings.save",
                  )
                }}</Button
              >
            </template>
          </FloatingActionDock>
        </form>
      </CardContent>
    </Card>
  </div>
</template>
