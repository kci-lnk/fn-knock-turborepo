<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from "vue";
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
import {
  formatAdvancedAuthValueList,
  getSourceNetworkValidationIssue,
  parseAdvancedAuthValueList,
  parseSourceNetworkTextarea,
  sourceNetworkInputKind,
} from "./advanced-auth-source-network";
import { isHttpTargetUrl, normalizeHostLike } from "./model";
import { useConfigStore } from "../../store/config";
import { getCidrRegionSelectionLabel } from "../../types/cidr";
import AdvancedAuthHeaderNameField from "./AdvancedAuthHeaderNameField.vue";
import type {
  AdvancedAuthCondition,
  AdvancedAuthConditionTarget,
  AdvancedAuthConfig,
  AdvancedAuthOperator,
  AdvancedAuthRuleGroup,
} from "../../types";

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

const MAX_GROUPS = 16;
const MAX_CONDITIONS = 16;
const SECONDS_PER_MINUTE = 60;
const SECONDS_PER_HOUR = 60 * SECONDS_PER_MINUTE;
const MIN_TTL_SECONDS = 5 * SECONDS_PER_MINUTE;
const MAX_IDLE_TTL_SECONDS = 30 * 24 * SECONDS_PER_HOUR;
const MAX_LIFETIME_SECONDS = 365 * 24 * SECONDS_PER_HOUR;
// Keep the shortest supported value representable in the two-decimal hour
// input. The setter rounds it back to the exact five-minute API boundary.
const MIN_TTL_HOURS = Number((MIN_TTL_SECONDS / SECONDS_PER_HOUR).toFixed(2));
const MAX_IDLE_TTL_HOURS = MAX_IDLE_TTL_SECONDS / SECONDS_PER_HOUR;
const MAX_LIFETIME_HOURS = MAX_LIFETIME_SECONDS / SECONDS_PER_HOUR;
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

const targetOptions: Array<{
  value: AdvancedAuthConditionTarget;
  labelKey: string;
}> = [
  { value: "source_ip", labelKey: "admin.advancedAuth.targetSourceIp" },
  { value: "source_region", labelKey: "admin.advancedAuth.targetSourceRegion" },
  { value: "url_path", labelKey: "admin.advancedAuth.targetUrlPath" },
  {
    value: "request_header",
    labelKey: "admin.advancedAuth.targetRequestHeader",
  },
  {
    value: "query_parameter",
    labelKey: "admin.advancedAuth.targetQueryParameter",
  },
  { value: "http_method", labelKey: "admin.advancedAuth.targetHttpMethod" },
];

const operatorsByTarget: Record<
  AdvancedAuthConditionTarget,
  Array<{ value: AdvancedAuthOperator; labelKey: string }>
