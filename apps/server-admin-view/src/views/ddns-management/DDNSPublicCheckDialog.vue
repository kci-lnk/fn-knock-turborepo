<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  CheckCircle2,
  Plus,
  RefreshCw,
  RotateCcw,
  Trash2,
  XCircle,
} from "lucide-vue-next";
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
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import type {
  DDNSHttpTransport,
  DDNSPublicDnsProvider,
  DDNSPublicCheckFamily,
  DDNSPublicCheckSourcesPayload,
  DDNSPublicCheckTestResultPayload,
} from "@/lib/api";
import {
  HTTP_TRANSPORT_OPTIONS,
  PUBLIC_DNS_PROVIDER_OPTIONS,
  normalizeDDNSHttpTransport,
  normalizeDDNSPublicDnsProvider,
} from "./model";

const props = defineProps<{
  draft: DDNSPublicCheckSourcesPayload;
  httpTransportDraft: DDNSHttpTransport;
  publicDnsProviderDraft: DDNSPublicDnsProvider;
  isSaving: boolean;
  isTesting: boolean;
  open: boolean;
  testResults: DDNSPublicCheckTestResultPayload[];
}>();

const emit = defineEmits<{
  "restore-defaults": [];
  save: [
    value: DDNSPublicCheckSourcesPayload,
    transport: DDNSHttpTransport,
    publicDnsProvider: DDNSPublicDnsProvider,
  ];
  test: [
    value: DDNSPublicCheckSourcesPayload,
    transport: DDNSHttpTransport,
    publicDnsProvider: DDNSPublicDnsProvider,
  ];
  "update:draft": [value: DDNSPublicCheckSourcesPayload];
  "update:httpTransportDraft": [value: DDNSHttpTransport];
  "update:publicDnsProviderDraft": [value: DDNSPublicDnsProvider];
  "update:open": [value: boolean];
}>();

const { t } = useI18n();

const families: Array<{
  key: DDNSPublicCheckFamily;
  labelKey: string;
}> = [
  { key: "ipv4", labelKey: "admin.ddns.publicCheckIpv4Title" },
  { key: "ipv6", labelKey: "admin.ddns.publicCheckIpv6Title" },
];

const cloneSources = (
  value: DDNSPublicCheckSourcesPayload,
): DDNSPublicCheckSourcesPayload => ({
  ipv4: [...value.ipv4],
  ipv6: [...value.ipv6],
});

const updateSource = (
  family: DDNSPublicCheckFamily,
  index: number,
  value: string,
) => {
  const next = cloneSources(props.draft);
  next[family][index] = value;
  emit("update:draft", next);
};

const addSource = (family: DDNSPublicCheckFamily) => {
  const next = cloneSources(props.draft);
  next[family].push("");
  emit("update:draft", next);
};

const removeSource = (family: DDNSPublicCheckFamily, index: number) => {
  const next = cloneSources(props.draft);
  next[family].splice(index, 1);
  emit("update:draft", next);
};

const groupedResults = computed(() => ({
  ipv4: props.testResults.filter((item) => item.family === "ipv4"),
  ipv6: props.testResults.filter((item) => item.family === "ipv6"),
}));

