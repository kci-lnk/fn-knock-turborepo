<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ShieldAlert, ShieldCheck } from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { ConfigAPI } from "../../lib/api";
import {
  dockerAdminPanelResetCommands,
  openWrtAdminPanelResetCommands,
} from "../../lib/docker-admin-panel-reset";
import { useConfigStore } from "../../store/config";
import { useDockerAdminAuthStore } from "../../store/dockerAdminAuth";

const configStore = useConfigStore();
const dockerAdminAuthStore = useDockerAdminAuthStore();
const { t } = useI18n();

const newPassword = ref("");
const confirmPassword = ref("");

const isOpenWrtMode = computed(() => configStore.isOpenWrtDeployment);
const resetSshCommand = computed(() =>
  isOpenWrtMode.value
    ? openWrtAdminPanelResetCommands.ssh
    : dockerAdminPanelResetCommands.ssh,
);
const openWrtResetCommand = openWrtAdminPanelResetCommands.reset;
const dockerComposeResetCommand = dockerAdminPanelResetCommands.compose;
const dockerExecResetCommand = dockerAdminPanelResetCommands.dockerExec;

const isPanelAuthMode = computed(
  () => configStore.isProtectedAdminPanelDeployment,
);
const isFormFilled = computed(
  () =>
    newPassword.value.trim().length > 0 &&
    confirmPassword.value.trim().length > 0,
);

const { isPending: isSaving, run: runSavePassword } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.panelSettings.updateFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.panelSettings.updatePasswordFailed"),
      ),
    });
  },
});

const resetForm = () => {
  newPassword.value = "";
  confirmPassword.value = "";
};

const savePassword = async () => {
  const password = newPassword.value.trim();
  const confirm = confirmPassword.value.trim();

  if (!password) {
    toast.error(t("admin.panelSettings.passwordRequired"));
    return;
  }
  if (password !== confirm) {
    toast.error(t("admin.panelSettings.passwordMismatch"));
    return;
  }

  const changed = await runSavePassword(async () => {
    await ConfigAPI.changeDockerAdminPassword(password);
    await dockerAdminAuthStore.bootstrap({ force: true });
    return true;
  });
  if (!changed) return;

  resetForm();
  toast.success(t("admin.panelSettings.passwordUpdated"), {
    description: t("admin.panelSettings.passwordUpdatedDescription"),
  });
};
</script>

<template>
  <div v-if="isPanelAuthMode" class="space-y-4">
    <Card>
      <CardHeader>
        <CardTitle>{{ t("admin.panelSettings.title") }}</CardTitle>
        <CardDescription>
          {{ t("admin.panelSettings.description") }}
        </CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="space-y-2">
          <Label for="docker-panel-password">
            {{ t("admin.panelSettings.newPassword") }}
          </Label>
          <Input
            id="docker-panel-password"
            v-model="newPassword"
            type="password"
            autocomplete="new-password"
            :placeholder="t('admin.panelSettings.newPasswordPlaceholder')"
            :disabled="isSaving"
          />
        </div>

        <div class="space-y-2">
          <Label for="docker-panel-password-confirm">
            {{ t("admin.panelSettings.confirmPassword") }}
          </Label>
          <Input
            id="docker-panel-password-confirm"
            v-model="confirmPassword"
            type="password"
            autocomplete="new-password"
            :placeholder="t('admin.panelSettings.confirmPasswordPlaceholder')"
            :disabled="isSaving"
            @keyup.enter="savePassword"
          />
        </div>

        <Alert>
          <ShieldCheck class="h-4 w-4" />
          <AlertTitle>{{ t("admin.panelSettings.passwordRulesTitle") }}</AlertTitle>
          <AlertDescription>
            {{ t("admin.panelSettings.passwordRulesDescription") }}
          </AlertDescription>
        </Alert>

        <div class="flex items-center gap-3">
          <Button :disabled="isSaving || !isFormFilled" @click="savePassword">
            <span
              v-if="isSaving"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{ t("admin.panelSettings.changePassword") }}
          </Button>
          <Button variant="outline" :disabled="isSaving" @click="resetForm">
            {{ t("admin.panelSettings.clear") }}
          </Button>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>{{ t("admin.panelSettings.forgotTitle") }}</CardTitle>
        <CardDescription>
          {{ t("admin.panelSettings.forgotDescription") }}
        </CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <Alert>
          <ShieldAlert class="h-4 w-4" />
          <AlertTitle>{{ t("admin.panelSettings.resetResultTitle") }}</AlertTitle>
          <AlertDescription>
            {{ t("admin.panelSettings.resetResultDescription") }}
          </AlertDescription>
        </Alert>

        <div class="space-y-2">
          <p class="text-sm font-medium">
            {{ t("admin.panelSettings.stepLoginHost") }}
          </p>
          <pre
            class="w-full max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/40 px-3 py-3 text-sm leading-6"
          ><code>{{ resetSshCommand }}</code></pre>
        </div>

        <div v-if="isOpenWrtMode" class="space-y-2">
          <p class="text-sm font-medium">
            {{ t("admin.panelSettings.stepOpenWrtReset") }}
          </p>
          <pre
            class="w-full max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/40 px-3 py-3 text-sm leading-6"
          ><code>{{ openWrtResetCommand }}</code></pre>
        </div>

        <template v-else>
          <div class="space-y-2">
            <p class="text-sm font-medium">
              {{ t("admin.panelSettings.stepCompose") }}
            </p>
            <pre
              class="w-full max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/40 px-3 py-3 text-sm leading-6"
            ><code>{{ dockerComposeResetCommand }}</code></pre>
          </div>

          <div class="space-y-2">
            <p class="text-sm font-medium">
              {{ t("admin.panelSettings.stepDockerExec") }}
            </p>
            <pre
              class="w-full max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/40 px-3 py-3 text-sm leading-6"
            ><code>{{ dockerExecResetCommand }}</code></pre>
          </div>
        </template>
      </CardContent>
    </Card>
  </div>
</template>
