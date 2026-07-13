<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import {
  useAsyncAction,
  extractErrorMessage,
} from "@admin-shared/composables/useAsyncAction";
import { parseCidrTextarea } from "@admin-shared/utils/cidr";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
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
import CidrRegionSelector from "@/components/CidrRegionSelector.vue";
import { ConfigAPI } from "../../lib/api";
import type {
  GatewayVisibilityDetails,
  GatewayVisibilitySelection,
} from "../../types";
import { getCidrRegionSelectionKey } from "../../types/cidr";

const { t } = useI18n();
const settings = ref<GatewayVisibilityDetails | null>(null);
const loadError = ref("");

const form = reactive({
  enabled: false,
  selections: [] as GatewayVisibilitySelection[],
  customCidrsText: "",
});

const customCidrsState = computed(() =>
  parseCidrTextarea(form.customCidrsText),
);

const customCidrCount = computed(() => customCidrsState.value.cidrs.length);
const invalidCustomCidrs = computed(() => customCidrsState.value.invalid);
const visibilityInputsDisabled = computed(
  () => isSaving.value || !form.enabled,
);

const hasVisibleTargets = computed(
  () => form.selections.length > 0 || customCidrsState.value.cidrs.length > 0,
);

const formSnapshot = computed(() =>
  JSON.stringify({
    enabled: form.enabled,
    selections: form.selections.map((item) => getCidrRegionSelectionKey(item)),
    custom_cidrs: customCidrsState.value.cidrs,
  }),
);

const savedSnapshot = computed(() =>
  JSON.stringify({
    enabled: settings.value?.config.enabled ?? false,
    selections: (settings.value?.config.selections ?? []).map((item) =>
      getCidrRegionSelectionKey(item),
    ),
    custom_cidrs: settings.value?.config.custom_cidrs ?? [],
  }),
);

const isDirty = computed(() => formSnapshot.value !== savedSnapshot.value);

const saveBlockedReason = computed(() => {
  if (invalidCustomCidrs.value.length > 0) {
    return t("admin.gatewayVisibilitySettings.fixCustomCidrs");
  }
  return "";
});

const { isPending: isLoading, run: runLoad } = useAsyncAction({
  onError: (error) => {
    loadError.value = extractErrorMessage(
      error,
      t("admin.gatewayVisibilitySettings.loadFailedDescription"),
    );
  },
});

const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.gatewayVisibilitySettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.gatewayVisibilitySettings.saveFailedDescription"),
      ),
    });
  },
});

const applyDetails = (details: GatewayVisibilityDetails) => {
  settings.value = {
    config: {
      enabled: details.config.enabled,
      selections: details.config.selections.map((item) => ({ ...item })),
      custom_cidrs: [...details.config.custom_cidrs],
    },
    summary: { ...details.summary },
  };

  form.enabled = details.config.enabled;
  form.selections = details.config.selections.map((item) => ({ ...item }));
  form.customCidrsText = details.config.custom_cidrs.join("\n");
};

const fetchDetails = async () => {
  await runLoad(async () => {
    loadError.value = "";
    const details = await ConfigAPI.getGatewayVisibility();
    applyDetails(details);
  });
};

const resetForm = () => {
  if (!settings.value) return;
  applyDetails(settings.value);
};

const saveSettings = async () => {
  if (invalidCustomCidrs.value.length > 0) {
    toast.error(t("admin.gatewayVisibilitySettings.cidrValidationFailed"), {
      description: t("admin.gatewayVisibilitySettings.fixEntries", {
        items: invalidCustomCidrs.value.join("、"),
      }),
    });
    return;
  }

  await runSave(
    () =>
      ConfigAPI.updateGatewayVisibility({
        enabled: form.enabled,
        selections: form.selections.map((item) => ({
          province: item.province,
          query_city: item.query_city,
        })),
        custom_cidrs: customCidrsState.value.cidrs,
      }),
    {
      onSuccess: async (details) => {
        applyDetails(details);
        toast.success(t("admin.gatewayVisibilitySettings.updated"));
      },
    },
  );
};

onMounted(() => {
  void fetchDetails();
});
</script>