> = {
  source_ip: [
    { value: "equals", labelKey: "admin.advancedAuth.operatorEquals" },
    { value: "not_equals", labelKey: "admin.advancedAuth.operatorNotEquals" },
    { value: "in_cidr", labelKey: "admin.advancedAuth.operatorInCidr" },
    { value: "not_in_cidr", labelKey: "admin.advancedAuth.operatorNotInCidr" },
  ],
  source_region: [
    { value: "in", labelKey: "admin.advancedAuth.operatorInRegion" },
    { value: "not_in", labelKey: "admin.advancedAuth.operatorNotInRegion" },
  ],
  url_path: [
    { value: "equals", labelKey: "admin.advancedAuth.operatorEquals" },
    { value: "not_equals", labelKey: "admin.advancedAuth.operatorNotEquals" },
    { value: "prefix", labelKey: "admin.advancedAuth.operatorPrefix" },
    { value: "not_prefix", labelKey: "admin.advancedAuth.operatorNotPrefix" },
    { value: "contains", labelKey: "admin.advancedAuth.operatorContains" },
    {
      value: "not_contains",
      labelKey: "admin.advancedAuth.operatorNotContains",
    },
    { value: "regex", labelKey: "admin.advancedAuth.operatorRegex" },
    { value: "not_regex", labelKey: "admin.advancedAuth.operatorNotRegex" },
  ],
  request_header: [
    { value: "exists", labelKey: "admin.advancedAuth.operatorExists" },
    { value: "not_exists", labelKey: "admin.advancedAuth.operatorNotExists" },
    { value: "equals", labelKey: "admin.advancedAuth.operatorEquals" },
    { value: "not_equals", labelKey: "admin.advancedAuth.operatorNotEquals" },
    { value: "contains", labelKey: "admin.advancedAuth.operatorContains" },
    {
      value: "not_contains",
      labelKey: "admin.advancedAuth.operatorNotContains",
    },
    { value: "starts_with", labelKey: "admin.advancedAuth.operatorStartsWith" },
    {
      value: "not_starts_with",
      labelKey: "admin.advancedAuth.operatorNotStartsWith",
    },
    { value: "ends_with", labelKey: "admin.advancedAuth.operatorEndsWith" },
    {
      value: "not_ends_with",
      labelKey: "admin.advancedAuth.operatorNotEndsWith",
    },
    { value: "regex", labelKey: "admin.advancedAuth.operatorRegex" },
    { value: "not_regex", labelKey: "admin.advancedAuth.operatorNotRegex" },
  ],
  query_parameter: [
    { value: "exists", labelKey: "admin.advancedAuth.operatorExists" },
    { value: "not_exists", labelKey: "admin.advancedAuth.operatorNotExists" },
    { value: "equals", labelKey: "admin.advancedAuth.operatorEquals" },
    { value: "not_equals", labelKey: "admin.advancedAuth.operatorNotEquals" },
    { value: "contains", labelKey: "admin.advancedAuth.operatorContains" },
    {
      value: "not_contains",
      labelKey: "admin.advancedAuth.operatorNotContains",
    },
    { value: "starts_with", labelKey: "admin.advancedAuth.operatorStartsWith" },
    {
      value: "not_starts_with",
      labelKey: "admin.advancedAuth.operatorNotStartsWith",
    },
    { value: "ends_with", labelKey: "admin.advancedAuth.operatorEndsWith" },
    {
      value: "not_ends_with",
      labelKey: "admin.advancedAuth.operatorNotEndsWith",
    },
    { value: "regex", labelKey: "admin.advancedAuth.operatorRegex" },
    { value: "not_regex", labelKey: "admin.advancedAuth.operatorNotRegex" },
  ],
  http_method: [
    { value: "in", labelKey: "admin.advancedAuth.operatorMethodIn" },
    { value: "not_in", labelKey: "admin.advancedAuth.operatorMethodNotIn" },
  ],
};

const newId = (prefix: string) =>
  `${prefix}-${Math.random().toString(36).slice(2, 10)}-${Date.now().toString(36)}`;

const blankCondition = (): AdvancedAuthCondition => ({
  id: newId("condition"),
  target: "source_ip",
  operator: "equals",
  values: [""],
  selections: [],
});

const blankGroup = (): AdvancedAuthRuleGroup => ({
  id: newId("group"),
  conditions: [blankCondition()],
});

const cloneCondition = (
  condition: AdvancedAuthCondition,
): AdvancedAuthCondition => {
  const compiledValues = condition.cidrs ?? [];
  const values = condition.values?.length
    ? [...condition.values]
    : condition.target === "source_ip"
      ? compiledValues.map((value) =>
          condition.operator === "equals" || condition.operator === "not_equals"
            ? value.replace(/\/(32|128)$/, "")
            : value,
        )
      : [];
  return {
    ...condition,
    values,
    selections: (condition.selections ?? []).map((selection) => ({
      ...selection,
      // Stored/imported metadata may contain a geography-only label even
      // though the carrier is present as a separate field. Normalize at both
      // load and save boundaries so the selected tag and persisted draft
      // always identify the carrier.
      label: getCidrRegionSelectionLabel(selection),
    })),
    cidrs: [...compiledValues],
  };
};

const cloneConfig = (config: AdvancedAuthConfig): AdvancedAuthConfig => ({
  enabled: config.enabled === true,
  idle_ttl_seconds: Number(config.idle_ttl_seconds) || 24 * 60 * 60,
  max_lifetime_seconds:
    Number(config.max_lifetime_seconds) || 30 * 24 * 60 * 60,
  policy_version: config.policy_version,
  groups: (config.groups ?? []).map((group) => ({
    id: group.id,
    conditions: (group.conditions ?? []).map(cloneCondition),
  })),
});

