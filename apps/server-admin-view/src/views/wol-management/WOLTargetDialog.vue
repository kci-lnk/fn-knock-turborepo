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
import { Textarea } from "@/components/ui/textarea";
import { Loader2, PlugZap, Terminal, Trash2 } from "lucide-vue-next";
import {
  type WOLRelay,
  type WOLTarget,
  type WOLTargetInput,
} from "@/lib/api/wol";
import { changeWolSshAuthMethod } from "./wol-management-model";

const props = defineProps<{
  open: boolean;
  mode: "create" | "edit";
  model: WOLTargetInput;
  relays: WOLRelay[];
  saving: boolean;
  target: WOLTarget | null;
  error: string;
  testingSsh: boolean;
}>();

const emit = defineEmits<{
  confirm: [];
  "test-ssh": [];
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
const ssh = computed(() => props.model.ssh);
const clearTrustedHostKey = () => {
  if (!ssh.value) return;
  ssh.value.hostKeyAlgorithm = "";
  ssh.value.hostKeyFingerprint = "";
};
const sshHostValue = computed({
  get: () => ssh.value?.host ?? "",
  set: (value: string | number) => {
    if (!ssh.value) return;
    const normalized = String(value).trim();
    if (ssh.value.host !== normalized) clearTrustedHostKey();
    ssh.value.host = normalized;
  },
});
const sshPortValue = computed({
  get: () => ssh.value?.port ?? 22,
  set: (value: string | number) => {
    if (!ssh.value) return;
    const normalized = Math.max(1, Math.min(65535, Number(value) || 22));
    if (ssh.value.port !== normalized) clearTrustedHostKey();
    ssh.value.port = normalized;
  },
});
const sshUsernameValue = computed({
  get: () => ssh.value?.username ?? "",
  set: (value: string | number) => {
    if (!ssh.value) return;
    const normalized = String(value).trim();
    if (ssh.value.username !== normalized) clearTrustedHostKey();
    ssh.value.username = normalized;
  },
});
const sshPasswordValue = computed({
  get: () => ssh.value?.password ?? "",
  set: (value: string | number) => {
    if (!ssh.value) return;
    ssh.value.password = String(value);
    ssh.value.clearCredential = false;
    clearTrustedHostKey();
  },
});
const sshPrivateKeyValue = computed({
  get: () => ssh.value?.privateKey ?? "",
  set: (value: string | number) => {
    if (!ssh.value) return;
    ssh.value.privateKey = String(value);
    ssh.value.clearCredential = false;
    clearTrustedHostKey();
  },
});
const sshPassphraseValue = computed({
  get: () => ssh.value?.privateKeyPassphrase ?? "",
  set: (value: string | number) => {
    if (!ssh.value) return;
    ssh.value.privateKeyPassphrase = String(value);
    ssh.value.clearCredential = false;
    clearTrustedHostKey();
  },
});
const sshAuthMethodValue = computed({
  get: () => ssh.value?.authMethod ?? "privateKey",
  set: (value: "password" | "privateKey") => {
    if (ssh.value) changeWolSshAuthMethod(ssh.value, value);
  },
});
const sshPlatformValue = computed({
  get: () => ssh.value?.platform ?? "linux",
  set: (value: "linux" | "macos" | "windows") => {
    if (!ssh.value || ssh.value.platform === value) return;
    ssh.value.platform = value;
    clearTrustedHostKey();
  },
});
const sshPlatformLabel = computed(() => {
  if (sshPlatformValue.value === "macos") return "macOS";
  if (sshPlatformValue.value === "windows") return "Windows";
  return "Linux";
});
const privateKeyCopyCommand = computed(() => {
  if (sshPlatformValue.value === "macos") return "pbcopy < ~/.ssh/id_ed25519";
  if (sshPlatformValue.value === "windows")
    return "Get-Content $env:USERPROFILE\\.ssh\\id_ed25519 -Raw | Set-Clipboard";
  return "cat ~/.ssh/id_ed25519";
});
const clearSshCredential = () => {
  if (!ssh.value) return;
  ssh.value.password = "";
  ssh.value.privateKey = "";
  ssh.value.privateKeyPassphrase = "";
  ssh.value.clearCredential = true;
  ssh.value.enabled = false;
};
const sshCredentialReady = computed(() => {
  if (!ssh.value) return false;
  const savedCredentialMatches =
    props.target?.ssh.authMethod === ssh.value.authMethod &&
    props.target.ssh.credentialConfigured;
  if (ssh.value.authMethod === "password")
    return Boolean(ssh.value.password || savedCredentialMatches);
  return Boolean(ssh.value.privateKey || savedCredentialMatches);
});
const canTestSsh = computed(
  () =>
    Boolean(
      ssh.value?.enabled &&
      ssh.value.host &&
      ssh.value.username &&
      !ssh.value.clearCredential &&
      sshCredentialReady.value,
    ) && !props.testingSsh,
);
const sshConfigurationReady = computed(
  () =>
    !ssh.value?.enabled ||
    Boolean(
      ssh.value.hostKeyAlgorithm &&
      ssh.value.hostKeyFingerprint &&
      sshCredentialReady.value,
    ),
);

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
      class="max-h-[calc(100dvh-1rem)] w-[calc(100%-1rem)] overflow-hidden sm:max-h-[90vh] sm:w-full"
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
        <div
          class="max-h-[calc(100dvh-12rem)] space-y-4 overflow-y-auto px-1 sm:max-h-[calc(90vh-11rem)]"
        >
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
          <section
            v-if="mode === 'edit' && ssh"
            class="space-y-4 border-t pt-4"
          >
            <div class="flex items-start justify-between gap-4">
              <div>
                <h3 class="text-sm font-semibold">
                  {{ t("admin.wol.ssh.title") }}
                </h3>
                <p class="mt-1 text-xs leading-5 text-muted-foreground">
                  {{ t("admin.wol.ssh.description") }}
                </p>
              </div>
              <Switch :id="`${id}-ssh-enabled`" v-model="ssh.enabled" />
            </div>

            <div v-if="ssh.enabled" class="space-y-4">
              <div class="grid gap-4 sm:grid-cols-[1fr_8rem]">
                <div class="space-y-2">
                  <Label :for="`${id}-ssh-host`">{{
                    t("admin.wol.ssh.host")
                  }}</Label>
                  <Input
                    :id="`${id}-ssh-host`"
                    v-model="sshHostValue"
                    autocomplete="off"
                    spellcheck="false"
                    placeholder="192.168.31.20"
                  />
                </div>
                <div class="space-y-2">
                  <Label :for="`${id}-ssh-port`">{{
                    t("admin.wol.ssh.port")
                  }}</Label>
                  <Input
                    :id="`${id}-ssh-port`"
                    v-model="sshPortValue"
                    type="number"
                    min="1"
                    max="65535"
                  />
                </div>
              </div>

              <div class="grid gap-4 sm:grid-cols-3">
                <div class="space-y-2 sm:col-span-1">
                  <Label :for="`${id}-ssh-user`">{{
                    t("admin.wol.ssh.username")
                  }}</Label>
                  <Input
                    :id="`${id}-ssh-user`"
                    v-model="sshUsernameValue"
                    autocomplete="off"
                    spellcheck="false"
                  />
                </div>
                <div class="space-y-2">
                  <Label :for="`${id}-ssh-platform`">{{
                    t("admin.wol.ssh.platform")
                  }}</Label>
                  <Select v-model="sshPlatformValue">
                    <SelectTrigger :id="`${id}-ssh-platform`">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="linux">Linux</SelectItem>
                      <SelectItem value="macos">macOS</SelectItem>
                      <SelectItem value="windows">Windows</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div class="space-y-2">
                  <Label :for="`${id}-ssh-auth`">{{
                    t("admin.wol.ssh.authMethod")
                  }}</Label>
                  <Select v-model="sshAuthMethodValue">
                    <SelectTrigger :id="`${id}-ssh-auth`">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="privateKey">
                        {{ t("admin.wol.ssh.privateKeyAuth") }}
                      </SelectItem>
                      <SelectItem value="password">
                        {{ t("admin.wol.ssh.passwordAuth") }}
                      </SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>

              <div v-if="ssh.authMethod === 'password'" class="space-y-2">
                <Label :for="`${id}-ssh-password`">{{
                  t("admin.wol.ssh.password")
                }}</Label>
                <Input
                  :id="`${id}-ssh-password`"
                  v-model="sshPasswordValue"
                  type="password"
                  autocomplete="new-password"
                  :placeholder="
                    target?.ssh.credentialConfigured
                      ? t('admin.wol.ssh.credentialConfigured')
                      : ''
                  "
                />
              </div>
              <template v-else>
                <div class="space-y-2">
                  <Label :for="`${id}-ssh-private-key`">{{
                    t("admin.wol.ssh.privateKey")
                  }}</Label>
                  <Textarea
                    :id="`${id}-ssh-private-key`"
                    v-model="sshPrivateKeyValue"
                    class="min-h-28 font-mono text-xs"
                    autocomplete="off"
                    spellcheck="false"
                    :placeholder="
                      target?.ssh.credentialConfigured
                        ? t('admin.wol.ssh.credentialConfigured')
                        : '-----BEGIN OPENSSH PRIVATE KEY-----'
                    "
                  />
                  <div
                    class="rounded-lg border border-sky-500/30 bg-sky-500/5 p-3"
                  >
                    <div class="flex items-start gap-2">
                      <Terminal
                        class="mt-0.5 h-4 w-4 shrink-0 text-sky-600 dark:text-sky-400"
                      />
                      <div class="min-w-0 space-y-1.5">
                        <p class="text-xs font-medium">
                          {{
                            t("admin.wol.ssh.privateKeyCopyTitle", {
                              platform: sshPlatformLabel,
                            })
                          }}
                        </p>
                        <p class="text-xs leading-5 text-muted-foreground">
                          {{ t("admin.wol.ssh.privateKeyCopyHint") }}
                        </p>
                        <code
                          class="block overflow-x-auto rounded bg-background px-2.5 py-2 font-mono text-xs"
                          >{{ privateKeyCopyCommand }}</code
                        >
                      </div>
                    </div>
                  </div>
                </div>
                <div class="space-y-2">
                  <Label :for="`${id}-ssh-passphrase`">{{
                    t("admin.wol.ssh.privateKeyPassphrase")
                  }}</Label>
                  <Input
                    :id="`${id}-ssh-passphrase`"
                    v-model="sshPassphraseValue"
                    type="password"
                    autocomplete="new-password"
                    :placeholder="
                      target?.ssh.passphraseConfigured
                        ? t('admin.wol.ssh.passphraseConfigured')
                        : t('admin.wol.ssh.optional')
                    "
                  />
                </div>
              </template>

              <div class="flex flex-wrap items-center justify-between gap-2">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  :disabled="!target?.ssh.credentialConfigured || testingSsh"
                  @click="clearSshCredential"
                >
                  <Trash2 class="mr-1.5 h-3.5 w-3.5 text-destructive" />
                  {{ t("admin.wol.ssh.clearCredential") }}
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  :disabled="!canTestSsh"
                  @click="emit('test-ssh')"
                >
                  <Loader2
                    v-if="testingSsh"
                    class="mr-1.5 h-3.5 w-3.5 animate-spin"
                  />
                  <PlugZap v-else class="mr-1.5 h-3.5 w-3.5" />
                  {{ t("admin.wol.ssh.testConnection") }}
                </Button>
              </div>
              <p
                v-if="!ssh.hostKeyFingerprint"
                class="text-xs leading-5 text-amber-600 dark:text-amber-400"
              >
                {{ t("admin.wol.ssh.testRequired") }}
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
          <Button
            type="submit"
            :disabled="saving || testingSsh || !sshConfigurationReady"
          >
            {{ saving ? t("admin.wol.saving") : t("common.save") }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