<template>
  <div class="space-y-6">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">{{
            t("admin.gatewayVisibilitySettings.systemSettings")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=gateway">{{
            t("admin.gatewayVisibilitySettings.gateway")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{
            t("admin.gatewayVisibilitySettings.title")
          }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/60 shadow-none">
      <CardHeader class="space-y-3">
        <div class="space-y-1.5">
          <CardTitle class="text-xl">{{
            t("admin.gatewayVisibilitySettings.title")
          }}</CardTitle>
          <CardDescription class="max-w-3xl leading-6">
            {{ t("admin.gatewayVisibilitySettings.description") }}
          </CardDescription>
        </div>
      </CardHeader>

      <CardContent class="space-y-6">
        <div
          v-if="isLoading"
          class="rounded-xl border border-border/60 bg-muted/20 px-5 py-12 text-center text-sm text-muted-foreground"
        >
          {{ t("admin.gatewayVisibilitySettings.loadingConfig") }}
        </div>

        <div
          v-else-if="loadError"
          class="rounded-xl border border-destructive/25 bg-destructive/5 px-5 py-4 text-sm text-destructive"
        >
          {{ loadError }}
        </div>

        <template v-else>
          <div
            class="rounded-2xl border border-border/60 bg-muted/10 px-4 py-4"
          >
            <div class="flex items-start justify-between gap-4">
              <div class="min-w-0 space-y-2">
                <div class="flex flex-wrap items-center gap-2">
                  <Label class="text-base font-medium">{{
                    t("admin.gatewayVisibilitySettings.visibilityConstraint")
                  }}</Label>
                </div>
              </div>

              <Switch
                v-model="form.enabled"
                class="mt-0.5 shrink-0"
                :disabled="isSaving"
              />
            </div>
          </div>

          <div class="overflow-hidden rounded-xl border border-border/60">
            <template v-if="form.enabled">
              <div
                v-if="!hasVisibleTargets"
                class="border-b border-amber-500/20 bg-amber-500/10 px-5 py-3 text-sm leading-6 text-amber-800 dark:text-amber-200"
              >
                {{ t("admin.gatewayVisibilitySettings.emptyRulesHint") }}
              </div>

              <section class="space-y-4 p-5">
                <Label class="text-base">{{
                  t("admin.gatewayVisibilitySettings.regionScope")
                }}</Label>
                <CidrRegionSelector
                  v-model="form.selections"
                  :disabled="visibilityInputsDisabled"
                  :description="
                    t('admin.gatewayVisibilitySettings.regionScopeHint')
                  "
                  :text="{
                    add: t('admin.gatewayVisibilitySettings.add'),
                    addRegion: t('admin.gatewayVisibilitySettings.addRegion'),
                    cancel: t('common.cancel'),
                    dialogDescription: t(
                      'admin.gatewayVisibilitySettings.addRegionDescription',
                    ),
                    loadFailed: t(
                      'admin.gatewayVisibilitySettings.cityLoadFailed',
                    ),
                    loadFailedDescription: t(
                      'admin.gatewayVisibilitySettings.cityLoadFailedDescription',
                    ),
                    loading: t('admin.gatewayVisibilitySettings.loading'),
                    noRegions: t('admin.gatewayVisibilitySettings.noRegions'),
                    province: t('admin.gatewayVisibilitySettings.province'),
                    retry: t('admin.subdomainProxy.retry'),
                    scope: t('admin.gatewayVisibilitySettings.scope'),
                    selectCity: t('admin.gatewayVisibilitySettings.selectCity'),
                    selectCityOrProvince: t(
                      'admin.gatewayVisibilitySettings.selectCityOrProvinceWide',
                    ),
                    selectProvince: t(
                      'admin.gatewayVisibilitySettings.selectProvince',
                    ),
                    selectProvinceFirst: t(
                      'admin.gatewayVisibilitySettings.selectProvinceFirst',
                    ),
                  }"
                />
              </section>

              <section class="space-y-4 border-t border-border/60 p-5">
                <div class="space-y-1">
                  <Label class="text-base">{{
                    t("admin.gatewayVisibilitySettings.customCidrs")
                  }}</Label>
                  <p class="text-sm leading-6 text-muted-foreground">
                    {{
                      t("admin.gatewayVisibilitySettings.customCidrsHintBefore")
                    }}
                    <code>1.2.3.0/24</code>
                    {{
                      t(
                        "admin.gatewayVisibilitySettings.customCidrsHintBetween",
                      )
                    }}
                    <code>2408:8000::/24</code>
                    {{
                      t("admin.gatewayVisibilitySettings.customCidrsHintAfter")
                    }}
                  </p>
                </div>

                <Textarea
                  v-model="form.customCidrsText"
                  :disabled="visibilityInputsDisabled"
                  class="min-h-36 font-mono text-sm"
                  :placeholder="
                    t('admin.gatewayVisibilitySettings.cidrPlaceholder')
                  "
                />

                <div class="flex flex-wrap gap-x-4 gap-y-2 text-sm">
                  <span class="text-muted-foreground">
                    {{
                      t(
                        "admin.gatewayVisibilitySettings.customCidrsRecognized",
                        {
                          count: customCidrCount,
                        },
                      )
                    }}
                  </span>
                  <span
                    v-if="invalidCustomCidrs.length > 0"
                    class="text-destructive"
                  >
                    {{
                      t("admin.gatewayVisibilitySettings.invalidCidrs", {
                        items: invalidCustomCidrs.join("、"),
                      })
                    }}
                  </span>
                  <span v-else class="text-emerald-600">
                    {{ t("admin.gatewayVisibilitySettings.cidrValid") }}
                  </span>
                </div>
              </section>
            </template>

            <FloatingActionDock :active="isDirty" inline-class="space-y-4 p-5">
              <template #inline>
                <div class="flex flex-wrap items-center justify-end gap-3">
                  <Button
                    variant="outline"
                    :disabled="!isDirty || isSaving"
                    @click="resetForm"
                  >
                    {{ t("admin.gatewayVisibilitySettings.reset") }}
                  </Button>
                  <Button
                    :disabled="
                      !isDirty || isSaving || Boolean(saveBlockedReason)
                    "
                    @click="saveSettings"
                  >
                    <span
                      v-if="isSaving"
                      class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
                    ></span>
                    {{
                      isSaving
                        ? t("admin.gatewayVisibilitySettings.savingAndSyncing")
                        : t("admin.gatewayVisibilitySettings.saveAndSync")
                    }}
                  </Button>
                </div>
              </template>
            </FloatingActionDock>
          </div>
        </template>
      </CardContent>
    </Card>
  </div>
</template>
