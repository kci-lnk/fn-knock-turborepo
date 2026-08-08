<script setup lang="ts">
import { onMounted, reactive, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
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
import { ConfigAPI } from "../../lib/api";
import {
  buildGatewayPortalVersionPatch,
  normalizeGatewayPortalConfig,
} from "../../lib/gatewayPortal";
import { useConfigStore } from "../../store/config";
import type {
  GatewayPortalConfig,
  GatewayPortalDisplayStyle,
  GatewayPortalIconDragMode,
  GatewayPortalVersion,
  GatewaySettings,
} from "../../types";

const a11yId = useId();

const { t } = useI18n();
const configStore = useConfigStore();
const settings = ref<GatewayPortalConfig | null>(null);
const loadError = ref("");

const form = reactive<GatewayPortalConfig>({
  enabled: true,
  display_style: "title",
  show_app_icon: true,
  show_wol: true,
  icon_drag_mode: "corners",
  version: "v1",
});

const applyPortal = (portal?: Partial<GatewayPortalConfig> | null) => {
  const normalized = normalizeGatewayPortalConfig(portal);
  settings.value = normalized;
  form.display_style = normalized.display_style;
  form.show_app_icon = normalized.show_app_icon;
  form.show_wol = normalized.show_wol;
  form.icon_drag_mode = normalized.icon_drag_mode;
  form.version = normalized.version;
  form.enabled = normalized.enabled;
};

const { isPending: isLoading, run: runLoad } = useAsyncAction({
  onError: (error) => {
    loadError.value = extractErrorMessage(
      error,
      t("admin.gatewayPortalSettings.loadFailedDescription"),
    );
  },
});

const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.gatewayPortalSettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.gatewayPortalSettings.saveFailedDescription"),
      ),
    });
  },
});

const refreshConfigStore = async () => {
  try {
    await configStore.loadConfig();
  } catch (error) {
    console.error("[gateway-portal] failed to refresh config store:", error);
  }
};

const applySavedSettings = async (data: GatewaySettings | undefined) => {
  if (!data) return false;
  applyPortal(data.portal);
  await refreshConfigStore();
  toast.success(t("admin.gatewayPortalSettings.updated"));
  return true;
};

const loadSettings = async () => {
  await runLoad(async () => {
    loadError.value = "";
    const data = await ConfigAPI.getGatewaySettings();
    applyPortal(data.portal);
  });
};

const saveDisplayStyle = async (style: GatewayPortalDisplayStyle) => {
  if (isSaving.value || form.display_style === style) return;

  const previous = { ...form };
  form.display_style = style;

  const data = await runSave(() =>
    ConfigAPI.updateGatewaySettings({
      portal: {
        display_style: style,
      },
    }),
  );

  if (!(await applySavedSettings(data))) {
    applyPortal(previous);
  }
};

const saveVersion = async (version: GatewayPortalVersion) => {
  if (isSaving.value || form.version === version) return;

  const previous = { ...form };
  form.version = version;

  const data = await runSave(() =>
    ConfigAPI.updateGatewaySettings(buildGatewayPortalVersionPatch(version)),
  );

  if (!(await applySavedSettings(data))) {
    applyPortal(previous);
  }
};

const saveEnabled = async (value: boolean) => {
  if (isSaving.value || form.enabled === value) return;

  const previous = { ...form };
  form.enabled = value;

  const data = await runSave(() =>
    ConfigAPI.updateGatewaySettings({
      portal: {
        enabled: value,
      },
    }),
  );

  if (!(await applySavedSettings(data))) {
    applyPortal(previous);
  }
};

const saveShowAppIcon = async (value: boolean) => {
  if (isSaving.value || form.show_app_icon === value) return;

  const previous = { ...form };
  form.show_app_icon = value;

  const data = await runSave(() =>
    ConfigAPI.updateGatewaySettings({
      portal: {
        show_app_icon: value,
      },
    }),
  );

  if (!(await applySavedSettings(data))) {
    applyPortal(previous);
  }
};

const saveShowWOL = async (value: boolean) => {
  if (isSaving.value || form.show_wol === value) return;

  const previous = { ...form };
  form.show_wol = value;
  const data = await runSave(() =>
    ConfigAPI.updateGatewaySettings({ portal: { show_wol: value } }),
  );
  if (!(await applySavedSettings(data))) applyPortal(previous);
};

