<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Clock3 } from "lucide-vue-next";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { AdvancedAuthConfig } from "../../types";
import {
  advancedAuthHourInputToSeconds,
  MAX_ADVANCED_AUTH_IDLE_TTL_HOURS,
  MAX_ADVANCED_AUTH_IDLE_TTL_SECONDS,
  MAX_ADVANCED_AUTH_LIFETIME_HOURS,
  MAX_ADVANCED_AUTH_LIFETIME_SECONDS,
  MIN_ADVANCED_AUTH_TTL_HOURS,
  SECONDS_PER_MINUTE,
  secondsToAdvancedAuthHourInput,
} from "./advanced-auth-form";

const { form, saving } = defineProps<{
  form: AdvancedAuthConfig;
  saving: boolean;
}>();
const { t } = useI18n();

const idleHours = computed({
  get: () => secondsToAdvancedAuthHourInput(form.idle_ttl_seconds),
  set: (value: number) => {
    form.idle_ttl_seconds = advancedAuthHourInputToSeconds(
      value,
      MAX_ADVANCED_AUTH_IDLE_TTL_SECONDS,
    );
  },
});
const maxLifetimeHours = computed({
  get: () => secondsToAdvancedAuthHourInput(form.max_lifetime_seconds),
  set: (value: number) => {
    form.max_lifetime_seconds = advancedAuthHourInputToSeconds(
      value,
      MAX_ADVANCED_AUTH_LIFETIME_SECONDS,
    );
  },
});

const formatGrantDuration = (seconds: number) => {
  const minutes = Math.max(5, Math.round(seconds / SECONDS_PER_MINUTE));
  if (minutes % (24 * 60) === 0) {
    return t("admin.advancedAuth.durationDaysWithHours", {
      days: minutes / (24 * 60),
      hours: minutes / 60,
    });
  }
  if (minutes % 60 === 0) {
    return t("admin.advancedAuth.durationHours", { hours: minutes / 60 });
  }
  if (minutes < 60) {
    return t("admin.advancedAuth.durationMinutes", { minutes });
  }
  return t("admin.advancedAuth.durationHoursMinutes", {
    hours: Math.floor(minutes / 60),
    minutes: minutes % 60,
  });
};

const idleDurationText = computed(() =>
  formatGrantDuration(form.idle_ttl_seconds),
);
const maxLifetimeDurationText = computed(() =>
  formatGrantDuration(form.max_lifetime_seconds),
);
const maxLifetimeTooShort = computed(
  () => form.max_lifetime_seconds < form.idle_ttl_seconds,
);
</script>

<template>
  <section class="space-y-5 border-y border-border/40 py-5">
    <div class="space-y-1">
      <h2 class="text-base font-medium">
        {{ t("admin.advancedAuth.durationTitle") }}
      </h2>
      <p class="text-sm leading-6 text-muted-foreground">
        {{ t("admin.advancedAuth.durationDescription") }}
      </p>
    </div>

    <div class="grid gap-5 sm:grid-cols-2">
      <div class="space-y-2">
        <Label for="advanced-auth-idle-ttl">
          {{ t("admin.advancedAuth.idleTtl") }}
        </Label>
        <div class="relative">
          <Input
            id="advanced-auth-idle-ttl"
            v-model.number="idleHours"
            class="pr-16"
            type="number"
            :min="MIN_ADVANCED_AUTH_TTL_HOURS"
            :max="MAX_ADVANCED_AUTH_IDLE_TTL_HOURS"
            step="any"
            :disabled="saving"
          />
          <span
            class="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground"
          >
            {{ t("admin.advancedAuth.hoursUnit") }}
          </span>
        </div>
        <p class="text-xs leading-5 text-muted-foreground">
          {{
            t("admin.advancedAuth.idleTtlDescription", {
              duration: idleDurationText,
            })
          }}
        </p>
      </div>

      <div class="space-y-2">
        <Label for="advanced-auth-max-lifetime">
          {{ t("admin.advancedAuth.maxLifetime") }}
        </Label>
        <div class="relative">
          <Input
            id="advanced-auth-max-lifetime"
            v-model.number="maxLifetimeHours"
            class="pr-16"
            type="number"
            :min="MIN_ADVANCED_AUTH_TTL_HOURS"
            :max="MAX_ADVANCED_AUTH_LIFETIME_HOURS"
            step="any"
            :disabled="saving"
          />
          <span
            class="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground"
          >
            {{ t("admin.advancedAuth.hoursUnit") }}
          </span>
        </div>
        <p class="text-xs leading-5 text-muted-foreground">
          {{
            t("admin.advancedAuth.maxLifetimeDescription", {
              duration: maxLifetimeDurationText,
            })
          }}
        </p>
        <p
          v-if="maxLifetimeTooShort"
          class="text-xs leading-5 text-destructive"
        >
          {{ t("admin.advancedAuth.maxLifetimeTooShort") }}
        </p>
      </div>
    </div>

    <div class="flex items-start gap-3 rounded-lg bg-muted/40 px-4 py-3">
      <Clock3 class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
      <div class="space-y-0.5 text-sm">
        <p class="font-medium">
          {{ t("admin.advancedAuth.durationSummaryTitle") }}
        </p>
        <p class="leading-6 text-muted-foreground">
          {{
            t("admin.advancedAuth.durationSummary", {
              idle: idleDurationText,
              maximum: maxLifetimeDurationText,
            })
          }}
        </p>
      </div>
    </div>
  </section>
</template>
