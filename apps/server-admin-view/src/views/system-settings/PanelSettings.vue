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
import { Label } from "@/components/ui/label";
import { ShieldAlert, ShieldCheck } from "lucide-vue-next";
import DockerAdminPasswordInput from "../../components/DockerAdminPasswordInput.vue";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { ConfigAPI } from "@/lib/api/config";
import { resolveAdminPanelResetGuide } from "../../lib/docker-admin-panel-reset";
import {
  dockerAdminPasswordValidationMessageKeys,
  validateDockerAdminPassword,
} from "../../lib/docker-admin-password";
import { useConfigStore } from "../../store/config";
import { useDockerAdminAuthStore } from "../../store/dockerAdminAuth";

const configStore = useConfigStore();
const dockerAdminAuthStore = useDockerAdminAuthStore();
const { t } = useI18n();

const newPassword = ref("");
const confirmPassword = ref("");

const resetGuide = computed(() =>
  resolveAdminPanelResetGuide(configStore.runtimeProfile?.deployment_target),
);

const isPanelAuthMode = computed(
  () => configStore.isProtectedAdminPanelDeployment,
);
const passwordValidationError = computed(() =>
  newPassword.value ? validateDockerAdminPassword(newPassword.value) : null,
);
const newPasswordError = computed(() => {
  const error = passwordValidationError.value;
  return error ? t(dockerAdminPasswordValidationMessageKeys[error]) : "";
});
const confirmPasswordError = computed(() =>
  confirmPassword.value && confirmPassword.value !== newPassword.value
    ? t("admin.panelSettings.passwordMismatch")
    : "",
);
const isFormValid = computed(
  () =>
    newPassword.value.length > 0 &&
    confirmPassword.value.length > 0 &&
    !passwordValidationError.value &&
    newPassword.value === confirmPassword.value,
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
  const password = newPassword.value;
  const confirm = confirmPassword.value;

  if (!password) {
    toast.error(t("admin.panelSettings.passwordRequired"));
    return;
  }
  const validationError = validateDockerAdminPassword(password);
  if (validationError) {
    toast.error(t(dockerAdminPasswordValidationMessageKeys[validationError]));
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
      <CardContent>
        <form
          class="space-y-4"
          autocomplete="off"
          @submit.prevent="savePassword"
        >
          <div class="space-y-2">
            <Label for="docker-panel-password">
              {{ t("admin.panelSettings.newPassword") }}
            </Label>
            <DockerAdminPasswordInput
              id="docker-panel-password"
              v-model="newPassword"
              autocomplete="new-password"
              :placeholder="t('admin.panelSettings.newPasswordPlaceholder')"
              :disabled="isSaving"
              :aria-invalid="Boolean(newPasswordError)"
              :aria-describedby="
                newPasswordError ? 'docker-panel-password-error' : undefined
              "
            />
            <p
              v-if="newPasswordError"
              id="docker-panel-password-error"
              class="text-xs leading-5 text-destructive"
              role="alert"
            >
              {{ newPasswordError }}
            </p>
          </div>

          <div class="space-y-2">
            <Label for="docker-panel-password-confirm">
              {{ t("admin.panelSettings.confirmPassword") }}
            </Label>
            <DockerAdminPasswordInput
              id="docker-panel-password-confirm"
              v-model="confirmPassword"
              autocomplete="new-password"
              :placeholder="t('admin.panelSettings.confirmPasswordPlaceholder')"
              :disabled="isSaving"
              :aria-invalid="Boolean(confirmPasswordError)"
              :aria-describedby="
                confirmPasswordError
                  ? 'docker-panel-password-confirm-error'
                  : undefined
              "
            />
            <p
              v-if="confirmPasswordError"
              id="docker-panel-password-confirm-error"
              class="text-xs leading-5 text-destructive"
              role="alert"
            >
              {{ confirmPasswordError }}
            </p>
          </div>

          <Alert>
            <ShieldCheck class="h-4 w-4" />
            <AlertTitle>{{
              t("admin.panelSettings.passwordRulesTitle")
            }}</AlertTitle>
            <AlertDescription>
              {{ t("admin.panelSettings.passwordRulesDescription") }}
            </AlertDescription>
          </Alert>

          <div class="flex items-center gap-3">
            <Button type="submit" :disabled="isSaving || !isFormValid">
              <span
                v-if="isSaving"
                class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
              ></span>
              {{ t("admin.panelSettings.changePassword") }}
            </Button>
            <Button
              type="button"
              variant="outline"
              :disabled="isSaving"
              @click="resetForm"
            >
              {{ t("admin.panelSettings.clear") }}
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>

    <Card v-if="resetGuide">
      <CardHeader>
        <CardTitle>{{ t("admin.panelSettings.forgotTitle") }}</CardTitle>
        <CardDescription>
          {{ t(resetGuide.descriptionKey) }}
        </CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <Alert>
          <ShieldAlert class="h-4 w-4" />
          <AlertTitle>{{
            t("admin.panelSettings.resetResultTitle")
          }}</AlertTitle>
          <AlertDescription>
            {{ t("admin.panelSettings.resetResultDescription") }}
          </AlertDescription>
        </Alert>

        <div
          v-for="step in resetGuide.steps"
          :key="step.labelKey"
          class="space-y-2"
        >
          <p class="text-sm font-medium">
            {{ t(step.labelKey) }}
          </p>
          <pre
            class="w-full max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/40 px-3 py-3 text-sm leading-6"
          ><code>{{ step.command }}</code></pre>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