const saveIconDragMode = async (mode: GatewayPortalIconDragMode) => {
  if (isSaving.value || form.icon_drag_mode === mode) return;

  const previous = { ...form };
  form.icon_drag_mode = mode;

  const data = await runSave(() =>
    ConfigAPI.updateGatewaySettings({
      portal: {
        icon_drag_mode: mode,
      },
    }),
  );

  if (!(await applySavedSettings(data))) {
    applyPortal(previous);
  }
};

onMounted(() => {
  void loadSettings();
});
</script>

<template>
  <div class="space-y-6">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system">{{
            t("admin.gatewayPortalSettings.systemSettings")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbLink href="#/system?tab=gateway">{{
            t("admin.gatewayPortalSettings.gateway")
          }}</BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{
            t("admin.gatewayPortalSettings.title")
          }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/60 shadow-none">
      <CardHeader class="space-y-3">
        <div class="space-y-1.5">
          <CardTitle class="text-xl">{{
            t("admin.gatewayPortalSettings.title")
          }}</CardTitle>
          <CardDescription class="max-w-3xl leading-6">
            {{ t("admin.gatewayPortalSettings.description") }}
          </CardDescription>
        </div>
      </CardHeader>

      <CardContent class="space-y-0 border-t p-0 divide-y">
        <div
          v-if="isLoading"
          class="px-5 py-12 text-center text-sm text-muted-foreground"
          role="status"
        >
          {{ t("admin.gatewayPortalSettings.loadingConfig") }}
        </div>

        <div
          v-else-if="loadError"
          class="px-5 py-4 text-sm text-destructive"
          role="alert"
        >
          {{ loadError }}
        </div>

        <template v-else>
          <section class="p-6">
            <div
              class="rounded-2xl border border-border/60 bg-muted/10 px-4 py-4"
            >
              <div class="flex items-start justify-between gap-4">
                <div class="min-w-0 space-y-1.5">
                  <Label
                    :for="`${a11yId}-gatewayportalsettings-1`"
                    class="text-base font-medium"
                    >{{ t("admin.gatewayPortalSettings.enabled") }}</Label
                  >
                  <div class="text-sm leading-6 text-muted-foreground">
                    {{ t("admin.gatewayPortalSettings.enabledDescription") }}
                  </div>
                </div>
                <Switch
                  :id="`${a11yId}-gatewayportalsettings-1`"
                  class="mt-0.5 shrink-0"
                  :model-value="form.enabled"
                  :disabled="isSaving"
                  @update:model-value="saveEnabled($event === true)"
                />
              </div>
            </div>
          </section>

          <template v-if="form.enabled">
            <section
              class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
            >
              <div class="space-y-1 pr-6">
                <div class="text-base font-medium">
                  {{ t("admin.gatewayPortalSettings.version") }}
                </div>
                <div class="text-sm text-muted-foreground">
                  {{ t("admin.gatewayPortalSettings.versionDescription") }}
                </div>
              </div>
              <div
                role="group"
                :aria-label="t('admin.gatewayPortalSettings.version')"
                class="inline-flex w-fit rounded-md border bg-background p-1"
              >
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  :aria-pressed="form.version === 'v1'"
                  :class="[
                    'h-8 px-3',
                    form.version === 'v1'
                      ? 'bg-foreground text-background hover:bg-foreground/90 hover:text-background'
                      : '',
                  ]"
                  :disabled="isSaving"
                  @click="saveVersion('v1')"
                >
                  {{ t("admin.gatewayPortalSettings.versionV1") }}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  :aria-pressed="form.version === 'v2'"
                  :class="[
                    'h-8 px-3',
                    form.version === 'v2'
                      ? 'bg-foreground text-background hover:bg-foreground/90 hover:text-background'
                      : '',
                  ]"
                  :disabled="isSaving"
                  @click="saveVersion('v2')"
                >
                  {{ t("admin.gatewayPortalSettings.versionV2") }}
                </Button>
              </div>
            </section>

            <section
              class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
            >
              <div class="space-y-1 pr-6">
                <div class="text-base font-medium">
                  {{ t("admin.gatewayPortalSettings.display") }}
                </div>
                <div class="text-sm text-muted-foreground">
                  {{ t("admin.gatewayPortalSettings.displayDescription") }}
                </div>
              </div>
              <div
                role="group"
                :aria-label="t('admin.gatewayPortalSettings.display')"
                class="inline-flex w-fit rounded-md border bg-background p-1"
              >
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  :aria-pressed="form.display_style === 'domain'"
                  :class="[
                    'h-8 px-3',
                    form.display_style === 'domain'
                      ? 'bg-foreground text-background hover:bg-foreground/90 hover:text-background'
                      : '',
                  ]"
                  :disabled="isSaving"
                  @click="saveDisplayStyle('domain')"
                >
                  {{ t("admin.gatewayPortalSettings.displayDomain") }}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  :aria-pressed="form.display_style === 'title'"
                  :class="[
                    'h-8 px-3',
                    form.display_style === 'title'
                      ? 'bg-foreground text-background hover:bg-foreground/90 hover:text-background'
                      : '',
                  ]"
                  :disabled="isSaving"
                  @click="saveDisplayStyle('title')"
                >
                  {{ t("admin.gatewayPortalSettings.displayTitle") }}
                </Button>
              </div>
            </section>

            <section
              class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
            >
              <div class="space-y-1 pr-6">
                <div class="text-base font-medium">
                  {{ t("admin.gatewayPortalSettings.iconDragMode") }}
                </div>
                <div class="text-sm text-muted-foreground">
                  {{ t("admin.gatewayPortalSettings.iconDragModeDescription") }}
                </div>
              </div>
              <div
                role="group"
                :aria-label="t('admin.gatewayPortalSettings.iconDragMode')"
                class="inline-flex w-fit rounded-md border bg-background p-1"
              >
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  :aria-pressed="form.icon_drag_mode === 'corners'"
                  :class="[
                    'h-8 px-3',
                    form.icon_drag_mode === 'corners'
                      ? 'bg-foreground text-background hover:bg-foreground/90 hover:text-background'
                      : '',
                  ]"
                  :disabled="isSaving"
                  @click="saveIconDragMode('corners')"
                >
                  {{ t("admin.gatewayPortalSettings.iconDragModeCorners") }}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  :aria-pressed="form.icon_drag_mode === 'free'"
                  :class="[
                    'h-8 px-3',
                    form.icon_drag_mode === 'free'
                      ? 'bg-foreground text-background hover:bg-foreground/90 hover:text-background'
                      : '',
                  ]"
                  :disabled="isSaving"
                  @click="saveIconDragMode('free')"
                >
                  {{ t("admin.gatewayPortalSettings.iconDragModeFree") }}
                </Button>
              </div>
            </section>

            <section class="flex items-center justify-between gap-4 p-6">
              <div class="space-y-1 pr-6">
                <Label
                  :for="`${a11yId}-gatewayportalsettings-2`"
                  class="text-base"
                  >{{ t("admin.gatewayPortalSettings.showAppIcon") }}</Label
                >
                <div class="text-sm text-muted-foreground">
                  {{ t("admin.gatewayPortalSettings.showAppIconDescription") }}
                </div>
              </div>
              <Switch
                :id="`${a11yId}-gatewayportalsettings-2`"
                :model-value="form.show_app_icon"
                :disabled="isSaving"
                @update:model-value="saveShowAppIcon($event === true)"
              />
            </section>

            <section
              v-if="configStore.config?.wol_feature?.enabled === true"
              class="flex items-center justify-between gap-4 p-6"
            >
              <div class="space-y-1 pr-6">
                <Label
                  :for="`${a11yId}-gatewayportalsettings-wol`"
                  class="text-base"
                >
                  {{ t("admin.gatewayPortalSettings.showWol") }}
                </Label>
                <div class="text-sm text-muted-foreground">
                  {{ t("admin.gatewayPortalSettings.showWolDescription") }}
                </div>
              </div>
              <Switch
                :id="`${a11yId}-gatewayportalsettings-wol`"
                :model-value="form.show_wol"
                :disabled="isSaving"
                @update:model-value="saveShowWOL($event === true)"
              />
            </section>
          </template>
        </template>
      </CardContent>
    </Card>
  </div>
</template>
