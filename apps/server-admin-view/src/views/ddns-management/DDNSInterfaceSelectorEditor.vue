<script setup lang="ts">
import { computed, ref, watch } from "vue";
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
import { Switch } from "@/components/ui/switch";
import {
  DDNSAPI,
  type DDNSInterfaceSelector,
  type DDNSInterfaceSelectorPreviewPayload,
  type DDNSNetworkInterfacePayload,
} from "@/lib/api/ddns";
import {
  createDefaultInterfaceSelector,
  buildInterfaceAddressCandidates,
  buildInterfaceSelectorFromLegacyIndex,
  parseInterfaceSelector,
  serializeInterfaceSelector,
} from "./model";

const NO_PREFERENCE = "__none__";
const PREVIEW_DEBOUNCE_MS = 250;

const props = defineProps<{
  allowPrivateAddresses: boolean;
  currentAddress?: string | null;
  family: "ipv4" | "ipv6";
  idPrefix: string;
  legacyIndex?: string;
  modelValue?: string;
  networkInterface: DDNSNetworkInterfacePayload | null;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const { t } = useI18n();
const selector = ref<DDNSInterfaceSelector>(createDefaultInterfaceSelector());
const preview = ref<DDNSInterfaceSelectorPreviewPayload | null>(null);
const previewError = ref("");
const previewing = ref(false);
const migratedLegacy = ref(false);
let lastEmitted = "";
let previewSequence = 0;

const displayedCandidates = computed(() =>
  buildInterfaceAddressCandidates(props.networkInterface, true).filter(
    (item) => item.family === props.family,
  ),
);
const privateCandidateKeys = computed(
  () =>
    new Set(
      (props.networkInterface?.privateAddresses || []).map(
        (item) => `${item.family}:${item.address}`,
      ),
    ),
);
const isPrivateCandidate = (
  item: DDNSNetworkInterfacePayload["selectableAddresses"][number],
) => privateCandidateKeys.value.has(`${item.family}:${item.address}`);
const candidateSignature = computed(() =>
  displayedCandidates.value
    .map(
      (item) =>
        `${item.address}:${String(item.temporary)}:${String(item.deprecated)}:${String(item.tentative)}:${String(item.dadFailed)}`,
    )
    .concat(String(props.allowPrivateAddresses))
    .join("|"),
);

const buildInitialSelector = () => {
  const parsed = parseInterfaceSelector(props.modelValue);
  if (parsed) {
    return parsed;
  }
  const legacy = buildInterfaceSelectorFromLegacyIndex(
    props.networkInterface,
    props.family,
    props.legacyIndex,
    props.currentAddress,
    props.allowPrivateAddresses,
  );
  migratedLegacy.value = legacy.migrated;
  return legacy.selector;
};

const emitSelector = () => {
  const serialized = serializeInterfaceSelector(selector.value);
  if (serialized === lastEmitted && serialized === props.modelValue) return;
  lastEmitted = serialized;
  emit("update:modelValue", serialized);
};

watch(
  () => [
    props.modelValue,
    props.legacyIndex,
    props.currentAddress,
    candidateSignature.value,
  ],
  () => {
    if (!props.networkInterface) return;
    const next = buildInitialSelector();
    const serialized = serializeInterfaceSelector(next);
    if (serialized !== serializeInterfaceSelector(selector.value)) {
      selector.value = next;
    }
    const hasLegacyIndex = Boolean(props.legacyIndex?.trim());
    if (
      !parseInterfaceSelector(props.modelValue) &&
      (!hasLegacyIndex || migratedLegacy.value)
    ) {
      emitSelector();
    }
  },
  { immediate: true },
);

const patchSelector = (patch: Partial<DDNSInterfaceSelector>) => {
  selector.value = { ...selector.value, ...patch };
  emitSelector();
};

const parseCidrList = (value: string) =>
  value
    .split(/[\s,，;；]+/)
    .map((item) => item.trim())
    .filter(Boolean);

const updatePreferredAddress = (value: string) => {
  if (value === NO_PREFERENCE) {
    const { preferredAddress: _ignored, ...rest } = selector.value;
    selector.value = rest as DDNSInterfaceSelector;
    emitSelector();
    return;
  }
  patchSelector({ preferredAddress: value });
};

const addressStatusLabel = (
  item: DDNSNetworkInterfacePayload["selectableAddresses"][number],
) => {
  const labels = isPrivateCandidate(item)
    ? [t("admin.ddns.selectorStatus.private")]
    : [];
  if (props.family === "ipv4") {
    labels.push(t("admin.ddns.selectorStatus.stable"));
  } else if (item.dadFailed) {
    labels.push(t("admin.ddns.selectorStatus.dadFailed"));
  } else if (item.tentative) {
    labels.push(t("admin.ddns.selectorStatus.tentative"));
  } else if (item.deprecated) {
    labels.push(t("admin.ddns.selectorStatus.deprecated"));
  } else if (item.temporary) {
    labels.push(t("admin.ddns.selectorStatus.temporary"));
  } else if (item.temporary === false) {
    labels.push(t("admin.ddns.selectorStatus.stable"));
  } else {
    labels.push(t("admin.ddns.selectorStatus.unknown"));
  }
  return labels.join(" · ");
};

watch(
  [
    () => props.networkInterface?.name,
    () => candidateSignature.value,
    () => props.currentAddress,
    () => serializeInterfaceSelector(selector.value),
  ],
  (_, __, onCleanup) => {
    const sequence = ++previewSequence;
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    preview.value = null;
    previewError.value = "";
    previewing.value = false;
    if (!props.networkInterface?.name) return;
    timer = setTimeout(async () => {
      previewing.value = true;
      try {
        const result = await DDNSAPI.resolveInterfaceSelector(
          {
            networkInterface: props.networkInterface!.name,
            family: props.family,
            selector: selector.value,
            currentAddress: props.currentAddress,
            allowPrivateAddresses: props.allowPrivateAddresses,
          },
          controller.signal,
        );
        if (sequence === previewSequence) preview.value = result;
      } catch (error: any) {
        if (sequence === previewSequence && !controller.signal.aborted) {
          previewError.value =
            error?.response?.data?.message ||
            error?.message ||
            t("admin.ddns.selectorPreviewFailed");
        }
      } finally {
        if (sequence === previewSequence) previewing.value = false;
      }
    }, PREVIEW_DEBOUNCE_MS);
    onCleanup(() => {
      if (timer !== undefined) clearTimeout(timer);
      controller.abort();
    });
  },
  { immediate: true },
);
</script>

<template>
  <div class="w-full max-w-md space-y-3">
    <div
      v-if="migratedLegacy"
      class="rounded-md border border-amber-500/30 bg-amber-500/5 px-3 py-2 text-xs text-amber-700 dark:text-amber-300"
    >
      {{ t("admin.ddns.selectorLegacyMigrated") }}
    </div>

    <div class="space-y-1.5">
      <Label :for="`${idPrefix}-mode`" class="text-xs text-muted-foreground">
        {{ t("admin.ddns.selectorModeLabel") }}
      </Label>
      <Select
        :model-value="selector.mode"
        @update:model-value="
          (value: any) =>
            patchSelector({ mode: value === 'rules' ? 'rules' : 'auto' })
        "
      >
        <SelectTrigger :id="`${idPrefix}-mode`">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="auto">
            {{ t("admin.ddns.selectorMode.auto") }}
          </SelectItem>
          <SelectItem value="rules">
            {{ t("admin.ddns.selectorMode.rules") }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>

    <div class="space-y-1.5">
      <Label
        :for="`${idPrefix}-preferred`"
        class="text-xs text-muted-foreground"
      >
        {{ t("admin.ddns.selectorPreferredLabel") }}
      </Label>
      <Select
        :model-value="selector.preferredAddress || NO_PREFERENCE"
        :disabled="displayedCandidates.length === 0"
        @update:model-value="
          (value: any) => updatePreferredAddress(String(value))
        "
      >
        <SelectTrigger :id="`${idPrefix}-preferred`">
          <SelectValue :placeholder="t('admin.ddns.selectorNoPreference')" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem :value="NO_PREFERENCE">
            {{ t("admin.ddns.selectorNoPreference") }}
          </SelectItem>
          <SelectItem
            v-for="item in displayedCandidates"
            :key="item.address"
            :value="item.address"
            :disabled="isPrivateCandidate(item) && !allowPrivateAddresses"
          >
            {{ item.address }} · {{ addressStatusLabel(item) }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>

    <template v-if="selector.mode === 'rules'">
      <div class="grid gap-3 sm:grid-cols-2">
        <div class="space-y-1.5">
          <Label
            :for="`${idPrefix}-include`"
            class="text-xs text-muted-foreground"
          >
            {{ t("admin.ddns.selectorIncludeCidrs") }}
          </Label>
          <Input
            :id="`${idPrefix}-include`"
            :model-value="(selector.includeCidrs || []).join(', ')"
            :placeholder="
              family === 'ipv6' ? '2001:db8::/32' : '203.0.113.0/24'
            "
            @update:model-value="
              (value: string | number) =>
                patchSelector({ includeCidrs: parseCidrList(String(value)) })
            "
          />
        </div>
        <div class="space-y-1.5">
          <Label
            :for="`${idPrefix}-exclude`"
            class="text-xs text-muted-foreground"
          >
            {{ t("admin.ddns.selectorExcludeCidrs") }}
          </Label>
          <Input
            :id="`${idPrefix}-exclude`"
            :model-value="(selector.excludeCidrs || []).join(', ')"
            @update:model-value="
              (value: string | number) =>
                patchSelector({ excludeCidrs: parseCidrList(String(value)) })
            "
          />
        </div>
      </div>

      <div v-if="family === 'ipv6'" class="space-y-1.5">
        <Label
          :for="`${idPrefix}-interface-id`"
          class="text-xs text-muted-foreground"
        >
          {{ t("admin.ddns.selectorInterfaceId") }}
        </Label>
        <Input
          :id="`${idPrefix}-interface-id`"
          :model-value="selector.ipv6InterfaceId || ''"
          placeholder="0000:0000:0000:1234"
          @update:model-value="
            (value: string | number) =>
              patchSelector({
                ipv6InterfaceId: String(value).trim() || undefined,
              })
          "
        />
        <p class="text-[11px] text-muted-foreground">
          {{ t("admin.ddns.selectorInterfaceIdHint") }}
        </p>
      </div>
    </template>

    <div
      v-if="family === 'ipv6'"
      class="flex items-center justify-between gap-3"
    >
      <div>
        <Label :for="`${idPrefix}-temporary`" class="text-sm">
          {{ t("admin.ddns.selectorAllowTemporary") }}
        </Label>
        <p class="text-[11px] text-muted-foreground">
          {{ t("admin.ddns.selectorAllowTemporaryHint") }}
        </p>
      </div>
      <Switch
        :id="`${idPrefix}-temporary`"
        :model-value="selector.allowTemporary"
        @update:model-value="
          (value: boolean) => patchSelector({ allowTemporary: value })
        "
      />
    </div>

    <div
      class="rounded-md border bg-muted/20 px-3 py-2 text-xs leading-5 text-muted-foreground"
    >
      <span v-if="previewing" role="status">{{
        t("admin.ddns.selectorPreviewing")
      }}</span>
      <span v-else-if="previewError" class="text-destructive" role="alert">
        {{ previewError }}
      </span>
      <template v-else-if="preview?.selectedAddress">
        <span>{{ t("admin.ddns.selectorPreviewSelected") }}：</span>
        <span class="break-all font-mono text-foreground">
          {{ preview.selectedAddress }}
        </span>
        <span v-if="preview.matchedAddresses.length > 1">
          ·
          {{
            t("admin.ddns.selectorPreviewMultiple", {
              count: preview.matchedAddresses.length,
            })
          }}
        </span>
      </template>
      <span v-else>{{ t("admin.ddns.selectorPreviewNoMatch") }}</span>
    </div>
  </div>
</template>
