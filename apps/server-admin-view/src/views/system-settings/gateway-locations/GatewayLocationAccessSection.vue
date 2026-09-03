<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { GatewayLocationForm } from "./gatewayLocationModel";

defineProps<{ form: GatewayLocationForm }>();
const { t } = useI18n();
</script>

<template>
  <div class="py-5">
    <div
      class="grid gap-3 sm:grid-cols-[18rem_minmax(0,1fr)] sm:items-start sm:gap-4"
    >
      <div class="space-y-2">
        <Label for="location-auth-mode">
          {{ t("admin.gatewayLocationsSettings.authBehavior") }}
        </Label>
        <Select v-model="form.auth_mode">
          <SelectTrigger id="location-auth-mode" class="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="inherit">
              {{ t("admin.gatewayLocationsSettings.authInherit") }}
            </SelectItem>
            <SelectItem value="public">
              {{ t("admin.gatewayLocationsSettings.authPublic") }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      <p class="text-xs leading-5 text-muted-foreground sm:mt-7">
        {{
          form.auth_mode === "public"
            ? t("admin.gatewayLocationsSettings.authPublicDescription")
            : t("admin.gatewayLocationsSettings.authInheritDescription")
        }}
      </p>
    </div>
  </div>
</template>
