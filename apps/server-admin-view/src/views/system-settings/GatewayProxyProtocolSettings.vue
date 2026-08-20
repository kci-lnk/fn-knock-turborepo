<script setup lang="ts">
import { computed, onMounted, reactive, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import {
  isValidCIDR,
  isValidIPv4Address,
  isValidIPv6Address,
  normalizeCidrLines,
  splitCidrTextarea,
} from "@admin-shared/utils/cidr";
import { toast } from "@admin-shared/utils/toast";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { ConfigAPI } from "@/lib/api/config";
import type { GatewayProxyProtocolConfig } from "@/types";

const { t } = useI18n();
const a11yId = useId();
const settings = ref<GatewayProxyProtocolConfig | null>(null);
const loadError = ref("");
const form = reactive({ enabled: false, trustedSourcesText: "" });

const parsedSources = computed(() => {
  const sources = normalizeCidrLines(
    splitCidrTextarea(form.trustedSourcesText),
  );
  const invalid = sources.filter((source) => {
    if (source === "0.0.0.0/0" || source === "::/0") return true;
    return !(
      isValidIPv4Address(source) ||
      isValidIPv6Address(source) ||
      isValidCIDR(source)
    );
  });
  return { sources, invalid };
});

const snapshot = computed(() =>
  JSON.stringify({
    enabled: form.enabled,
    trusted_sources: parsedSources.value.sources,
  }),
);
const savedSnapshot = computed(() =>
  JSON.stringify({
    enabled: settings.value?.enabled ?? false,
    trusted_sources: settings.value?.trusted_sources ?? [],
  }),
);
const isDirty = computed(() => snapshot.value !== savedSnapshot.value);
const saveBlockedReason = computed(() => {
  if (parsedSources.value.invalid.length > 0) {
    return t("admin.gatewayProxyProtocolSettings.invalidSources");
  }
  if (form.enabled && parsedSources.value.sources.length === 0) {
    return t("admin.gatewayProxyProtocolSettings.sourceRequired");
  }
  return "";
});

const { isPending: isLoading, run: runLoad } = useAsyncAction({
  onError: (error) => {
    loadError.value = extractErrorMessage(
      error,
      t("admin.gatewayProxyProtocolSettings.loadFailedDescription"),
    );
  },
});
const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.gatewayProxyProtocolSettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.gatewayProxyProtocolSettings.saveFailedDescription"),
      ),
    });
  },
});

const applySettings = (value: GatewayProxyProtocolConfig) => {
  settings.value = {
    ...value,
    trusted_sources: [...value.trusted_sources],
  };
  form.enabled = value.enabled;
  form.trustedSourcesText = value.trusted_sources.join("\n");
};

const fetchSettings = async () => {
  await runLoad(async () => {
    loadError.value = "";
    applySettings(await ConfigAPI.getGatewayProxyProtocol());
  });
};

const resetForm = () => {
  if (settings.value) applySettings(settings.value);
};

const saveSettings = async () => {
  if (saveBlockedReason.value) {
    toast.error(saveBlockedReason.value, {
      description:
        parsedSources.value.invalid.length > 0
          ? parsedSources.value.invalid.join("、")
          : undefined,
    });
    return;
  }
  await runSave(
    () =>
      ConfigAPI.updateGatewayProxyProtocol({
        enabled: form.enabled,
        trusted_sources: parsedSources.value.sources,
      }),
    {
      onSuccess: (value) => {
        applySettings(value);
        toast.success(t("admin.gatewayProxyProtocolSettings.updated"));
      },
    },
  );
};

onMounted(() => void fetchSettings());
</script>

