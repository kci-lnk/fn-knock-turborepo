<template>
  <Dialog :open="props.open" @update:open="handleOpenChange">
    <DialogContent class="sm:max-w-[720px] max-h-[88vh] overflow-y-auto">
      <DialogHeader>
        <DialogTitle>{{ dialogTitle }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.acmeApplicationDialog.description") }}
        </DialogDescription>
      </DialogHeader>

      <div class="grid gap-6 py-1">
        <div class="grid gap-2">
          <label class="text-sm text-muted-foreground">
            {{ t("admin.acmeApplicationDialog.name") }}
          </label>
          <Input
            v-model.trim="name"
            :disabled="props.pending"
            :placeholder="t('admin.acmeApplicationDialog.namePlaceholder')"
          />
        </div>

        <div class="grid gap-2">
          <div class="flex items-center justify-between gap-3">
            <label class="text-sm text-muted-foreground">
              {{ t("admin.acmeApplicationDialog.domains") }}
            </label>
            <span class="text-xs text-muted-foreground"
              >{{ t("admin.acmeApplicationDialog.domainsHint") }}</span
            >
          </div>
          <TagsInput
            v-model="domains"
            add-on-blur
            class="min-h-[65px]"
            :disabled="props.pending"
          >
            <TagsInputItem v-for="item in domains" :key="item" :value="item">
              <TagsInputItemText />
              <TagsInputItemDelete />
            </TagsInputItem>
            <TagsInputInput
              :disabled="props.pending"
              :placeholder="t('admin.acmeApplicationDialog.domainsPlaceholder')"
            />
          </TagsInput>
        </div>

        <div class="grid gap-2">
          <div class="flex items-center justify-between gap-3">
            <label class="text-sm text-muted-foreground">
              {{ t("admin.acmeApplicationDialog.dnsProvider") }}
            </label>
            <span
              v-if="activeDnsType"
              class="text-xs font-mono text-muted-foreground"
            >
              {{ activeDnsType }}
            </span>
          </div>
          <Select v-model="dnsType" :disabled="props.pending">
            <SelectTrigger class="w-full">
              <SelectValue
                :placeholder="t('admin.acmeApplicationDialog.selectDnsProvider')"
              />
            </SelectTrigger>
            <SelectContent class="max-h-[320px]">
              <SelectGroup
                v-for="group in groupedProviders"
                :key="group.groupKey"
              >
                <SelectLabel>{{ group.group }}</SelectLabel>
                <SelectItem
                  v-for="provider in group.items"
                  :key="provider.dnsType"
                  :value="provider.dnsType"
                >
                  <div class="flex w-full items-center justify-between gap-3">
                    <span class="truncate">{{ provider.label }}</span>
                    <span
                      class="shrink-0 font-mono text-xs text-muted-foreground"
                    >
                      {{ provider.dnsType }}
                    </span>
                  </div>
                </SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </div>

        <div
          v-if="activeCredentialFields.length"
          data-acme-dialog="credentials"
          class="grid gap-4 rounded-xl border bg-muted/15 p-4 sm:p-5"
        >
          <div class="flex items-start justify-between gap-3">
            <div class="grid gap-0.5">
              <div class="flex flex-wrap items-center gap-2">
                <div class="text-sm font-medium">
                  {{ t("admin.acmeApplicationDialog.dnsApiCredentials") }}
                </div>
                <span
                  class="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground"
                >
                  {{ credentialSummary }}
                </span>
              </div>
              <p class="text-xs text-muted-foreground">
                {{ t("admin.acmeApplicationDialog.credentialsDescription") }}
              </p>
              <p
                v-if="hasMultipleCredentialSchemes"
                class="text-xs text-muted-foreground"
              >
                {{ t("admin.acmeApplicationDialog.multipleSchemesDescription") }}
              </p>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              class="text-muted-foreground hover:text-foreground"
              :title="
                isCredentialsVisible
                  ? t('admin.acmeApplicationDialog.hide')
                  : t('admin.acmeApplicationDialog.show')
              "
              :aria-label="
                isCredentialsVisible
                  ? t('admin.acmeApplicationDialog.hideCredentials')
                  : t('admin.acmeApplicationDialog.showCredentials')
              "
              @click="isCredentialsVisible = !isCredentialsVisible"
            >
              <component
                :is="isCredentialsVisible ? EyeOff : Eye"
                class="h-4 w-4"
              />
            </Button>
          </div>

          <div class="grid gap-3">
            <CredentialTransferHint
              v-if="credentialTransferSuggestion"
              :action-label="
                t('admin.acmeApplicationDialog.fillFromSource', {
                  source: transferSourceScopeLabel,
                })
              "
              :description="credentialTransferDescription"
              :fields="
                credentialTransferSuggestion.fillableFields.map(
                  (field) => field.targetKey,
                )
              "
              :loading="isTransferSourceLoading"
              :source-label="`${transferSourceScopeLabel} · ${credentialTransferSuggestion.bridgeLabel}`"
              @apply="applyCredentialTransfer"
            />

            <div class="grid gap-3">
              <div
                v-for="(scheme, schemeIndex) in activeCredentialSchemes"
                :key="scheme.id"
                :class="
                  hasMultipleCredentialSchemes
                    ? 'grid gap-3 rounded-lg border bg-background/60 p-3'
                    : 'grid gap-3'
                "
              >
                <div
                  v-if="hasMultipleCredentialSchemes || scheme.description"
                  class="grid gap-1"
                >
                  <div
                    v-if="hasMultipleCredentialSchemes"
                    class="text-xs font-medium text-foreground"
                  >
                    {{ scheme.label }}
                  </div>
                  <p
                    v-if="scheme.description"
                    class="text-[11px] leading-5 text-muted-foreground"
                  >
                    {{ scheme.description }}
                  </p>
                </div>

                <div class="grid gap-3">
                  <div
                    v-for="(field, fieldIndex) in scheme.fields"
                    :key="field.key"
                    class="grid gap-2"
                  >
                    <div class="flex items-center justify-between gap-2">
                      <span class="text-sm font-mono text-muted-foreground">
                        {{ field.key }}
                      </span>
                      <span
                        v-if="field.required === false"
                        class="text-[11px] text-muted-foreground"
                      >
                        {{ t("admin.acmeApplicationDialog.optional") }}
                      </span>
                    </div>
                    <Input
                      v-model.trim="credentials[field.key]"
                      :type="isCredentialsVisible ? 'text' : 'password'"
                      class="font-mono"
                      :name="`acme-credential-${schemeIndex}-${fieldIndex}`"
                      autocomplete="new-password"
                      :readonly="!isCredentialEditReady(field.key)"
                      :disabled="props.pending"
                      @focus="enableCredentialEditing(field.key)"
                      @pointerdown="enableCredentialEditing(field.key)"
                    />
                    <p
                      v-if="field.description"
                      class="text-[11px] leading-5 text-muted-foreground"
                    >
                      {{ field.description }}
                    </p>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div
          class="flex items-center justify-between rounded-lg border bg-muted/20 px-4 py-3"
        >
          <div class="grid gap-0.5">
            <div class="text-sm font-medium">
              {{ t("admin.acmeApplicationDialog.autoRenew") }}
            </div>
            <div class="text-xs text-muted-foreground">
              {{ t("admin.acmeApplicationDialog.autoRenewDescription") }}
            </div>
          </div>
          <Switch v-model="renewEnabled" :disabled="props.pending" />
        </div>
      </div>

      <DialogFooter class="gap-2 sm:justify-end">
        <Button
          type="button"
          variant="outline"
          :disabled="props.pending"
          @click="handleOpenChange(false)"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          type="button"
          variant="secondary"
          :disabled="!canSubmit || props.pending"
          @click="submit(false)"
        >
          {{ t("common.save") }}
        </Button>
        <Button
          type="button"
          :disabled="!canSubmit || props.pending"
          @click="submit(true)"
        >
          {{ t("admin.acmeApplicationDialog.saveAndApply") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Eye, EyeOff } from "lucide-vue-next";
import type {
  AcmeApplicationPayload,
  AcmeApplicationRecord,
  AcmeDnsProvider,
} from "@/lib/api";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  TagsInput,
  TagsInputInput,
  TagsInputItem,
  TagsInputItemDelete,
  TagsInputItemText,
} from "@/components/ui/tags-input";
import CredentialTransferHint from "@/components/CredentialTransferHint.vue";
import { useDnsCredentialTransfer } from "@/composables/useDnsCredentialTransfer";
import { toast } from "@admin-shared/utils/toast";

type DnsCredentialField = {
  key: string;
  label?: string;
  description?: string;
  required?: boolean;
};

type DnsCredentialScheme = {
  id: string;
  label: string;
  description?: string;
  fields: DnsCredentialField[];
};

const { locale, t } = useI18n();

const props = defineProps<{
  open: boolean;
  mode: "create" | "edit";
  initialValue?: AcmeApplicationRecord | null;
  dnsProviders: AcmeDnsProvider[];
  pending?: boolean;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  submit: [payload: AcmeApplicationPayload];
}>();

const name = ref("");
const domains = ref<string[]>([]);
const dnsType = ref("");
const credentials = ref<Record<string, string>>({});
const renewEnabled = ref(true);
const isCredentialsVisible = ref(false);
const credentialEditReady = ref<Record<string, boolean>>({});

const activeProvider = computed(() => {
  return (
    props.dnsProviders.find((provider) => provider.dnsType === dnsType.value) ||
    null
  );
});

const activeDnsType = computed(() => dnsType.value.trim());

const getProviderCredentialFields = (provider: AcmeDnsProvider | null) => {
  if (!provider) return [] as DnsCredentialField[];

  const fields: DnsCredentialField[] = [];
  const seen = new Set<string>();

  for (const scheme of provider.credentialSchemes) {
    for (const field of scheme.fields) {
      if (seen.has(field.key)) continue;
      seen.add(field.key);
      fields.push(field);
    }
  }

  return fields;
};

const getSatisfiedCredentialScheme = (
  provider: AcmeDnsProvider | null,
  values: Record<string, string>,
) => {
  if (!provider) return null;

  return (
    provider.credentialSchemes.find((scheme) =>
      scheme.fields
        .filter((field) => field.required !== false)
        .every((field) => Boolean((values[field.key] || "").trim())),
    ) || null
  );
};

const activeCredentialSchemes = computed<DnsCredentialScheme[]>(
  () => activeProvider.value?.credentialSchemes || [],
);
const activeCredentialFields = computed(() =>
  getProviderCredentialFields(activeProvider.value),
);
const hasMultipleCredentialSchemes = computed(
  () => activeCredentialSchemes.value.length > 1,
);
const matchedCredentialScheme = computed(() =>
  getSatisfiedCredentialScheme(activeProvider.value, credentials.value),
);
const filledCredentialCount = computed(() => {
  return activeCredentialFields.value.filter(
    (field) => (credentials.value[field.key] || "").trim().length > 0,
  ).length;
});

const credentialSummary = computed(() => {
  if (!activeCredentialFields.value.length) {
    return t("admin.acmeApplicationDialog.noExtraCredentials");
  }
  if (matchedCredentialScheme.value) {
    return t("admin.acmeApplicationDialog.schemeSatisfied", {
      label: matchedCredentialScheme.value.label,
    });
  }
  if (!filledCredentialCount.value) {
    return hasMultipleCredentialSchemes.value
      ? t("admin.acmeApplicationDialog.schemeCount", {
          count: activeCredentialSchemes.value.length,
        })
      : t("admin.acmeApplicationDialog.requiredFieldCount", {
          count: activeCredentialFields.value.length,
        });
  }
  if (hasMultipleCredentialSchemes.value) {
    return t("admin.acmeApplicationDialog.filledAnyScheme", {
      count: filledCredentialCount.value,
    });
  }
  return t("admin.acmeApplicationDialog.filledFieldCount", {
    filled: filledCredentialCount.value,
    total: activeCredentialFields.value.length,
  });
});

const providerGroupKey = (group?: string | null) => {
  if (group === "\u5e38\u7528") return "common";
  if (group === "\u56fd\u5185") return "china";
  if (group === "\u56fd\u9645") return "international";
  if (group === "\u81ea\u5efa/\u9ad8\u7ea7") return "customAdvanced";
  if (!group || group === "\u5176\u4ed6") return "other";
  return group;
};

const providerGroupLabel = (key: string) => {
  if (
    key === "common" ||
    key === "china" ||
    key === "international" ||
    key === "customAdvanced" ||
    key === "other"
  ) {
    return t(`admin.acmeApplicationDialog.providerGroups.${key}`);
  }
  return key;
};

const groupedProviders = computed(() => {
  const groupOrder = ["common", "china", "international", "customAdvanced"];
  const bucket = new Map<string, AcmeDnsProvider[]>();
  for (const provider of props.dnsProviders) {
    const group = providerGroupKey(provider.group);
    if (!bucket.has(group)) bucket.set(group, []);
    bucket.get(group)!.push(provider);
  }

  const groups = Array.from(bucket.entries()).map(([group, items]) => ({
    group: providerGroupLabel(group),
    groupKey: group,
    items: items
      .slice()
      .sort((a, b) => a.label.localeCompare(b.label, locale.value)),
  }));

  groups.sort((a, b) => {
    const ai = groupOrder.indexOf(a.groupKey);
    const bi = groupOrder.indexOf(b.groupKey);
    if (ai === -1 && bi === -1) {
      return a.group.localeCompare(b.group, locale.value);
    }
    if (ai === -1) return 1;
    if (bi === -1) return -1;
    return ai - bi;
  });

  return groups;
});

const dialogTitle = computed(() => {
  return props.mode === "edit"
    ? t("admin.acmeApplicationDialog.editTitle")
    : t("admin.acmeApplicationDialog.createTitle");
});

const getCredentialStateKey = (key: string) => `${activeDnsType.value}:${key}`;

const enableCredentialEditing = (key: string) => {
  credentialEditReady.value[getCredentialStateKey(key)] = true;
};

const isCredentialEditReady = (key: string) =>
  credentialEditReady.value[getCredentialStateKey(key)] === true;

const {
  applySuggestion: applyTransferredCredentials,
  isLoadingSource: isTransferSourceLoading,
  sourceScopeLabel: transferSourceScopeLabel,
  suggestion: credentialTransferSuggestion,
} = useDnsCredentialTransfer({
  target: "acme",
  providerId: activeDnsType,
  targetCredentials: credentials,
});

const credentialTransferDescription = computed(() => {
  const suggestion = credentialTransferSuggestion.value;
  if (!suggestion) return "";
  return t("admin.acmeApplicationDialog.transferDescription", {
    source: transferSourceScopeLabel.value,
    bridge: suggestion.bridgeLabel,
    count: suggestion.fillableFields.length,
  });
});

const canSubmit = computed(() => {
  if (!domains.value.length) return false;
  if (!/^dns_[a-z0-9_]+$/i.test(activeDnsType.value)) return false;
  if (!activeCredentialFields.value.length) return true;
  return Boolean(matchedCredentialScheme.value);
});

const buildCredentialsPayload = () => {
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(credentials.value || {})) {
    const normalizedKey = key.trim();
    const normalizedValue = String(value ?? "").trim();
    if (!normalizedKey || !normalizedValue) continue;
    out[normalizedKey] = normalizedValue;
  }
  return out;
};

