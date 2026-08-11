<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref, useId } from "vue";
import { onBeforeRouteLeave, useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import {
  ArrowLeft,
  Clock3,
  Plus,
  Save,
  ShieldOff,
  Trash2,
} from "lucide-vue-next";
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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import CidrRegionSelector from "@/components/CidrRegionSelector.vue";
import ConfirmationDialog from "@admin-shared/components/common/ConfirmationDialog.vue";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { useConfirmationDialog } from "@admin-shared/composables/useConfirmationDialog";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI, type AdvancedAuthDetails } from "../../lib/api";
import { isHttpTargetUrl, normalizeHostLike } from "./model";
import { useConfigStore } from "../../store/config";
import AdvancedAuthHeaderNameField from "./AdvancedAuthHeaderNameField.vue";
import type {
  AdvancedAuthConditionTarget,
  AdvancedAuthConfig,
  AdvancedAuthOperator,
} from "../../types";
import {
  advancedAuthHourInputToSeconds,
  advancedAuthTargetOptions,
  cloneAdvancedAuthConfig,
  createAdvancedAuthRuleEditor,
  getAdvancedAuthValidationIssue,
  isAdvancedAuthBroadRule,
  MAX_ADVANCED_AUTH_CONDITIONS,
  MAX_ADVANCED_AUTH_GROUPS,
  MAX_ADVANCED_AUTH_IDLE_TTL_HOURS,
  MAX_ADVANCED_AUTH_IDLE_TTL_SECONDS,
  MAX_ADVANCED_AUTH_LIFETIME_HOURS,
  MAX_ADVANCED_AUTH_LIFETIME_SECONDS,
  MIN_ADVANCED_AUTH_TTL_HOURS,
  SECONDS_PER_MINUTE,
  secondsToAdvancedAuthHourInput,
  snapshotAdvancedAuthConfig,
} from "./advanced-auth-form";

const a11yId = useId();

const route = useRoute();
const router = useRouter();
const { t } = useI18n();
const configStore = useConfigStore();
const {
  confirmationDialogOpen,
  confirmationDialogOptions,
  confirmPendingAction,
  handleConfirmationDialogOpenChange,
  requestConfirmation,
} = useConfirmationDialog();

const MAX_GROUPS = MAX_ADVANCED_AUTH_GROUPS;
const MAX_CONDITIONS = MAX_ADVANCED_AUTH_CONDITIONS;
const MIN_TTL_HOURS = MIN_ADVANCED_AUTH_TTL_HOURS;
const MAX_IDLE_TTL_HOURS = MAX_ADVANCED_AUTH_IDLE_TTL_HOURS;
const MAX_LIFETIME_HOURS = MAX_ADVANCED_AUTH_LIFETIME_HOURS;
const host = computed(() => String(route.params.host ?? "").trim());
const loading = ref(true);
const saving = ref(false);
const loadError = ref("");
const missing = ref(false);
const revision = ref<string | null>(null);
const savedSnapshot = ref("");
const confirmedBroadSnapshot = ref("");
const valueDrafts = reactive<Record<string, string>>({});

const form = reactive<AdvancedAuthConfig>({
  enabled: false,
  idle_ttl_seconds: 24 * 60 * 60,
  max_lifetime_seconds: 30 * 24 * 60 * 60,
  groups: [],
});

const targetOptions = advancedAuthTargetOptions;
const cloneConfig = cloneAdvancedAuthConfig;

const {
  addCondition,
  addGroup,
  needsValue,
  normalizeValueDraft,
  operatorsFor,
  removeCondition,
  removeGroup,
  setSourceIpValue,
  setValueText,
  sourceNetworkTranslationKey,
  updateOperator,
  updateTarget,
  valueInputText,
} = createAdvancedAuthRuleEditor(form, valueDrafts);

const secondsToHourInput = secondsToAdvancedAuthHourInput;
const hourInputToSeconds = advancedAuthHourInputToSeconds;