<template>
  <div class="space-y-6">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">{{
            t("admin.gatewayProxyProtocolSettings.systemSettings")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=gateway">{{
            t("admin.gatewayProxyProtocolSettings.gateway")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{
            t("admin.gatewayProxyProtocolSettings.title")
          }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/60 shadow-none">
      <CardHeader class="space-y-2">
        <CardTitle class="text-xl">{{
          t("admin.gatewayProxyProtocolSettings.title")
        }}</CardTitle>
        <CardDescription class="max-w-3xl leading-6">
          {{ t("admin.gatewayProxyProtocolSettings.description") }}
        </CardDescription>
      </CardHeader>

      <CardContent class="space-y-6">
        <div
          v-if="isLoading"
          class="rounded-xl border border-border/60 bg-muted/20 px-5 py-12 text-center text-sm text-muted-foreground"
          role="status"
        >
          {{ t("admin.gatewayProxyProtocolSettings.loading") }}
        </div>
        <div
          v-else-if="loadError"
          class="rounded-xl border border-destructive/25 bg-destructive/5 px-5 py-4 text-sm text-destructive"
          role="alert"
        >
          {{ loadError }}
        </div>

        <template v-else-if="settings">
          <div class="flex flex-wrap items-center gap-2">
            <Badge
              :variant="settings.effective_enabled ? 'default' : 'secondary'"
            >
              {{
                settings.effective_enabled
                  ? t("admin.gatewayProxyProtocolSettings.effectiveEnabled")
                  : t("admin.gatewayProxyProtocolSettings.effectiveDisabled")
              }}
            </Badge>
            <Badge v-if="settings.managed_frp_enabled" variant="secondary">
              {{ t("admin.gatewayProxyProtocolSettings.managedFrpEnabled") }}
            </Badge>
          </div>

          <div class="rounded-xl border border-border/60 bg-muted/10 p-5">
            <div class="flex items-start justify-between gap-4">
              <div class="space-y-1">
                <Label :for="`${a11yId}-enabled`" class="text-base">{{
                  t("admin.gatewayProxyProtocolSettings.externalEnabled")
                }}</Label>
                <p class="max-w-3xl text-sm leading-6 text-muted-foreground">
                  {{
                    t("admin.gatewayProxyProtocolSettings.externalEnabledHint")
                  }}
                </p>
              </div>
              <Switch
                :id="`${a11yId}-enabled`"
                v-model="form.enabled"
                :disabled="isSaving"
              />
            </div>
          </div>

          <div class="space-y-2">
            <Label :for="`${a11yId}-sources`">{{
              t("admin.gatewayProxyProtocolSettings.trustedSources")
            }}</Label>
            <Textarea
              :id="`${a11yId}-sources`"
              v-model="form.trustedSourcesText"
              class="min-h-40 font-mono text-sm"
              :disabled="isSaving"
              :placeholder="t('admin.gatewayProxyProtocolSettings.placeholder')"
            />
            <p class="text-sm leading-6 text-muted-foreground">
              {{ t("admin.gatewayProxyProtocolSettings.trustedSourcesHint") }}
            </p>
            <p
              v-if="parsedSources.invalid.length"
              class="text-sm text-destructive"
              role="alert"
            >
              {{ t("admin.gatewayProxyProtocolSettings.invalidEntries") }}:
              {{ parsedSources.invalid.join("、") }}
            </p>
          </div>

          <div
            class="rounded-xl border border-amber-500/25 bg-amber-500/10 px-5 py-4 text-sm leading-6 text-amber-800 dark:text-amber-200"
          >
            {{ t("admin.gatewayProxyProtocolSettings.securityWarning") }}
          </div>

          <FloatingActionDock
            :active="isDirty"
            inline-class="flex items-center justify-end gap-3"
          >
            <template #inline>
              <Button
                variant="outline"
                :disabled="!isDirty || isSaving"
                @click="resetForm"
              >
                {{ t("admin.gatewayProxyProtocolSettings.reset") }}
              </Button>
              <Button
                :disabled="!isDirty || isSaving || !!saveBlockedReason"
                @click="saveSettings"
              >
                {{ t("admin.gatewayProxyProtocolSettings.save") }}
              </Button>
            </template>
          </FloatingActionDock>
        </template>
      </CardContent>
    </Card>
  </div>
</template>