const syncForm = () => {
  const initialValue = props.initialValue;
  name.value = initialValue?.name || "";
  domains.value = Array.isArray(initialValue?.domains)
    ? [...initialValue!.domains]
    : [];
  dnsType.value = initialValue?.dnsType || "";
  credentials.value = { ...(initialValue?.credentials || {}) };
  renewEnabled.value = initialValue?.renewEnabled ?? true;
  isCredentialsVisible.value = false;
  credentialEditReady.value = {};
};

const handleOpenChange = (nextOpen: boolean) => {
  emit("update:open", nextOpen);
};

const submit = (submitNow: boolean) => {
  if (!canSubmit.value) return;
  emit("submit", {
    name: name.value.trim() || undefined,
    domains: domains.value,
    dnsType: activeDnsType.value,
    credentials: buildCredentialsPayload(),
    renewEnabled: renewEnabled.value,
    submitNow,
  });
};

const applyCredentialTransfer = () => {
  const result = applyTransferredCredentials();
  if (!result) return;

  for (const key of result.appliedKeys) {
    enableCredentialEditing(key);
  }

  toast.success(
    t("admin.acmeApplicationDialog.transferApplied", {
      source: transferSourceScopeLabel.value,
      count: result.count,
    }),
  );
};

watch(
  () => [props.open, props.initialValue] as const,
  ([open]) => {
    if (!open) return;
    syncForm();
  },
  { immediate: true },
);

watch(dnsType, () => {
  credentialEditReady.value = {};
  const keys = getProviderCredentialFields(activeProvider.value).map(
    (field) => field.key,
  );
  if (!keys.length) {
    credentials.value = {};
    isCredentialsVisible.value = false;
    return;
  }

  const previous = { ...(credentials.value || {}) };
  const next: Record<string, string> = {};
  for (const key of keys) {
    next[key] = typeof previous[key] === "string" ? previous[key] : "";
  }
  credentials.value = next;
  isCredentialsVisible.value = false;
});
</script>