const sourceIpDisplayValue = (condition: AdvancedAuthCondition) => {
  const values = condition.values?.length
    ? condition.values
    : (condition.cidrs ?? []);
  return formatAdvancedAuthValueList(
    values.map((value) => {
      if (
        condition.operator === "equals" ||
        condition.operator === "not_equals"
      ) {
        return value.replace(/\/(32|128)$/, "");
      }
      return value;
    }),
  );
};

const setSourceIpValue = (condition: AdvancedAuthCondition, value: string) => {
  valueDrafts[condition.id] = value;
  condition.values = parseSourceNetworkTextarea(value);
};

const sourceNetworkTranslationKey = (
  condition: AdvancedAuthCondition,
  suffix: "Label" | "Placeholder" | "Hint",
) =>
  `admin.advancedAuth.source${sourceNetworkInputKind(condition.operator) === "address" ? "Ip" : "Cidr"}${suffix}`;

const valueText = (condition: AdvancedAuthCondition) =>
  formatAdvancedAuthValueList(condition.values ?? []);
const setValueText = (condition: AdvancedAuthCondition, value: string) => {
  valueDrafts[condition.id] = value;
  condition.values = parseAdvancedAuthValueList(value);
};
const valueInputText = (condition: AdvancedAuthCondition) =>
  valueDrafts[condition.id] ??
  (condition.target === "source_ip"
    ? sourceIpDisplayValue(condition)
    : valueText(condition));
const normalizeValueDraft = (condition: AdvancedAuthCondition) => {
  valueDrafts[condition.id] =
    condition.target === "source_ip"
      ? sourceIpDisplayValue(condition)
      : valueText(condition);
};
const clearValueDraft = (condition: AdvancedAuthCondition) => {
  delete valueDrafts[condition.id];
};
const needsValue = (condition: AdvancedAuthCondition) =>
  condition.target !== "source_region" &&
  condition.operator !== "exists" &&
  condition.operator !== "not_exists";

const operatorsFor = (target: AdvancedAuthConditionTarget) =>
  operatorsByTarget[target];

const updateTarget = (
  condition: AdvancedAuthCondition,
  target: AdvancedAuthConditionTarget,
) => {
  clearValueDraft(condition);
  condition.target = target;
  condition.operator = operatorsByTarget[target][0]?.value ?? "equals";
  condition.values = target === "source_region" ? [] : [""];
  condition.selections = [];
  condition.cidrs = undefined;
};

const updateOperator = (
  condition: AdvancedAuthCondition,
  operator: AdvancedAuthOperator,
) => {
  clearValueDraft(condition);
  condition.operator = operator;
  if (operator === "exists" || operator === "not_exists") condition.values = [];
};

const secondsToHourInput = (seconds: number) => {
  const hours = seconds / SECONDS_PER_HOUR;
  return Number.isInteger(hours) ? hours : Number(hours.toFixed(2));
};

const hourInputToSeconds = (value: number, maximum: number) => {
  const hours = Number(value);
  if (!Number.isFinite(hours)) return MIN_TTL_SECONDS;
  return Math.min(
    maximum,
    Math.max(MIN_TTL_SECONDS, Math.round(hours * 60) * SECONDS_PER_MINUTE),
  );
};

