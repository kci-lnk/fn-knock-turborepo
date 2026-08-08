<script setup lang="ts">
import { computed, useId } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
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
import { Switch } from "@/components/ui/switch";
import type { WOLRelay, WOLTarget, WOLTargetInput } from "@/lib/api";

const props = defineProps<{
  open: boolean;
  mode: "create" | "edit";
  model: WOLTargetInput;
  relays: WOLRelay[];
  saving: boolean;
  target: WOLTarget | null;
  error: string;
}>();

const emit = defineEmits<{
  confirm: [];
  "update:open": [value: boolean];
}>();

const { t } = useI18n();
const id = useId();
const localDeliveryValue = "__local__";
const deliveryValue = computed({
  get: () => props.model.relayId ?? localDeliveryValue,
  set: (value: string) => {
    props.model.relayId = value === localDeliveryValue ? null : value;
    if (props.model.relayId) props.model.broadcastAddress = null;
  },
});
const broadcastValue = computed({
  get: () => props.model.broadcastAddress ?? "",
  set: (value: string | number) => {
    const normalized = String(value).trim();
    props.model.broadcastAddress = normalized || null;
  },
});
const ipAddressValue = computed({
  get: () => props.model.ipAddress ?? "",
  set: (value: string | number) => {
    const normalized = String(value).trim();
    props.model.ipAddress = normalized || null;
  },
});