const idleHours = computed({
  get: () => secondsToHourInput(form.idle_ttl_seconds),
  set: (value: number) => {
    form.idle_ttl_seconds = hourInputToSeconds(
      value,
      MAX_ADVANCED_AUTH_IDLE_TTL_SECONDS,
    );
  },
});
const maxLifetimeHours = computed({
  get: () => secondsToHourInput(form.max_lifetime_seconds),
  set: (value: number) => {
    form.max_lifetime_seconds = hourInputToSeconds(
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
    return t("admin.advancedAuth.durationHours", {
      hours: minutes / 60,
    });
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

const snapshotConfig = () => snapshotAdvancedAuthConfig(form);
const isDirty = computed(() => snapshotConfig() !== savedSnapshot.value);
const isBroadRule = computed(() => isAdvancedAuthBroadRule(form));

const regionText = {
  add: t("admin.advancedAuth.addRegion"),
  addRegion: t("admin.advancedAuth.addRegion"),
  cancel: t("common.cancel"),
  dialogDescription: t("admin.advancedAuth.regionDialogDescription"),
  loadFailed: t("admin.advancedAuth.regionLoadFailed"),
  loadFailedDescription: t("admin.advancedAuth.regionLoadFailedDescription"),
  loading: t("common.loadingConfig"),
  noRegions: t("admin.advancedAuth.noRegions"),
  province: t("admin.advancedAuth.province"),
  retry: t("admin.advancedAuth.retry"),
  selectedCount: (count: number) =>
    t("admin.advancedAuth.selectedRegions", { count }),
  scope: t("admin.advancedAuth.scope"),
  selectCity: t("admin.advancedAuth.selectCity"),
  selectProvince: t("admin.advancedAuth.selectProvince"),
  selectProvinceFirst: t("admin.advancedAuth.selectProvinceFirst"),
  unavailable: t("admin.advancedAuth.unavailable"),
};

const applyDetails = (details: AdvancedAuthDetails) => {
  Object.keys(valueDrafts).forEach((key) => delete valueDrafts[key]);
  revision.value = details.revision;
  const next = cloneConfig(details.advanced_auth);
  form.enabled = next.enabled;
  form.idle_ttl_seconds = next.idle_ttl_seconds;
  form.max_lifetime_seconds = next.max_lifetime_seconds;
  form.policy_version = next.policy_version;
  form.groups.splice(0, form.groups.length, ...next.groups);
  savedSnapshot.value = snapshotConfig();
  confirmedBroadSnapshot.value = "";
};

const load = async () => {
  loading.value = true;
  loadError.value = "";
  missing.value = false;
  try {
    if (!host.value) throw new Error("Missing host");
    if (!configStore.config) await configStore.loadConfig();
    const mapping = configStore.config?.host_mappings?.find(
      (item) => normalizeHostLike(item.host) === normalizeHostLike(host.value),
    );
    if (
      !mapping ||
      mapping.service_role === "auth" ||
      mapping.use_auth !== true ||
      !isHttpTargetUrl(mapping.target)
    ) {
      missing.value = true;
      loadError.value = t("admin.advancedAuth.notFound");
      return;
    }
    applyDetails(await ConfigAPI.getAdvancedAuth(host.value));
  } catch (error) {
    loadError.value = extractErrorMessage(
      error,
      t("admin.advancedAuth.loadFailed"),
    );
    missing.value = true;
  } finally {
    loading.value = false;
  }
};

const cancel = () => {
  void router.push("/subdomains");
};

const save = async () => {
  if (saving.value || !isDirty.value) return;
  const validationIssue = getAdvancedAuthValidationIssue(form);
  if (validationIssue) {
    if (
      validationIssue.kind === "invalid-source-address" ||
      validationIssue.kind === "invalid-source-cidr"
    ) {
      toast.error(
        t(
          validationIssue.kind === "invalid-source-address"
            ? "admin.advancedAuth.invalidSourceIpLine"
            : "admin.advancedAuth.invalidSourceCidrLine",
          { line: validationIssue.line },
        ),
      );
    } else {
      const translationKey = {
        "invalid-rules": "admin.advancedAuth.invalidRules",
        "empty-group": "admin.advancedAuth.emptyGroup",
        "invalid-condition": "admin.advancedAuth.invalidCondition",
        "max-lifetime-too-short": "admin.advancedAuth.maxLifetimeTooShort",
      }[validationIssue.kind];
      toast.error(t(translationKey));
    }
    return;
  }
  const pendingSnapshot = snapshotConfig();
  if (
    form.enabled &&
    isBroadRule.value &&
    confirmedBroadSnapshot.value !== pendingSnapshot
  ) {
    const confirmed = await requestConfirmation({
      confirmText: t("common.save"),
      description: t("admin.advancedAuth.broadRuleConfirm"),
      title: t("common.confirm"),
    });
    if (!confirmed) return;
    confirmedBroadSnapshot.value = pendingSnapshot;
  }
  const acknowledgeBroadRules =
    form.enabled &&
    isBroadRule.value &&
    confirmedBroadSnapshot.value === pendingSnapshot;
  saving.value = true;
  try {
    const details = await ConfigAPI.updateAdvancedAuth(
      host.value,
      revision.value,
      cloneConfig(form),
      acknowledgeBroadRules,
    );
    applyDetails(details);
    void configStore.loadConfig();
    toast.success(t("admin.advancedAuth.saved"));
  } catch (error) {
    toast.error(t("admin.advancedAuth.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.advancedAuth.saveFailedDescription"),
      ),
    });
  } finally {
    saving.value = false;
  }
};

const confirmDiscard = () => {
  if (!isDirty.value || saving.value) return true;
  return requestConfirmation({
    confirmVariant: "destructive",
    description: t("admin.advancedAuth.discardConfirm"),
    title: t("common.confirm"),
  });
};
onBeforeRouteLeave(() => confirmDiscard());
const handleBeforeUnload = (event: BeforeUnloadEvent) => {
  if (!isDirty.value) return;
  event.preventDefault();
  event.returnValue = "";
};
onMounted(() => {
  void load();
  window.addEventListener("beforeunload", handleBeforeUnload);
});
onUnmounted(() =>
  window.removeEventListener("beforeunload", handleBeforeUnload),
);
</script>

<template>
  <div class="space-y-5">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem
          ><BreadcrumbLink href="#/subdomains">{{
            t("admin.advancedAuth.subdomains")
          }}</BreadcrumbLink></BreadcrumbItem
        >
        <BreadcrumbSeparator />
        <BreadcrumbItem
          ><BreadcrumbPage>{{
            t("admin.advancedAuth.title")
          }}</BreadcrumbPage></BreadcrumbItem
        >
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/50 shadow-none">
      <CardHeader>
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="space-y-1.5">
            <CardTitle class="flex items-center gap-2 text-xl"
              ><ShieldOff class="h-5 w-5 text-primary" />{{
                t("admin.advancedAuth.title")
              }}</CardTitle
            >
            <CardDescription>{{ host }}</CardDescription>
          </div>
          <Button variant="outline" @click="cancel"
            ><ArrowLeft class="mr-2 h-4 w-4" />{{
              t("admin.advancedAuth.back")
            }}</Button
          >
        </div>
      </CardHeader>
      <CardContent
        v-if="loading"
        class="py-12 text-center text-muted-foreground"
        >{{ t("common.loadingConfig") }}</CardContent
      >
      <CardContent v-else-if="missing" class="space-y-4 py-8">
        <p
          class="rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive"
        >
          {{ loadError || t("admin.advancedAuth.notFound") }}
        </p>
        <Button variant="outline" @click="cancel">{{
          t("admin.advancedAuth.back")
        }}</Button>
      </CardContent>
      <CardContent v-else class="space-y-6 px-3 sm:px-6">
        <section class="rounded-xl bg-muted/30 p-4 sm:p-5">
          <div class="flex items-start justify-between gap-4">
            <div>
              <Label
                :for="`${a11yId}-subdomainadvancedauth-1`"
                class="text-base"
                >{{ t("admin.advancedAuth.enabled") }}</Label
              >
              <p class="mt-1 text-sm leading-6 text-muted-foreground">
                {{ t("admin.advancedAuth.enabledDescription") }}
              </p>
            </div>
            <Switch
              :id="`${a11yId}-subdomainadvancedauth-1`"
              v-model:model-value="form.enabled"
              :disabled="saving"
            />
          </div>
          <p class="mt-3 text-xs leading-5 text-amber-700 dark:text-amber-300">
            {{ t("admin.advancedAuth.temporaryGrantNotice") }}
          </p>
        </section>

        <div v-if="form.enabled" class="space-y-6">
          <section class="space-y-4">
            <div class="flex flex-wrap items-center justify-between gap-3">
              <div>
                <h2 class="text-base font-medium">
                  {{ t("admin.advancedAuth.ruleGroups") }}
                </h2>
                <p class="text-sm text-muted-foreground">
                  {{ t("admin.advancedAuth.ruleGroupsDescription") }}
                </p>
              </div>
              <Button
                variant="outline"
                class="w-full min-[480px]:w-auto"
                :disabled="form.groups.length >= MAX_GROUPS || saving"
                @click="addGroup"
                ><Plus class="mr-2 h-4 w-4" />{{
                  t("admin.advancedAuth.addOrGroup")
                }}</Button
              >
            </div>
            <div
              v-if="form.groups.length === 0"
              class="rounded-xl border border-dashed p-8 text-center text-sm text-muted-foreground"
            >
              {{ t("admin.advancedAuth.noGroups") }}
            </div>
            <div
              v-else
              class="relative space-y-4 sm:space-y-5 sm:pl-10 sm:before:absolute sm:before:inset-y-7 sm:before:left-4 sm:before:w-px sm:before:bg-border"
            >
              <div
                v-for="(group, groupIndex) in form.groups"
                :key="group.id"
                class="relative sm:before:absolute sm:before:top-7 sm:before:-left-6 sm:before:h-px sm:before:w-6 sm:before:bg-border"
              >
                <div
                  class="group/rule space-y-3 rounded-xl border border-border/65 bg-muted/25 p-3 shadow-none ring-2 ring-transparent transition-[border-color,background-color,box-shadow] duration-[280ms] ease-out hover:border-primary/25 hover:bg-muted/35 hover:ring-primary/5 focus-within:border-primary/50 focus-within:bg-muted/35 focus-within:ring-primary/15 motion-reduce:transition-none dark:bg-muted/20 dark:hover:bg-muted/30 dark:focus-within:bg-muted/30 sm:p-5"
                >
                  <div class="flex items-center justify-between gap-3">
                    <div
                      class="flex min-w-0 items-center gap-2 text-sm font-medium"
                    >
                      <span
                        class="shrink-0 rounded-md border border-primary/20 bg-primary/10 px-2 py-1 text-xs font-semibold text-primary transition-[border-color,background-color] duration-[280ms] ease-out group-hover/rule:border-primary/35 group-hover/rule:bg-primary/15 group-focus-within/rule:border-primary/50 group-focus-within/rule:bg-primary/20 motion-reduce:transition-none"
                      >
                        OR {{ groupIndex + 1 }}
                      </span>
                      <span class="truncate">{{
                        t("admin.advancedAuth.groupAll")
                      }}</span>
                    </div>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-8 w-8 shrink-0"
                      :disabled="saving"
                      :aria-label="t('admin.advancedAuth.deleteGroup')"
                      @click="removeGroup(groupIndex)"
                    >
                      <Trash2 class="h-4 w-4 text-destructive" />
                    </Button>
                  </div>

                  <div
                    class="relative space-y-3"
                    :class="group.conditions.length > 1 ? 'sm:pl-9' : ''"
                  >
                    <div
                      v-if="group.conditions.length > 1"
                      class="absolute inset-y-8 left-3.5 hidden w-px bg-border sm:block"
                    ></div>
                    <span
                      v-if="group.conditions.length > 1"
                      class="absolute top-1/2 left-3.5 z-10 hidden -translate-x-1/2 -translate-y-1/2 rounded border border-border bg-background px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground sm:block"
                    >
                      AND
                    </span>

                    <template
                      v-for="(condition, conditionIndex) in group.conditions"
                      :key="condition.id"
                    >
                      <div
                        v-if="conditionIndex > 0"
                        class="flex items-center gap-2 py-0.5 sm:hidden"
                      >
                        <span class="h-px flex-1 bg-border/80"></span>
                        <span
                          class="rounded border border-border/80 bg-background px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground"
                        >
                          AND
                        </span>
                        <span class="h-px flex-1 bg-border/80"></span>
                      </div>

                      <div
                        class="relative rounded-lg border border-border/60 bg-background/80 p-3 shadow-none"
                        :class="
                          group.conditions.length > 1
                            ? 'sm:before:absolute sm:before:top-1/2 sm:before:-left-[1.375rem] sm:before:h-px sm:before:w-[1.375rem] sm:before:bg-border'
                            : ''
                        "
                      >
                        <div class="flex min-w-0 items-start gap-1.5 sm:gap-2">
                          <div
                            class="grid min-w-0 flex-1 gap-3 sm:grid-cols-2"
                            :class="
                              condition.target === 'request_header' ||
                              condition.target === 'query_parameter'
                                ? needsValue(condition)
                                  ? 'xl:grid-cols-[minmax(8.5rem,0.8fr)_minmax(10rem,1fr)_minmax(8.5rem,0.8fr)_minmax(13rem,1.5fr)]'
                                  : 'xl:grid-cols-[minmax(8.5rem,0.8fr)_minmax(10rem,1fr)_minmax(8.5rem,0.8fr)]'
                                : condition.target === 'source_region' ||
                                    needsValue(condition)
                                  ? 'xl:grid-cols-[minmax(9rem,0.8fr)_minmax(9rem,0.8fr)_minmax(15rem,1.8fr)]'
                                  : 'xl:grid-cols-[minmax(9rem,1fr)_minmax(9rem,1fr)]'
                            "
                          >
                            <div class="min-w-0 space-y-1.5">
                              <Label
                                :for="`${a11yId}-subdomainadvancedauth-2`"
                                class="text-xs"
                                >{{
                                  t("admin.advancedAuth.matchTarget")
                                }}</Label
                              >
                              <select
                                :id="`${a11yId}-subdomainadvancedauth-2`"
                                class="h-9 w-full min-w-0 rounded-md border border-input bg-background px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                                :value="condition.target"
                                :disabled="saving"
                                @change="
                                  updateTarget(
                                    condition,
                                    ($event.target as HTMLSelectElement)
                                      .value as AdvancedAuthConditionTarget,
                                  )
                                "
                              >
                                <option
                                  v-for="target in targetOptions"
                                  :key="target.value"
                                  :value="target.value"
                                >
                                  {{ t(target.labelKey) }}
                                </option>
                              </select>
                            </div>

                            <div
                              v-if="
                                condition.target === 'request_header' ||
                                condition.target === 'query_parameter'
                              "
                              class="min-w-0 space-y-1.5"
                            >
                              <Label
                                class="text-xs"
                                :for="`advanced-auth-condition-name-${condition.id}`"
                                >{{
                                  condition.target === "request_header"
                                    ? t("admin.advancedAuth.headerName")
                                    : t("admin.advancedAuth.queryName")
                                }}</Label
                              >
                              <AdvancedAuthHeaderNameField
                                v-if="condition.target === 'request_header'"
                                :id="`advanced-auth-condition-name-${condition.id}`"
                                v-model="condition.name"
                                :disabled="saving"
                              />
                              <Input
                                v-else
                                :id="`advanced-auth-condition-name-${condition.id}`"
                                v-model="condition.name"
                                :placeholder="
                                  t('admin.advancedAuth.namePlaceholder')
                                "
                                :disabled="saving"
                              />
                            </div>

                            <div class="min-w-0 space-y-1.5">
                              <Label
                                :for="`${a11yId}-subdomainadvancedauth-3`"
                                class="text-xs"
                                >{{
                                  t("admin.advancedAuth.matchOperator")
                                }}</Label
                              >
                              <select
                                :id="`${a11yId}-subdomainadvancedauth-3`"
                                class="h-9 w-full min-w-0 rounded-md border border-input bg-background px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                                :value="condition.operator"
                                :disabled="saving"
                                @change="
                                  updateOperator(
                                    condition,
                                    ($event.target as HTMLSelectElement)
                                      .value as AdvancedAuthOperator,
                                  )
                                "
                              >
                                <option
                                  v-for="operator in operatorsFor(
                                    condition.target,
                                  )"
                                  :key="operator.value"
                                  :value="operator.value"
                                >
                                  {{ t(operator.labelKey) }}
                                </option>
                              </select>
                            </div>

                            <div
                              v-if="condition.target === 'source_region'"
                              class="min-w-0 space-y-1.5 sm:col-span-2 xl:col-span-1"
                            >
                              <div class="text-xs font-medium">
                                {{ t("admin.advancedAuth.matchValue") }}
                              </div>
                              <CidrRegionSelector
                                v-model="condition.selections"
                                layout="compact"
                                :disabled="saving"
                                :text="regionText"
                                :description="
                                  t('admin.advancedAuth.regionDescription')
                                "
                              />
                            </div>

                            <div
                              v-else-if="needsValue(condition)"
                              class="min-w-0 space-y-1.5"
                              :class="
                                condition.target === 'request_header' ||
                                condition.target === 'query_parameter'
                                  ? 'xl:col-span-1'
                                  : 'sm:col-span-2 xl:col-span-1'
                              "
                            >
                              <Label
                                :for="`${a11yId}-subdomainadvancedauth-4`"
                                class="text-xs"
                                :title="
                                  condition.target === 'http_method'
                                    ? t('admin.advancedAuth.methodHint')
                                    : condition.target === 'source_ip'
                                      ? t(
                                          sourceNetworkTranslationKey(
                                            condition,
                                            'Hint',
                                          ),
                                        )
                                      : t('admin.advancedAuth.valueHint')
                                "
                                >{{
                                  condition.target === "source_ip"
                                    ? t(
                                        sourceNetworkTranslationKey(
                                          condition,
                                          "Label",
                                        ),
                                      )
                                    : t("admin.advancedAuth.matchValue")
                                }}</Label
                              >
                              <Input
                                :id="`${a11yId}-subdomainadvancedauth-4`"
                                :model-value="valueInputText(condition)"
                                :class="
                                  condition.target === 'source_ip'
                                    ? 'font-mono'
                                    : ''
                                "
                                :placeholder="
                                  condition.target === 'source_ip'
                                    ? t(
                                        sourceNetworkTranslationKey(
                                          condition,
                                          'Placeholder',
                                        ),
                                      )
                                    : t('admin.advancedAuth.valuePlaceholder')
                                "
                                :title="
                                  condition.target === 'http_method'
                                    ? t('admin.advancedAuth.methodHint')
                                    : condition.target === 'source_ip'
                                      ? t(
                                          sourceNetworkTranslationKey(
                                            condition,
                                            'Hint',
                                          ),
                                        )
                                      : t('admin.advancedAuth.valueHint')
                                "
                                :disabled="saving"
                                @update:model-value="
                                  condition.target === 'source_ip'
                                    ? setSourceIpValue(
                                        condition,
                                        String($event),
                                      )
                                    : setValueText(condition, String($event))
                                "
                                @blur="normalizeValueDraft(condition)"
                              />
                            </div>
                          </div>

                          <Button
                            variant="ghost"
                            size="icon"
                            class="group absolute top-1.5 right-1.5 h-7 w-7 shrink-0 sm:static sm:mt-5.5 sm:h-8 sm:w-8"
                            :disabled="saving"
                            :aria-label="
                              t('admin.advancedAuth.deleteCondition')
                            "
                            @click="removeCondition(group, conditionIndex)"
                          >
                            <Trash2
                              class="h-4 w-4 text-muted-foreground transition-colors group-hover:text-destructive"
                            />
                          </Button>
                        </div>
                      </div>
                    </template>
                  </div>

                  <div
                    class="flex items-center justify-start"
                    :class="group.conditions.length > 1 ? 'sm:pl-9' : ''"
                  >
                    <Button
                      variant="outline"
                      size="sm"
                      class="w-full min-[480px]:w-auto"
                      :disabled="
                        group.conditions.length >= MAX_CONDITIONS || saving
                      "
                      @click="addCondition(group)"
                    >
                      <Plus class="mr-2 h-4 w-4" />{{
                        t("admin.advancedAuth.addAndCondition")
                      }}
                    </Button>
                  </div>
                </div>
              </div>
            </div>
          </section>

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
                <Label :for="`${a11yId}-subdomainadvancedauth-5`">{{
                  t("admin.advancedAuth.idleTtl")
                }}</Label>
                <div class="relative">
                  <Input
                    :id="`${a11yId}-subdomainadvancedauth-5`"
                    v-model.number="idleHours"
                    class="pr-16"
                    type="number"
                    :min="MIN_TTL_HOURS"
                    :max="MAX_IDLE_TTL_HOURS"
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
                <Label :for="`${a11yId}-subdomainadvancedauth-6`">{{
                  t("admin.advancedAuth.maxLifetime")
                }}</Label>
                <div class="relative">
                  <Input
                    :id="`${a11yId}-subdomainadvancedauth-6`"
                    v-model.number="maxLifetimeHours"
                    class="pr-16"
                    type="number"
                    :min="MIN_TTL_HOURS"
                    :max="MAX_LIFETIME_HOURS"
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

            <div
              class="flex items-start gap-3 rounded-lg bg-muted/40 px-4 py-3"
            >
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
        </div>
      </CardContent>
      <FloatingActionDock
        v-if="!loading && !missing"
        :active="isDirty"
        inline-class="border-t border-border/60 p-5"
      >
        <template #inline
          ><div
            class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
          >
            <p class="text-sm text-muted-foreground">
              {{
                form.enabled && isBroadRule
                  ? t("admin.advancedAuth.broadRuleWarning")
                  : t("admin.advancedAuth.saveHint")
              }}
            </p>
            <div class="flex gap-3 sm:ml-auto">
              <Button variant="outline" :disabled="saving" @click="cancel">{{
                t("common.cancel")
              }}</Button
              ><Button :disabled="!isDirty || saving" @click="save"
                ><Save class="mr-2 h-4 w-4" />{{
                  saving ? t("admin.advancedAuth.saving") : t("common.save")
                }}</Button
              >
            </div>
          </div></template
        >
        <template #floating
          ><Button variant="outline" :disabled="saving" @click="cancel">{{
            t("common.cancel")
          }}</Button
          ><Button :disabled="!isDirty || saving" @click="save"
            ><Save class="mr-2 h-4 w-4" />{{ t("common.save") }}</Button
          ></template
        >
      </FloatingActionDock>
    </Card>

    <ConfirmationDialog
      :open="confirmationDialogOpen"
      v-bind="confirmationDialogOptions"
      @update:open="handleConfirmationDialogOpenChange"
      @confirm="confirmPendingAction"
    />
  </div>
</template>