const idleHours = computed({
  get: () => secondsToHourInput(form.idle_ttl_seconds),
  set: (value: number) => {
    form.idle_ttl_seconds = hourInputToSeconds(value, MAX_IDLE_TTL_SECONDS);
  },
});
const maxLifetimeHours = computed({
  get: () => secondsToHourInput(form.max_lifetime_seconds),
  set: (value: number) => {
    form.max_lifetime_seconds = hourInputToSeconds(value, MAX_LIFETIME_SECONDS);
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

const snapshotConfig = () =>
  JSON.stringify({
    enabled: form.enabled,
    idle_ttl_seconds: form.idle_ttl_seconds,
    max_lifetime_seconds: form.max_lifetime_seconds,
    groups: form.groups,
  });
const isDirty = computed(() => snapshotConfig() !== savedSnapshot.value);

const isBroadRule = computed(() =>
  form.groups.some((group) => {
    const conditions = group.conditions;
    if (!conditions.length) return false;
    if (conditions.every((condition) => condition.operator.startsWith("not_")))
      return true;
    if (conditions.length === 1 && conditions[0]?.target === "http_method")
      return true;
    return conditions.some(
      (condition) =>
        (condition.target === "url_path" &&
          (condition.operator === "prefix" ||
            condition.operator === "not_prefix") &&
          (condition.values ?? []).includes("/")) ||
        (condition.target === "source_ip" &&
          (condition.values ?? []).some((value) =>
            ["0.0.0.0/0", "::/0"].includes(value.trim()),
          )),
    );
  }),
);

const addGroup = () => {
  if (form.groups.length >= MAX_GROUPS) return;
  form.groups.push(blankGroup());
};
const removeGroup = (groupIndex: number) => {
  form.groups[groupIndex]?.conditions.forEach(clearValueDraft);
  form.groups.splice(groupIndex, 1);
};
const addCondition = (group: AdvancedAuthRuleGroup) => {
  if (group.conditions.length >= MAX_CONDITIONS) return;
  group.conditions.push(blankCondition());
};
const removeCondition = (group: AdvancedAuthRuleGroup, index: number) => {
  const condition = group.conditions[index];
  if (condition) clearValueDraft(condition);
  group.conditions.splice(index, 1);
};

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
  if (form.enabled) {
    if (form.groups.length === 0) {
      toast.error(t("admin.advancedAuth.invalidRules"));
      return;
    }
    if (form.groups.some((group) => group.conditions.length === 0)) {
      toast.error(t("admin.advancedAuth.emptyGroup"));
      return;
    }
    const conditions = form.groups.flatMap((group) => group.conditions);
    const invalidSourceNetwork = conditions
      .filter((condition) => condition.target === "source_ip")
      .map((condition) =>
        getSourceNetworkValidationIssue(
          condition.values ?? [],
          condition.operator,
        ),
      )
      .find((issue) => issue != null);
    if (invalidSourceNetwork) {
      toast.error(
        t(
          invalidSourceNetwork.kind === "address"
            ? "admin.advancedAuth.invalidSourceIpLine"
            : "admin.advancedAuth.invalidSourceCidrLine",
          { line: invalidSourceNetwork.line },
        ),
      );
      return;
    }
    const invalidCondition = conditions.find(
      (condition) =>
        (condition.target === "source_region" &&
          (condition.selections ?? []).length === 0) ||
        ((condition.target === "request_header" ||
          condition.target === "query_parameter") &&
          !condition.name?.trim()) ||
        (needsValue(condition) &&
          ((condition.values ?? []).length === 0 ||
            (condition.values ?? []).some((value) => !value.trim()))),
    );
    if (invalidCondition) {
      toast.error(t("admin.advancedAuth.invalidCondition"));
      return;
    }
  }
  if (form.max_lifetime_seconds < form.idle_ttl_seconds) {
    toast.error(t("admin.advancedAuth.maxLifetimeTooShort"));
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
              <Label class="text-base">{{
                t("admin.advancedAuth.enabled")
              }}</Label>
              <p class="mt-1 text-sm leading-6 text-muted-foreground">
                {{ t("admin.advancedAuth.enabledDescription") }}
              </p>
            </div>
            <Switch v-model:model-value="form.enabled" :disabled="saving" />
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
                              <Label class="text-xs">{{
                                t("admin.advancedAuth.matchTarget")
                              }}</Label>
                              <select
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
                              <Label class="text-xs">{{
                                t("admin.advancedAuth.matchOperator")
                              }}</Label>
                              <select
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
                              <Label class="text-xs">{{
                                t("admin.advancedAuth.matchValue")
                              }}</Label>
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
                <Label>{{ t("admin.advancedAuth.idleTtl") }}</Label>
                <div class="relative">
                  <Input
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
                <Label>{{ t("admin.advancedAuth.maxLifetime") }}</Label>
                <div class="relative">
                  <Input
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