const hasResults = computed(() => props.testResults.length > 0);
const isBusy = computed(() => props.isSaving || props.isTesting);
const transportDraft = computed({
  get: () => props.httpTransportDraft,
  set: (value: string) =>
    emit("update:httpTransportDraft", normalizeDDNSHttpTransport(value)),
});
const dnsProviderDraft = computed({
  get: () => props.publicDnsProviderDraft,
  set: (value: string) =>
    emit(
      "update:publicDnsProviderDraft",
      normalizeDDNSPublicDnsProvider(value),
    ),
});
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-h-[85vh] overflow-y-auto sm:max-w-[640px]">
      <DialogHeader>
        <DialogTitle>{{ t("admin.ddns.publicCheckDialogTitle") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.ddns.publicCheckDialogDescription") }}
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-5 py-2">
        <section class="space-y-2">
          <Label for="ddns-http-transport" class="text-sm font-medium">
            {{ t("admin.ddns.httpTransportLabel") }}
          </Label>
          <Select v-model="transportDraft" :disabled="isBusy">
            <SelectTrigger id="ddns-http-transport" class="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="option in HTTP_TRANSPORT_OPTIONS"
                :key="option.value"
                :value="option.value"
              >
                {{ t(option.labelKey) }}
              </SelectItem>
            </SelectContent>
          </Select>
          <p class="text-xs leading-5 text-muted-foreground">
            {{ t("admin.ddns.httpTransportHint") }}
          </p>
        </section>

        <section class="space-y-2">
          <Label for="ddns-public-dns-provider" class="text-sm font-medium">
            {{ t("admin.ddns.publicDnsProviderLabel") }}
          </Label>
          <Select v-model="dnsProviderDraft" :disabled="isBusy">
            <SelectTrigger id="ddns-public-dns-provider" class="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="option in PUBLIC_DNS_PROVIDER_OPTIONS"
                :key="option.value"
                :value="option.value"
              >
                {{ t(option.labelKey) }}
              </SelectItem>
            </SelectContent>
          </Select>
          <p class="text-xs leading-5 text-muted-foreground">
            {{ t("admin.ddns.publicDnsProviderHint") }}
          </p>
        </section>

        <section v-for="family in families" :key="family.key" class="space-y-3">
          <div class="flex items-center justify-between gap-3">
            <Label class="text-sm font-medium">
              {{ t(family.labelKey) }}
            </Label>
            <Button
              type="button"
              variant="outline"
              size="sm"
              :disabled="isBusy"
              @click="addSource(family.key)"
            >
              <Plus class="h-4 w-4" />
              {{ t("admin.ddns.addPublicCheckSource") }}
            </Button>
          </div>

          <div v-if="draft[family.key].length > 0" class="space-y-2">
            <div
              v-for="(source, index) in draft[family.key]"
              :key="`${family.key}-${index}`"
              class="flex min-w-0 items-center gap-2"
            >
              <Input
                :id="`ddns-public-check-${family.key}-${index}`"
                :model-value="source"
                :disabled="isBusy"
                :placeholder="t('admin.ddns.publicCheckSourcePlaceholder')"
                inputmode="url"
                @update:model-value="
                  updateSource(family.key, index, String($event))
                "
                @keydown.enter.prevent="
                  emit(
                    'save',
                    cloneSources(draft),
                    transportDraft,
                    dnsProviderDraft,
                  )
                "
              />
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                class="text-muted-foreground hover:text-destructive"
                :disabled="isBusy"
                :aria-label="t('admin.ddns.removePublicCheckSource')"
                :title="t('admin.ddns.removePublicCheckSource')"
                @click="removeSource(family.key, index)"
              >
                <Trash2 class="h-4 w-4" />
              </Button>
            </div>
          </div>

          <p v-else class="text-sm text-muted-foreground">
            {{
              t("admin.ddns.publicCheckNoSources", {
                family: family.key === "ipv4" ? "IPv4" : "IPv6",
              })
            }}
          </p>
        </section>

        <div v-if="hasResults" class="space-y-3 border-t pt-4">
          <h3 class="text-sm font-medium">
            {{ t("admin.ddns.publicCheckTestResultTitle") }}
          </h3>

          <div
            v-for="family in families"
            :key="`results-${family.key}`"
            class="space-y-2"
          >
            <p
              v-if="groupedResults[family.key].length > 0"
              class="text-xs font-medium uppercase text-muted-foreground"
            >
              {{ t(family.labelKey) }}
            </p>
            <div
              v-for="result in groupedResults[family.key]"
              :key="`${result.family}-${result.url}`"
              class="rounded-md border px-3 py-2 text-sm"
            >
              <div class="flex min-w-0 items-start gap-2">
                <CheckCircle2
                  v-if="result.success"
                  class="mt-0.5 h-4 w-4 shrink-0 text-emerald-600"
                />
                <XCircle
                  v-else
                  class="mt-0.5 h-4 w-4 shrink-0 text-destructive"
                />
                <div class="min-w-0 flex-1 space-y-1">
                  <p class="break-all font-medium">{{ result.url }}</p>
                  <p
                    :class="
                      result.success ? 'text-emerald-700' : 'text-destructive'
                    "
                  >
                    {{
                      result.success && result.ip
                        ? t("admin.ddns.publicCheckTestSuccess", {
                            ip: result.ip,
                          })
                        : result.error || t("admin.ddns.publicCheckTestFailed")
                    }}
                  </p>
                  <p
                    v-if="typeof result.status === 'number'"
                    class="text-xs text-muted-foreground"
                  >
                    {{
                      t("admin.ddns.publicCheckStatusCode", {
                        status: result.status,
                      })
                    }}
                  </p>
                  <p
                    v-if="result.responsePreview"
                    class="break-all text-xs text-muted-foreground"
                  >
                    {{
                      t("admin.ddns.publicCheckResponsePreview", {
                        preview: result.responsePreview,
                      })
                    }}
                  </p>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div
        class="flex flex-col gap-2 border-t pt-4 sm:flex-row sm:items-center sm:justify-between"
      >
        <ConfirmDangerPopover
          :title="t('admin.ddns.restorePublicCheckDefaultsTitle')"
          :description="t('admin.ddns.restorePublicCheckDefaultsDescription')"
          :confirm-text="t('admin.ddns.restorePublicCheckDefaultsConfirm')"
          confirm-variant="default"
          :disabled="isBusy"
          :on-confirm="() => emit('restore-defaults')"
        >
          <template #trigger>
            <Button
              type="button"
              variant="ghost"
              class="justify-center text-muted-foreground hover:text-foreground"
              :disabled="isBusy"
            >
              <RotateCcw class="h-4 w-4" />
              {{ t("admin.ddns.restorePublicCheckDefaults") }}
            </Button>
          </template>
        </ConfirmDangerPopover>

        <DialogFooter class="gap-2">
          <Button
            type="button"
            variant="outline"
            :disabled="isBusy"
            @click="emit('update:open', false)"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            type="button"
            variant="outline"
            :disabled="isBusy"
            @click="
              emit(
                'test',
                cloneSources(draft),
                transportDraft,
                dnsProviderDraft,
              )
            "
          >
            <RefreshCw
              class="mr-1.5 h-4 w-4"
              :class="{ 'animate-spin': isTesting }"
            />
            {{ t("admin.ddns.testPublicCheckSources") }}
          </Button>
          <Button
            type="button"
            :disabled="isBusy"
            @click="
              emit(
                'save',
                cloneSources(draft),
                transportDraft,
                dnsProviderDraft,
              )
            "
          >
            <RefreshCw v-if="isSaving" class="mr-1.5 h-4 w-4 animate-spin" />
            {{ isSaving ? t("admin.ddns.saving") : t("common.save") }}
          </Button>
        </DialogFooter>
      </div>
    </DialogContent>
  </Dialog>
</template>
