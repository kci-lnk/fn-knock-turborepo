<script setup lang="ts">
import { computed, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Checkbox } from "@/components/ui/checkbox";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
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
function setSecurity(value: unknown) {
  if (
    typeof value !== "string" ||
    !["ssl_tls", "starttls", "none"].includes(value)
  )
    return;
  const ports: Record<string, number> = {
    ssl_tls: 465,
    starttls: 587,
    none: 25,
  };
  if (model.value.smtp.port === ports[model.value.smtp.security])
    model.value.smtp.port = ports[value]!;
  model.value.smtp.security = value;
}
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
  <fieldset class="space-y-5" :disabled="disabled || testing">
    <legend class="sr-only">
      {{ t("admin.maintenanceSettings.emailTitle") }}
    </legend>
    <div
      class="flex items-center justify-between gap-4 rounded-xl border bg-muted/10 p-5"
    >
      <Label :for="`${id}-enabled`" class="text-base">{{
        t("admin.maintenanceSettings.emailEnabled")
      }}</Label>
      <Switch
        :id="`${id}-enabled`"
        v-model="model.enabled"
        :disabled="disabled || testing"
      />
    </div>
    <Card class="border-border/60 shadow-none">
      <CardHeader
        ><CardTitle class="text-base">{{
          t("admin.maintenanceSettings.emailConnection")
        }}</CardTitle></CardHeader
      >
      <CardContent class="grid gap-5 sm:grid-cols-2">
        <div class="space-y-2 sm:col-span-2">
          <Label :for="`${id}-host`">{{
            t("admin.maintenanceSettings.email_host")
          }}</Label
          ><Input
            :id="`${id}-host`"
            v-model="model.smtp.host"
            placeholder="smtp.example.com"
            autocomplete="off"
          />
        </div>
        <div class="space-y-2">
          <Label :for="`${id}-security`">{{
            t("admin.maintenanceSettings.emailSecurity")
          }}</Label>
          <Select
            :model-value="model.smtp.security"
            :disabled="disabled || testing"
            @update:model-value="setSecurity"
          >
            <SelectTrigger :id="`${id}-security`" class="w-full"
              ><SelectValue
            /></SelectTrigger>
            <SelectContent
              ><SelectItem value="ssl_tls">TLS</SelectItem
              ><SelectItem value="starttls">STARTTLS</SelectItem
              ><SelectItem value="none">{{
                t("admin.maintenanceSettings.emailNoTls")
              }}</SelectItem></SelectContent
            >
          </Select>
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
          <Select
            v-model="model.smtp.auth_mode"
            :disabled="disabled || testing"
          >
            <SelectTrigger :id="`${id}-auth`" class="w-full"
              ><SelectValue
            /></SelectTrigger>
            <SelectContent
              ><SelectItem value="auto">{{
                t("admin.maintenanceSettings.emailAuto")
              }}</SelectItem
              ><SelectItem value="plain">PLAIN</SelectItem
              ><SelectItem value="login">LOGIN</SelectItem
              ><SelectItem value="none">{{
                t("admin.maintenanceSettings.emailNoAuth")
              }}</SelectItem></SelectContent
            >
          </Select>
        </div>
        <div class="space-y-2">
          <Label :for="`${id}-username`">{{
            t("admin.maintenanceSettings.email_username")
          }}</Label
          ><Input
            :id="`${id}-username`"
            v-model="model.smtp.username"
            autocomplete="off"
          />
        </div>
        <div class="space-y-3 sm:col-span-2">
          <Label :for="`${id}-password`">{{
            t("admin.maintenanceSettings.emailPassword")
          }}</Label>
          <Input
            :id="`${id}-password`"
            v-model="model.password"
            type="password"
            autocomplete="new-password"
            :disabled="disabled || testing || model.clear_password"
            :placeholder="
              model.password_configured
                ? t('admin.maintenanceSettings.emailPasswordSaved')
                : ''
            "
          />
          <div class="flex items-center gap-2">
            <Checkbox
              :id="`${id}-clear`"
              :model-value="model.clear_password === true"
              :disabled="disabled || testing"
              @update:model-value="
                model.clear_password = $event === true;
                model.password = undefined;
              "
            /><Label
              :for="`${id}-clear`"
              class="text-sm text-muted-foreground"
              >{{ t("admin.maintenanceSettings.emailClearPassword") }}</Label
            >
          </div>
        </div>
      </CardContent>
    </Card>
    <Card class="border-border/60 shadow-none">
      <CardHeader
        ><CardTitle class="text-base">{{
          t("admin.maintenanceSettings.emailAddresses")
        }}</CardTitle></CardHeader
      >
      <CardContent class="grid gap-5 sm:grid-cols-2">
        <div class="space-y-2">
          <Label :for="`${id}-from`">{{
            t("admin.maintenanceSettings.emailFrom")
          }}</Label
          ><Input
            :id="`${id}-from`"
            v-model="model.from_address"
            type="email"
          />
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
      </CardContent>
    </Card>
    <Card class="border-border/60 shadow-none">
      <CardHeader
        ><CardTitle class="text-base">{{
          t("admin.maintenanceSettings.emailLimits")
        }}</CardTitle></CardHeader
      >
      <CardContent class="grid gap-5 sm:grid-cols-2">
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
        <div
          class="flex flex-wrap items-center gap-3 border-t pt-4 sm:col-span-2"
        >
          <Button
            type="button"
            variant="outline"
            :disabled="
              disabled ||
              testing ||
              !isBackupEmailValid({ ...model, enabled: true })
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
          <p role="status" class="text-sm text-muted-foreground">
            {{ result }}
          </p>
        </div>
      </CardContent>
    </Card>
  </fieldset>
</template>
