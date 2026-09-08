<script setup lang="ts">
import { computed, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { MaintenanceAPI } from "@/lib/api/config";
import { isBackupEmailValid, type BackupEmailForm } from "@/lib/backup-email";
const model = defineModel<BackupEmailForm>({ required: true });
defineProps<{ disabled: boolean }>();
const { t } = useI18n();
const id = useId();
const testing = ref(false);
const result = ref("");
const recipients = computed({
  get: () => model.value.to_addresses.join(", "),
  set: (value: string) => {
    model.value.to_addresses = value
      .split(/[,;\n]/)
      .map((address) => address.trim())
      .filter(Boolean);
  },
});
const fields = ["host", "username"] as const;
async function test() {
  testing.value = true;
  result.value = "";
  try {
    await MaintenanceAPI.testBackupEmail(model.value);
    result.value = t("admin.maintenanceSettings.emailTestSuccess");
  } catch {
    result.value = t("admin.maintenanceSettings.emailTestFailed");
  } finally {
    testing.value = false;
  }
}
</script>
<template>
  <fieldset
    class="mt-6 space-y-4 border-t pt-6"
    :disabled="disabled || testing"
  >
    <legend class="text-sm font-medium">
      {{ t("admin.maintenanceSettings.emailTitle") }}
    </legend>
    <div class="flex items-center gap-3">
      <Switch
        :id="`${id}-enabled`"
        v-model="model.enabled"
        :disabled="disabled || testing"
      />
      <Label :for="`${id}-enabled`">{{
        t("admin.maintenanceSettings.emailEnabled")
      }}</Label>
    </div>
    <p class="text-xs text-muted-foreground">
      {{ t("admin.maintenanceSettings.emailDescription") }}
    </p>
    <div class="grid gap-4 sm:grid-cols-2">
      <div v-for="field in fields" :key="field" class="space-y-2">
        <Label :for="`${id}-${field}`">{{
          t(`admin.maintenanceSettings.email_${field}`)
        }}</Label>
        <Input
          :id="`${id}-${field}`"
          v-model="model.smtp[field]"
          autocomplete="off"
        />
      </div>
      <div class="space-y-2">
        <Label :for="`${id}-security`">{{
          t("admin.maintenanceSettings.emailSecurity")
        }}</Label>
        <select
          :id="`${id}-security`"
          v-model="model.smtp.security"
          class="h-9 w-full rounded-md border bg-background px-3"
          @change="
            model.smtp.port =
              model.smtp.security === 'ssl_tls'
                ? 465
                : model.smtp.security === 'starttls'
                  ? 587
                  : 25
          "
        >
          <option value="ssl_tls">TLS</option>
          <option value="starttls">STARTTLS</option>
          <option value="none">
            {{ t("admin.maintenanceSettings.emailNoTls") }}
          </option>
        </select>
      </div>
      <div class="space-y-2">
        <Label :for="`${id}-port`">{{
          t("admin.maintenanceSettings.emailPort")
        }}</Label
        ><Input
          :id="`${id}-port`"
          v-model.number="model.smtp.port"
          type="number"
          min="1"
          max="65535"
        />
      </div>
      <div class="space-y-2">
        <Label :for="`${id}-auth`">{{
          t("admin.maintenanceSettings.emailAuth")
        }}</Label>
        <select
          :id="`${id}-auth`"
          v-model="model.smtp.auth_mode"
          class="h-9 w-full rounded-md border bg-background px-3"
        >
          <option value="auto">
            {{ t("admin.maintenanceSettings.emailAuto") }}
          </option>
          <option value="plain">PLAIN</option>
          <option value="login">LOGIN</option>
          <option value="none">
            {{ t("admin.maintenanceSettings.emailNoAuth") }}
          </option>
        </select>
      </div>
      <div class="space-y-2">
        <Label :for="`${id}-password`">{{
          t("admin.maintenanceSettings.emailPassword")
        }}</Label>
        <Input
          :id="`${id}-password`"
          v-model="model.password"
          type="password"
          autocomplete="new-password"
          :disabled="model.clear_password"
          :placeholder="
            model.password_configured
              ? t('admin.maintenanceSettings.emailPasswordSaved')
              : ''
          "
        />
        <div class="flex items-center gap-2">
          <input
            :id="`${id}-clear`"
            v-model="model.clear_password"
            type="checkbox"
            @change="model.password = undefined"
          /><Label :for="`${id}-clear`">{{
            t("admin.maintenanceSettings.emailClearPassword")
          }}</Label>
        </div>
      </div>
      <div class="space-y-2">
        <Label :for="`${id}-from`">{{
          t("admin.maintenanceSettings.emailFrom")
        }}</Label
        ><Input :id="`${id}-from`" v-model="model.from_address" type="email" />
      </div>
      <div class="space-y-2">
        <Label :for="`${id}-name`">{{
          t("admin.maintenanceSettings.emailName")
        }}</Label
        ><Input :id="`${id}-name`" v-model="model.from_name" />
      </div>
      <div class="space-y-2 sm:col-span-2">
        <Label :for="`${id}-to`">{{
          t("admin.maintenanceSettings.emailTo")
        }}</Label
        ><Input :id="`${id}-to`" v-model="recipients" />
      </div>
      <div class="space-y-2">
        <Label :for="`${id}-timeout`">{{
          t("admin.maintenanceSettings.emailTimeout")
        }}</Label
        ><Input
          :id="`${id}-timeout`"
          v-model.number="model.smtp.timeout_seconds"
          type="number"
          min="1"
          max="120"
        />
      </div>
      <div class="space-y-2">
        <Label :for="`${id}-limit`">{{
          t("admin.maintenanceSettings.emailLimit")
        }}</Label
        ><Input
          :id="`${id}-limit`"
          v-model.number="model.attachment_limit_mib"
          type="number"
          min="1"
          max="100"
        />
      </div>
    </div>
    <Button
      type="button"
      variant="outline"
      :disabled="
        disabled || testing || !isBackupEmailValid({ ...model, enabled: true })
      "
      @click="test"
      >{{
        t(
          testing
            ? "admin.maintenanceSettings.emailTesting"
            : "admin.maintenanceSettings.emailTest",
        )
      }}</Button
    >
    <p role="status" class="text-sm">{{ result }}</p>
  </fieldset>
</template>