const integrations = computed(() => props.model.integrations);
type IntegrationProvider = "none" | "blinker" | "bemfa";
const integrationProvider = computed<IntegrationProvider>({
  get: () => {
    if (integrations.value?.blinker.enabled) return "blinker";
    if (integrations.value?.bemfa.enabled) return "bemfa";
    return "none";
  },
  set: (provider) => {
    if (!integrations.value) return;
    integrations.value.blinker.enabled = provider === "blinker";
    integrations.value.bemfa.enabled = provider === "bemfa";
    // Certificate verification is intentionally hidden for these consumer
    // cloud endpoints and defaults to the compatibility mode requested by the
    // product. Keep both drafts normalized when switching providers.
    integrations.value.blinker.skipTlsVerify = true;
    integrations.value.bemfa.skipTlsVerify = true;
  },
});
const bemfaTopicNeedsSuffix = computed(() => {
  const topic = integrations.value?.bemfa.topic.trim() ?? "";
  return Boolean(topic && !topic.endsWith("001") && !topic.endsWith("006"));
});
const selectedIntegrationRuntime = computed(() => {
  if (!props.target) return null;
  if (integrationProvider.value === "blinker")
    return props.target.integrations.blinker.runtime;
  if (integrationProvider.value === "bemfa")
    return props.target.integrations.bemfa.runtime;
  return null;
});
const blinkerError = computed(() =>
  /blinker/iu.test(props.error) ? props.error : "",
);
const bemfaError = computed(() =>
  /bemfa/iu.test(props.error) ? props.error : "",
);
const runtimeBadgeClass = (state: string) => {
  if (state === "connected")
    return "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300";
  if (state === "error" || state === "credential_missing")
    return "border-destructive/30 bg-destructive/10 text-destructive";
  return "border-border bg-muted text-muted-foreground";
};
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="max-h-[90vh] overflow-hidden"
      :class="mode === 'edit' ? 'sm:max-w-3xl' : 'sm:max-w-lg'"
    >
      <DialogHeader>
        <DialogTitle>
          {{
            mode === "create"
              ? t("admin.wol.targetDialog.createTitle")
              : t("admin.wol.targetDialog.editTitle")
          }}
        </DialogTitle>
        <DialogDescription>
          {{ t("admin.wol.targetDialog.description") }}
        </DialogDescription>
      </DialogHeader>
      <form
        class="min-h-0 space-y-4"
        autocomplete="off"
        @submit.prevent="emit('confirm')"
      >
        <div class="max-h-[calc(90vh-11rem)] space-y-4 overflow-y-auto px-1">
          <div class="space-y-2">
            <Label :for="`${id}-name`">{{ t("admin.wol.name") }}</Label>
            <Input
              :id="`${id}-name`"
              v-model="model.name"
              :placeholder="t('admin.wol.targetDialog.namePlaceholder')"
              maxlength="64"
            />
          </div>
          <div class="space-y-2">
            <Label :for="`${id}-mac`">{{ t("admin.wol.mac") }}</Label>
            <Input
              :id="`${id}-mac`"
              v-model="model.mac"
              autocomplete="off"
              autocapitalize="characters"
              spellcheck="false"
              placeholder="AA:BB:CC:DD:EE:FF"
            />
          </div>
          <div class="space-y-2">
            <Label :for="`${id}-ip`">{{ t("admin.wol.ipAddress") }}</Label>
            <Input
              :id="`${id}-ip`"
              v-model="ipAddressValue"
              inputmode="decimal"
              placeholder="192.168.31.20"
            />
            <p class="text-xs text-muted-foreground">
              {{ t("admin.wol.targetDialog.ipAddressHint") }}
            </p>
          </div>
          <div v-if="relays.length" class="space-y-2">
            <Label :for="`${id}-relay`">{{
              t("admin.wol.deliveryPath")
            }}</Label>
            <Select v-model="deliveryValue">
              <SelectTrigger :id="`${id}-relay`">
                <SelectValue
                  :placeholder="t('admin.wol.targetDialog.selectRelay')"
                />
              </SelectTrigger>
              <SelectContent>
                <SelectItem :value="localDeliveryValue">
                  {{ t("admin.wol.localDelivery") }}
                </SelectItem>
                <SelectItem
                  v-for="relay in relays"
                  :key="relay.id"
                  :value="relay.id"
                >
                  {{ relay.name }} · {{ relay.address
                  }}<template v-if="relay.port !== 40009"
                    >:{{ relay.port }}</template
                  >
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div v-if="!model.relayId" class="space-y-2">
            <Label :for="`${id}-broadcast`">{{
              t("admin.wol.broadcastAddress")
            }}</Label>
            <Input
              :id="`${id}-broadcast`"
              v-model="broadcastValue"
              inputmode="decimal"
              placeholder="192.168.31.255"
            />
            <p class="text-xs text-muted-foreground">
              {{ t("admin.wol.targetDialog.broadcastHint") }}
            </p>
          </div>
          <div
            class="flex items-center justify-between rounded-lg border px-3 py-3"
          >
            <div>
              <Label :for="`${id}-enabled`">{{ t("admin.wol.enabled") }}</Label>
              <p class="mt-0.5 text-xs text-muted-foreground">
                {{ t("admin.wol.targetDialog.enabledHint") }}
              </p>
            </div>
            <Switch :id="`${id}-enabled`" v-model="model.enabled" />
          </div>
          <section
            v-if="mode === 'edit' && integrations"
            class="space-y-4 border-t pt-4"
          >
            <div>
              <h3 class="text-sm font-semibold">
                {{ t("admin.wol.targetDialog.integrations.title") }}
              </h3>
              <p class="mt-1 text-xs leading-5 text-muted-foreground">
                {{ t("admin.wol.targetDialog.integrations.selectHint") }}
              </p>
            </div>

            <div class="space-y-2">
              <div class="flex items-center justify-between gap-3">
                <Label :for="`${id}-integration-provider`">
                  {{ t("admin.wol.targetDialog.integrations.provider") }}
                </Label>
                <Badge
                  v-if="selectedIntegrationRuntime"
                  variant="outline"
                  :class="runtimeBadgeClass(selectedIntegrationRuntime.state)"
                >
                  {{
                    t(
                      `admin.wol.targetDialog.integrations.runtime.${selectedIntegrationRuntime.state}`,
                    )
                  }}
                </Badge>
              </div>
              <Select v-model="integrationProvider">
                <SelectTrigger :id="`${id}-integration-provider`">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">
                    {{ t("admin.wol.targetDialog.integrations.none") }}
                  </SelectItem>
                  <SelectItem value="blinker">
                    {{ t("admin.wol.targetDialog.integrations.blinker.title") }}
                  </SelectItem>
                  <SelectItem value="bemfa">
                    {{ t("admin.wol.targetDialog.integrations.bemfa.title") }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div v-if="integrationProvider === 'blinker'" class="space-y-4">
              <div class="space-y-2">
                <Label :for="`${id}-blinker-key`">{{
                  t("admin.wol.targetDialog.integrations.blinker.deviceKey")
                }}</Label>
                <Input
                  :id="`${id}-blinker-key`"
                  v-model="integrations.blinker.deviceKey"
                  type="password"
                  autocomplete="new-password"
                  spellcheck="false"
                  maxlength="512"
                  :placeholder="
                    target?.integrations.blinker.credentialConfigured
                      ? t(
                          'admin.wol.targetDialog.integrations.credentialConfigured',
                        )
                      : t(
                          'admin.wol.targetDialog.integrations.blinker.deviceKeyPlaceholder',
                        )
                  "
                />
                <p v-if="blinkerError" class="text-xs text-destructive">
                  {{ blinkerError }}
                </p>
              </div>
              <div class="flex items-center justify-between gap-4">
                <div>
                  <Label :for="`${id}-blinker-component`">{{
                    t(
                      "admin.wol.targetDialog.integrations.blinker.bindComponent",
                    )
                  }}</Label>
                  <p class="mt-1 text-xs leading-5 text-muted-foreground">
                    {{
                      t(
                        "admin.wol.targetDialog.integrations.blinker.bindComponentHint",
                      )
                    }}
                  </p>
                </div>
                <Switch
                  :id="`${id}-blinker-component`"
                  v-model="integrations.blinker.bindComponent"
                />
              </div>
              <p
                v-if="target?.integrations.blinker.runtime.lastError"
                class="break-words rounded-md bg-destructive/5 px-3 py-2 text-xs text-destructive"
              >
                {{ target.integrations.blinker.runtime.lastError }}
              </p>
            </div>

            <div v-else-if="integrationProvider === 'bemfa'" class="space-y-4">
              <div class="grid gap-4 sm:grid-cols-2">
                <div class="space-y-2">
                  <Label :for="`${id}-bemfa-key`">{{
                    t("admin.wol.targetDialog.integrations.bemfa.privateKey")
                  }}</Label>
                  <Input
                    :id="`${id}-bemfa-key`"
                    v-model="integrations.bemfa.privateKey"
                    type="password"
                    autocomplete="new-password"
                    spellcheck="false"
                    maxlength="512"
                    :placeholder="
                      target?.integrations.bemfa.credentialConfigured
                        ? t(
                            'admin.wol.targetDialog.integrations.credentialConfigured',
                          )
                        : t(
                            'admin.wol.targetDialog.integrations.bemfa.privateKeyPlaceholder',
                          )
                    "
                  />
                  <p v-if="bemfaError" class="text-xs text-destructive">
                    {{ bemfaError }}
                  </p>
                </div>
                <div class="space-y-2">
                  <Label :for="`${id}-bemfa-topic`">{{
                    t("admin.wol.targetDialog.integrations.bemfa.topic")
                  }}</Label>
                  <Input
                    :id="`${id}-bemfa-topic`"
                    v-model="integrations.bemfa.topic"
                    maxlength="64"
                    autocomplete="off"
                    spellcheck="false"
                    placeholder="desktop001"
                  />
                  <p
                    class="text-xs leading-5"
                    :class="
                      bemfaTopicNeedsSuffix
                        ? 'text-amber-600 dark:text-amber-400'
                        : 'text-muted-foreground'
                    "
                  >
                    {{
                      t("admin.wol.targetDialog.integrations.bemfa.topicHint")
                    }}
                  </p>
                </div>
              </div>
              <p
                v-if="target?.integrations.bemfa.runtime.lastError"
                class="break-words rounded-md bg-destructive/5 px-3 py-2 text-xs text-destructive"
              >
                {{ target.integrations.bemfa.runtime.lastError }}
              </p>
            </div>
          </section>
        </div>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            @click="emit('update:open', false)"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button type="submit" :disabled="saving">
            {{ saving ? t("admin.wol.saving") : t("common.save") }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
