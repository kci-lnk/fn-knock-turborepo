<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { GatewayLocationForm } from "./gatewayLocationModel";

const props = defineProps<{ form: GatewayLocationForm }>();
const { t } = useI18n();

const isExactMatch = computed(() => props.form.match === "exact");
const pathLabel = computed(() =>
  t(
    isExactMatch.value
      ? "admin.gatewayLocationsSettings.exactPath"
      : "admin.gatewayLocationsSettings.pathPrefix",
  ),
);
const pathDescription = computed(() =>
  t(
    isExactMatch.value
      ? "admin.gatewayLocationsSettings.exactPathDescription"
      : "admin.gatewayLocationsSettings.pathPrefixDescription",
  ),
);
const pathPlaceholder = computed(() =>
  isExactMatch.value ? "/api/status" : "/api",
);
</script>

<template>
  <section
    aria-labelledby="location-match-heading"
    class="space-y-4 rounded-lg border border-border/60 p-4"
  >
    <div class="space-y-1">
      <h3 id="location-match-heading" class="text-sm font-semibold">
        {{ t("admin.gatewayLocationsSettings.requestMatchSection") }}
      </h3>
      <p class="text-xs leading-5 text-muted-foreground">
        {{ t("admin.gatewayLocationsSettings.requestMatchDescription") }}
      </p>
    </div>

    <div class="grid gap-4 sm:grid-cols-[13rem_minmax(0,1fr)]">
      <div class="space-y-2">
        <Label for="location-match">
          {{ t("admin.gatewayLocationsSettings.matchMethod") }}
        </Label>
        <Select v-model="form.match">
          <SelectTrigger id="location-match" class="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="exact">
              {{ t("admin.gatewayLocationsSettings.exactMatch") }}
            </SelectItem>
            <SelectItem value="prefix">
              {{ t("admin.gatewayLocationsSettings.prefixMatch") }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div class="space-y-2">
        <Label for="location-path">{{ pathLabel }}</Label>
        <Input
          id="location-path"
          v-model="form.path"
          :placeholder="pathPlaceholder"
          aria-describedby="location-path-description"
        />
        <p
          id="location-path-description"
          class="text-xs leading-5 text-muted-foreground"
        >
          {{ pathDescription }}
        </p>
      </div>
    </div>
  </section>
</template>
