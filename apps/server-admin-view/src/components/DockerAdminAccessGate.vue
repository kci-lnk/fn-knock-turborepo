<template>
  <div class="relative min-h-screen overflow-hidden bg-muted/40 p-4">
    <div
      class="theme-grid-background pointer-events-none absolute inset-0 z-0"
    ></div>
    <div
      class="fixed right-[calc(env(safe-area-inset-right)+1rem)] top-[calc(env(safe-area-inset-top)+1rem)] z-30"
    >
      <ThemeModeToggle />
    </div>

    <div
      class="relative z-10 flex min-h-[calc(100vh-2rem)] items-center justify-center"
    >
      <Card
        class="w-full max-w-[400px] border-border/70 bg-card/95 shadow-lg shadow-black/5"
      >
        <CardHeader class="space-y-2 pb-5 text-center">
          <CardTitle class="text-2xl font-semibold tracking-tight">
            {{ title }}
          </CardTitle>
          <CardDescription class="text-sm leading-6 sm:text-base">
            {{ description }}
          </CardDescription>
        </CardHeader>

        <CardContent class="pt-0">
          <form class="space-y-5" autocomplete="off" @submit.prevent="submit">
            <div class="space-y-3">
              <Input
                v-model="password"
                type="password"
                :placeholder="placeholder"
                :autocomplete="autocomplete"
                class="h-11 rounded-md"
                :disabled="loading"
              />

              <p
                v-if="helperText"
                class="text-xs leading-5 text-muted-foreground"
              >
                {{ helperText }}
              </p>

              <div
                v-if="errorMessage"
                class="rounded-md border border-destructive/20 bg-destructive/5 px-3 py-2 text-sm leading-6 text-destructive"
              >
                {{ errorMessage }}
              </div>

              <div
                v-if="showRememberMe"
                class="flex min-h-6 items-center justify-between gap-3"
              >
                <div class="flex min-w-0 items-center gap-2">
                  <Checkbox
                    id="dockerAdminRememberMe"
                    v-model="rememberMe"
                    :disabled="loading"
                    class="data-[state=checked]:border-primary data-[state=checked]:bg-primary"
                  />
                  <label
                    for="dockerAdminRememberMe"
                    class="cursor-pointer select-none text-sm leading-none text-muted-foreground transition-colors hover:text-foreground"
                  >
                    {{ t("admin.components.dockerAdminGate.rememberMe") }}
                  </label>
                </div>

                <Button
                  v-if="showForgotPassword"
                  type="button"
                  variant="link"
                  class="h-auto shrink-0 px-0 py-0 text-sm font-medium text-muted-foreground hover:text-foreground"
                  :disabled="loading"
                  @click="showResetDialog = true"
                >
                  {{ t("admin.components.dockerAdminGate.forgotPassword") }}
                </Button>
              </div>
            </div>

            <Button
              type="submit"
              class="h-11 w-full"
              :disabled="loading || !password.trim()"
            >
              <span
                v-if="loading"
                class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
              ></span>
              {{ actionLabel }}
            </Button>

            <Button
              v-if="showRetry"
              type="button"
              variant="outline"
              class="h-11 w-full"
              :disabled="loading"
              @click="$emit('retry')"
            >
              {{ t("admin.components.dockerAdminGate.retry") }}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>

    <Dialog :open="showResetDialog" @update:open="showResetDialog = $event">
      <DialogContent
        class="max-h-[calc(100vh-2rem)] min-w-0 overflow-y-auto sm:max-w-[560px]"
      >
        <DialogHeader>
          <DialogTitle>{{
            t("admin.components.dockerAdminGate.resetTitle")
          }}</DialogTitle>
          <DialogDescription>
            {{
              t(
                isWindowsMode
                  ? "admin.components.dockerAdminGate.resetDescriptionWindows"
                  : "admin.components.dockerAdminGate.resetDescription",
              )
            }}
          </DialogDescription>
        </DialogHeader>

        <div class="min-w-0 space-y-4">
          <div
            class="rounded-lg border border-border/70 bg-muted/40 px-3 py-3 text-sm leading-6"
          >
            {{ t("admin.components.dockerAdminGate.resetNotice") }}
          </div>

          <div v-if="isWindowsMode" class="min-w-0 space-y-2">
            <p class="text-sm font-medium">
              {{ t("admin.components.dockerAdminGate.resetStepWindows") }}
            </p>
            <pre
              class="w-full max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/40 px-3 py-3 text-sm leading-6"
            ><code>{{ windowsResetCommand }}</code></pre>
          </div>

          <template v-else>
            <div class="min-w-0 space-y-2">
              <p class="text-sm font-medium">
                {{
                  t(
                    isOpenWrtMode
                      ? "admin.components.dockerAdminGate.resetStepOpenWrtSsh"
                      : "admin.components.dockerAdminGate.resetStepSsh",
                  )
                }}
              </p>
              <pre
                class="w-full max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/40 px-3 py-3 text-sm leading-6"
              ><code>{{ resetSshCommand }}</code></pre>
            </div>

            <div v-if="isOpenWrtMode" class="min-w-0 space-y-2">
              <p class="text-sm font-medium">
                {{
                  t("admin.components.dockerAdminGate.resetStepOpenWrtCommand")
                }}
              </p>
              <pre
                class="w-full max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/40 px-3 py-3 text-sm leading-6"
              ><code>{{ openWrtResetCommand }}</code></pre>
            </div>

            <template v-else>
              <div class="min-w-0 space-y-2">
                <p class="text-sm font-medium">
                  {{ t("admin.components.dockerAdminGate.resetStepCompose") }}
                </p>
                <pre
                  class="w-full max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/40 px-3 py-3 text-sm leading-6"
                ><code>{{ dockerComposeResetCommand }}</code></pre>
              </div>

              <div class="min-w-0 space-y-2">
                <p class="text-sm font-medium">
                  {{
                    t("admin.components.dockerAdminGate.resetStepDockerExec")
                  }}
                </p>
                <pre
                  class="w-full max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/40 px-3 py-3 text-sm leading-6"
                ><code>{{ dockerExecResetCommand }}</code></pre>
              </div>
            </template>
          </template>
        </div>

        <DialogFooter>
          <Button type="button" @click="showResetDialog = false">{{
            t("admin.components.dockerAdminGate.acknowledge")
          }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { ThemeModeToggle } from "@/components/ui/theme-toggle";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import {
  dockerAdminPanelResetCommands,
  openWrtAdminPanelResetCommands,
  windowsAdminPanelResetCommands,
} from "../lib/docker-admin-panel-reset";
import type { DeploymentTarget } from "../types";

const props = defineProps<{
  mode: "setup" | "login";
  loading: boolean;
  errorMessage?: string;
  showRetry?: boolean;
  deploymentTarget?: DeploymentTarget;
}>();

const emit = defineEmits<{
  submit: [password: string, rememberMe: boolean];
  retry: [];
}>();

const password = ref("");
const rememberMe = ref(false);
const showResetDialog = ref(false);
const { t } = useI18n();
const isOpenWrtMode = computed(() => props.deploymentTarget === "openwrt");
const isWindowsMode = computed(() => props.deploymentTarget === "windows");
const resetSshCommand = computed(() =>
  isOpenWrtMode.value
    ? openWrtAdminPanelResetCommands.ssh
    : dockerAdminPanelResetCommands.ssh,
);
const openWrtResetCommand = openWrtAdminPanelResetCommands.reset;
const dockerComposeResetCommand = dockerAdminPanelResetCommands.compose;
const dockerExecResetCommand = dockerAdminPanelResetCommands.dockerExec;
const windowsResetCommand = windowsAdminPanelResetCommands.reset;

const title = computed(() =>
  props.mode === "setup"
    ? t("admin.components.dockerAdminGate.setupTitle")
    : t("admin.components.dockerAdminGate.loginTitle"),
);
const description = computed(() =>
  props.mode === "setup"
    ? t("admin.components.dockerAdminGate.setupDescription")
    : t("admin.components.dockerAdminGate.loginDescription"),
);
const helperText = computed(() =>
  props.mode === "setup"
    ? t("admin.components.dockerAdminGate.setupHelper")
    : "",
);
const actionLabel = computed(() =>
  props.mode === "setup"
    ? t("admin.components.dockerAdminGate.setupAction")
    : t("admin.components.dockerAdminGate.loginAction"),
);
const placeholder = computed(() =>
  props.mode === "setup"
    ? t("admin.components.dockerAdminGate.setupPlaceholder")
    : t("admin.components.dockerAdminGate.loginPlaceholder"),
);
const autocomplete = computed(() =>
  props.mode === "setup" ? "new-password" : "current-password",
);
const showForgotPassword = computed(() => props.mode === "login");
const showRememberMe = computed(() => props.mode === "login");

const submit = () => {
  emit("submit", password.value, showRememberMe.value && rememberMe.value);
};

watch(
  () => props.mode,
  () => {
    password.value = "";
    rememberMe.value = false;
  },
);

watch(
  () => props.loading,
  (loading) => {
    if (!loading && !props.errorMessage) {
      password.value = "";
    }
  },
);
</script>
