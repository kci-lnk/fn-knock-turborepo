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
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import RefreshButton from "@/components/RefreshButton.vue";
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
const loading = ref(false);
const saving = ref(false);
const error = ref("");
let disposed = false;
const dirty = computed(
  () => settings.value !== null && enabled.value !== settings.value.enabled,
);
const back = () => router.push({ path: "/system", query: { tab: "features" } });
function apply(value: WebTerminalSettings) {
  settings.value = value;
  enabled.value = value.enabled;
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
  saving.value = true;
  error.value = "";
  try {
    const value = await TerminalAccessAPI.update({
      enabled: enabled.value,
      revision: settings.value.revision,
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
onMounted(load);
onUnmounted(() => {
  disposed = true;
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
      <CardHeader class="space-y-4">
        <div
          class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between"
        >
          <div class="space-y-1.5">
            <CardTitle class="text-xl tracking-tight">{{
              t("admin.nav.webTerminal")
            }}</CardTitle>
            <CardDescription class="max-w-2xl leading-6">{{
              t("admin.webTerminalSettings.description")
            }}</CardDescription>
          </div>
          <RefreshButton
            :loading="loading"
            :disabled="saving || loading"
            @click="load"
          />
        </div>
      </CardHeader>
      <CardContent class="space-y-5">
        <p
          v-if="loading && !settings"
          role="status"
          class="text-sm text-muted-foreground"
        >
          {{ t("admin.webTerminalSettings.loading") }}
        </p>
        <div
          v-if="error"
          role="alert"
          class="rounded-xl border border-destructive/25 bg-destructive/5 px-5 py-4 text-sm text-destructive"
        >
          {{ error }}
        </div>
        <form v-if="settings" class="space-y-5" @submit.prevent="save">
          <div
            class="rounded-2xl border border-border/60 bg-muted/10 px-4 py-4"
          >
            <div class="flex items-start justify-between gap-4">
              <Label :for="`${id}-enabled`" class="text-base font-medium">{{
                t("admin.webTerminalSettings.enabled")
              }}</Label>
              <Switch
                :id="`${id}-enabled`"
                v-model="enabled"
                class="mt-0.5 shrink-0"
                :disabled="saving || loading"
              />
            </div>
          </div>
          <div class="overflow-hidden rounded-xl border border-border/60">
            <FloatingActionDock
              :active="Boolean(dirty)"
              inline-class="space-y-4 p-5"
            >
              <template #inline>
                <div
                  class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
                >
                  <p
                    v-if="!enabled"
                    class="text-sm leading-6 text-muted-foreground"
                  >
                    {{ t("admin.webTerminalSettings.enabledHint") }}
                  </p>
                  <div class="flex gap-3 sm:ml-auto">
                    <Button
                      type="button"
                      variant="outline"
                      :disabled="saving"
                      @click="back"
                      >{{ t("admin.webTerminalSettings.cancel") }}</Button
                    >
                    <Button
                      type="submit"
                      :disabled="!dirty || saving || loading"
                      >{{
                        t(
                          saving
                            ? "admin.webTerminalSettings.saving"
                            : "admin.webTerminalSettings.save",
                        )
                      }}</Button
                    >
                  </div>
                </div>
              </template>
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
          </div>
        </form>
      </CardContent>
    </Card>
  </div>
</template>
