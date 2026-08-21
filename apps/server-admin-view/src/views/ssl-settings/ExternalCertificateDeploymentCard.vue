<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Copy,
  Globe2,
  KeyRound,
  Monitor,
  Network,
  Pencil,
  Plus,
  Settings2,
  ShieldAlert,
  Trash2,
  X,
} from "lucide-vue-next";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import type { ExternalCertificateBinding } from "@/types";
import ExternalCertificateLanEditor from "./ExternalCertificateLanEditor.vue";
import { useExternalCertificateBindings } from "./useExternalCertificateBindings";

const { t } = useI18n();
const activeTab = ref("bindings");
const lanEditorOpen = ref(false);
const editingBindingId = ref<string | null>(null);
const {
  bindingName,
  bindingNameDrafts,
  bindings,
  clearCredential,
  configured,
  copyCompleteConfiguration,
  copyValue,
  createBinding,
  credential,
  credentialFields,
  deployUrlOptions,
  selectedDeployUrl,
  formatDate,
  isCreating,
  isLoading,
  isSavingLan,
  lanSettings,
  lanAddressDraft,
  addDetectedLanAddress,
  saveLanSettings,
  loadBindings,
  pendingBindingId,
  provider,
  providerName,
  providerOptions,
  publicDeployStatusDescription,
  renameBinding,
  revokeBinding,
  rotateToken,
  setBindingEnabled,
  summary,
} = useExternalCertificateBindings();
const primaryBinding = computed(() => bindings.value[0] ?? null);

function collapseAndClear(collapse: () => void) {
  clearCredential();
  editingBindingId.value = null;
  lanEditorOpen.value = false;
  activeTab.value = "bindings";
  collapse();
}

function showBindingSetup() {
  lanEditorOpen.value = false;
  activeTab.value = "bindings";
}

function beginRename(binding: ExternalCertificateBinding) {
  bindingNameDrafts.value[binding.id] = binding.name;
  editingBindingId.value = binding.id;
}

async function saveRename(binding: ExternalCertificateBinding) {
  if (await renameBinding(binding)) editingBindingId.value = null;
}

async function saveLan(enabled: boolean) {
  if (await saveLanSettings(enabled)) lanEditorOpen.value = false;
}

function endpointTemplate(url: string, binding: ExternalCertificateBinding) {
  const suffix = `/${binding.id}`;
  return url.endsWith(suffix)
    ? `${url.slice(0, -suffix.length)}/{binding_id}`
    : url;
}

function lanEndpointTemplate(address: string) {
  return `https://${address}:${lanSettings.value?.gateway_port ?? 7999}/__certificates__/{binding_id}`;
}

onMounted(() => void loadBindings());
</script>

<template>
  <ConfigCollapsibleCard
    :title="t('admin.certConfig.externalAutomationTitle')"
    :configured="configured"
    :ready="!isLoading"
    :edit-label="t('admin.certConfig.externalManage')"
    collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
    summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
    expanded-content-class="p-0 sm:p-0"
    actions-class="border-t bg-muted/20 px-4 py-4 sm:px-6 flex justify-end rounded-b-lg"
    card-class="dynamic-white-cert-card"
  >
    <template #summary>{{ summary }}</template>

    <template #default>
      <div class="grid min-w-0 gap-4 p-4 sm:p-5">
        <p class="max-w-4xl text-xs leading-5 text-muted-foreground">
          {{ t("admin.certConfig.externalDescription") }}
        </p>

        <Tabs v-model="activeTab" class="grid min-w-0 gap-4">
          <TabsList class="h-8 w-fit max-w-full p-0.5">
            <TabsTrigger value="bindings" class="h-7 gap-1.5 px-2.5 text-xs">
              <KeyRound class="size-3.5" />
              {{ t("admin.certConfig.externalBindingsTab") }}
              <span class="tabular-nums text-muted-foreground">
                ({{ bindings.length }})
              </span>
            </TabsTrigger>
            <TabsTrigger value="endpoints" class="h-7 gap-1.5 px-2.5 text-xs">
              <Network class="size-3.5" />
              {{ t("admin.certConfig.externalEndpointsTab") }}
            </TabsTrigger>
          </TabsList>

          <TabsContent value="bindings" class="mt-0 grid min-w-0 gap-4">
            <div
              class="flex items-start gap-2 rounded-md bg-amber-50 px-3 py-2.5 text-xs leading-5 text-amber-950 dark:bg-amber-950/20 dark:text-amber-100"
            >
              <ShieldAlert class="mt-0.5 size-4 shrink-0" />
              <p>
                <span class="font-medium">
                  {{ t("admin.certConfig.externalSecurityTitle") }}：
                </span>
                {{ t("admin.certConfig.externalSecurityDescription") }}
              </p>
            </div>

            <section class="grid gap-3" aria-labelledby="external-create-title">
              <div class="grid gap-0.5">
                <h3 id="external-create-title" class="text-sm font-semibold">
                  {{ t("admin.certConfig.externalCreateTitle") }}
                </h3>
                <p class="text-xs text-muted-foreground">
                  {{ t("admin.certConfig.externalCreateDescription") }}
                </p>
              </div>
              <div
                class="grid min-w-0 gap-3 rounded-lg border p-3 sm:grid-cols-[160px_minmax(0,1fr)_auto] sm:items-end"
              >
                <div class="grid gap-1.5">
                  <Label for="external-certificate-provider">
                    {{ t("admin.certConfig.externalProviderLabel") }}
                  </Label>
                  <Select v-model="provider">
                    <SelectTrigger id="external-certificate-provider">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem
                        v-for="option in providerOptions"
                        :key="option.value"
                        :value="option.value"
                      >
                        {{ option.label }}
                      </SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div class="grid gap-1.5">
                  <Label for="external-certificate-binding-name">
                    {{ t("admin.certConfig.externalNameLabel") }}
                  </Label>
                  <Input
                    id="external-certificate-binding-name"
                    v-model="bindingName"
                    :placeholder="t('admin.certConfig.externalNamePlaceholder')"
                    maxlength="80"
                    @keyup.enter="createBinding"
                  />
                </div>
                <Button
                  class="gap-2"
                  :disabled="isCreating || !bindingName.trim()"
                  @click="createBinding"
                >
                  <Plus class="size-4" />
                  {{
                    isCreating
                      ? t("admin.certConfig.externalCreating")
                      : t("admin.certConfig.externalCreate")
                  }}
                </Button>
              </div>
            </section>

            <section
              v-if="credential"
              class="grid min-w-0 gap-4 rounded-lg border p-4"
              aria-labelledby="external-credential-title"
            >
              <div class="flex flex-wrap items-start justify-between gap-3">
                <div class="grid gap-1">
                  <div class="flex flex-wrap items-center gap-2">
                    <h3 id="external-credential-title" class="font-semibold">
                      {{
                        t("admin.certConfig.externalCredentialTitle", {
                          provider: providerName(credential.binding.provider),
                        })
                      }}
                    </h3>
                    <Badge variant="outline">
                      {{ t("admin.certConfig.externalCredentialOneTime") }}
                    </Badge>
                  </div>
                  <p class="max-w-3xl text-sm leading-6 text-muted-foreground">
                    {{ t("admin.certConfig.externalCredentialDescription") }}
                  </p>
                </div>
                <Button class="gap-2" @click="copyCompleteConfiguration">
                  <Copy class="size-4" />
                  {{ t("admin.certConfig.externalCopyAll") }}
                </Button>
              </div>

              <div class="grid gap-1.5">
                <Label for="external-certificate-deploy-endpoint">
                  {{ t("admin.certConfig.externalEndpointLabel") }}
                </Label>
                <Select v-model="selectedDeployUrl">
                  <SelectTrigger
                    id="external-certificate-deploy-endpoint"
                    class="bg-background"
                  >
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem
                      v-for="option in deployUrlOptions"
                      :key="option.value"
                      :value="option.value"
                    >
                      {{ option.label }}
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div class="grid gap-3">
                <div
                  v-for="item in credentialFields"
                  :key="item.label"
                  class="grid gap-1.5"
                >
                  <div class="text-xs font-medium text-muted-foreground">
                    {{ item.label }}
                  </div>
                  <div
                    class="grid min-w-0 gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-start"
                  >
                    <code
                      class="block min-w-0 max-w-full rounded-md bg-muted px-3 py-2.5 text-xs [overflow-wrap:anywhere]"
                      :class="
                        item.multiline
                          ? 'max-h-80 overflow-auto whitespace-pre-wrap break-words'
                          : 'break-all'
                      "
                      >{{ item.value }}</code
                    >
                    <Button
                      size="sm"
                      variant="outline"
                      class="shrink-0 gap-1.5"
                      @click="copyValue(item.value)"
                    >
                      <Copy class="size-3.5" />
                      {{ t("admin.certConfig.externalCopy") }}
                    </Button>
                  </div>
                </div>
              </div>
            </section>

            <section
              class="grid gap-3"
              aria-labelledby="external-bindings-title"
            >
              <div class="flex items-center justify-between gap-3">
                <div>
                  <h3 id="external-bindings-title" class="font-semibold">
                    {{ t("admin.certConfig.externalBindingsTitle") }}
                  </h3>
                  <p class="text-sm text-muted-foreground">
                    {{ t("admin.certConfig.externalBindingsDescription") }}
                  </p>
                </div>
                <Badge variant="outline" class="tabular-nums">
                  {{ bindings.length }}
                </Badge>
              </div>

              <div
                v-if="!bindings.length"
                class="rounded-lg border border-dashed p-6 text-center"
              >
                <KeyRound
                  class="mx-auto mb-3 size-8 text-muted-foreground/60"
                />
                <div class="text-sm font-medium">
                  {{ t("admin.certConfig.externalNoBindings") }}
                </div>
                <p class="mt-1 text-xs text-muted-foreground">
                  {{ t("admin.certConfig.externalNoBindingsHint") }}
                </p>
              </div>

              <article
                v-for="binding in bindings"
                :key="binding.id"
                class="grid min-w-0 gap-3 rounded-lg border p-4"
              >
                <div class="flex flex-wrap items-start justify-between gap-3">
                  <div class="grid min-w-0 gap-2">
                    <div class="flex flex-wrap items-center gap-2">
                      <span class="font-semibold">{{ binding.name }}</span>
                      <Badge variant="outline">
                        {{ providerName(binding.provider) }}
                      </Badge>
                      <Badge
                        :variant="binding.enabled ? 'default' : 'secondary'"
                      >
                        {{
                          binding.enabled
                            ? t("admin.certConfig.externalEnabled")
                            : t("admin.certConfig.externalDisabled")
                        }}
                      </Badge>
                      <Badge
                        :variant="
                          binding.last_result === 'failed'
                            ? 'destructive'
                            : 'outline'
                        "
                      >
                        {{
                          binding.last_result === "success"
                            ? t("admin.certConfig.externalLastSuccess")
                            : binding.last_result === "failed"
                              ? t("admin.certConfig.externalLastFailed")
                              : binding.last_result === "superseded"
                                ? t("admin.certConfig.externalSuperseded")
                                : t("admin.certConfig.externalNeverDeployed")
                        }}
                      </Badge>
                    </div>
                    <div
                      v-if="editingBindingId === binding.id"
                      class="flex max-w-xl flex-col gap-2 sm:flex-row"
                    >
                      <Input
                        v-model="bindingNameDrafts[binding.id]"
                        :aria-label="t('admin.certConfig.externalRename')"
                        maxlength="80"
                        @keyup.enter="saveRename(binding)"
                        @keyup.esc="editingBindingId = null"
                      />
                      <Button
                        size="sm"
                        :disabled="
                          pendingBindingId === binding.id ||
                          !bindingNameDrafts[binding.id]?.trim() ||
                          bindingNameDrafts[binding.id]?.trim() === binding.name
                        "
                        @click="saveRename(binding)"
                      >
                        {{ t("admin.certConfig.externalRename") }}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        class="gap-1.5"
                        @click="editingBindingId = null"
                      >
                        <X class="size-3.5" />
                        {{ t("common.cancel") }}
                      </Button>
                    </div>
                  </div>
                  <div class="flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      class="gap-1.5"
                      @click="beginRename(binding)"
                    >
                      <Pencil class="size-3.5" />
                      {{ t("admin.certConfig.externalRename") }}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      :disabled="pendingBindingId === binding.id"
                      @click="setBindingEnabled(binding, !binding.enabled)"
                    >
                      {{
                        binding.enabled
                          ? t("admin.certConfig.externalDisable")
                          : t("admin.certConfig.externalEnable")
                      }}
                    </Button>
                    <ConfirmDangerPopover
                      :title="t('admin.certConfig.externalRotateTitle')"
                      :description="
                        t('admin.certConfig.externalRotateDescription')
                      "
                      :confirm-text="t('admin.certConfig.externalRotate')"
                      :loading="pendingBindingId === binding.id"
                      :on-confirm="() => rotateToken(binding)"
                    >
                      <template #trigger>
                        <Button size="sm" variant="outline">
                          {{ t("admin.certConfig.externalRotate") }}
                        </Button>
                      </template>
                    </ConfirmDangerPopover>
                    <ConfirmDangerPopover
                      :title="t('admin.certConfig.externalRevokeTitle')"
                      :description="
                        t('admin.certConfig.externalRevokeDescription')
                      "
                      :confirm-text="t('admin.certConfig.externalRevoke')"
                      :loading="pendingBindingId === binding.id"
                      :on-confirm="() => revokeBinding(binding)"
                    >
                      <template #trigger>
                        <Button
                          size="sm"
                          variant="ghost"
                          class="gap-1.5 text-destructive"
                        >
                          <Trash2 class="size-3.5" />
                          {{ t("admin.certConfig.externalRevoke") }}
                        </Button>
                      </template>
                    </ConfirmDangerPopover>
                  </div>
                </div>

                <dl
                  class="flex min-w-0 flex-wrap gap-x-6 gap-y-2 border-t pt-3 text-xs"
                >
                  <div class="flex gap-1.5">
                    <dt class="text-muted-foreground">
                      {{ t("admin.certConfig.externalLastDeployment") }}:
                    </dt>
                    <dd class="font-medium">
                      {{ formatDate(binding.last_deployed_at) }}
                    </dd>
                  </div>
                  <div class="flex gap-1.5">
                    <dt class="text-muted-foreground">
                      {{ t("admin.certConfig.externalExpiresAt") }}:
                    </dt>
                    <dd class="font-medium">
                      {{ formatDate(binding.last_valid_to) }}
                    </dd>
                  </div>
                  <div class="flex min-w-0 gap-1.5">
                    <dt class="shrink-0 text-muted-foreground">
                      {{ t("admin.certConfig.externalDomains") }}:
                    </dt>
                    <dd class="min-w-0 break-all font-medium">
                      {{ binding.last_dns_names.join(", ") || "—" }}
                    </dd>
                  </div>
                </dl>
                <Alert v-if="binding.last_error" variant="destructive">
                  <AlertDescription class="break-words">
                    {{ binding.last_error }}
                  </AlertDescription>
                </Alert>
                <div
                  v-if="binding.last_replaced_certificate_count > 0"
                  class="rounded-lg border bg-background/70 p-3 text-xs leading-5 text-muted-foreground"
                >
                  {{
                    t("admin.certConfig.externalTakeoverSummary", {
                      certificates: binding.last_replaced_certificate_count,
                      sources: binding.last_replaced_sources.join(", "),
                      bindings: binding.last_disabled_external_binding_count,
                      acme: binding.last_disabled_acme_renewal_count,
                      time: formatDate(binding.last_takeover_at),
                    })
                  }}
                </div>
              </article>
            </section>
          </TabsContent>

          <TabsContent value="endpoints" class="mt-0 grid min-w-0 gap-3">
            <div
              v-if="!primaryBinding"
              class="grid min-w-0 gap-3 rounded-lg border border-dashed p-4"
            >
              <div
                class="flex min-w-0 flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
              >
                <div class="flex min-w-0 items-start gap-3">
                  <div class="rounded-full bg-muted p-2 text-muted-foreground">
                    <KeyRound class="size-4" />
                  </div>
                  <div class="grid min-w-0 gap-0.5">
                    <h3 class="text-sm font-medium">
                      {{
                        t("admin.certConfig.externalEndpointNoBindingsTitle")
                      }}
                    </h3>
                    <p class="text-xs leading-5 text-muted-foreground">
                      {{
                        t(
                          "admin.certConfig.externalEndpointNoBindingsDescription",
                        )
                      }}
                    </p>
                  </div>
                </div>
                <div class="flex flex-col gap-2 sm:shrink-0 sm:flex-row">
                  <Button
                    size="sm"
                    variant="outline"
                    class="w-full sm:w-auto"
                    :aria-expanded="lanEditorOpen"
                    aria-controls="external-lan-editor"
                    @click="lanEditorOpen = !lanEditorOpen"
                  >
                    <Settings2 class="mr-1.5 size-3.5" />
                    {{
                      lanEditorOpen
                        ? t("admin.certConfig.externalLanClose")
                        : t("admin.certConfig.externalLanConfigure")
                    }}
                  </Button>
                  <Button
                    size="sm"
                    class="w-full sm:w-auto"
                    @click="showBindingSetup"
                  >
                    <Plus class="mr-1.5 size-3.5" />
                    {{ t("admin.certConfig.externalCreate") }}
                  </Button>
                </div>
              </div>

              <ExternalCertificateLanEditor
                v-if="lanEditorOpen"
                id="external-lan-editor"
                v-model:address-draft="lanAddressDraft"
                class="border-t pt-3"
                :settings="lanSettings"
                :saving="isSavingLan"
                @add-address="addDetectedLanAddress"
                @save="saveLan"
              />
            </div>

            <template v-else>
              <p class="text-xs leading-5 text-muted-foreground">
                {{ t("admin.certConfig.externalEndpointOverviewDescription") }}
              </p>

              <div class="min-w-0 overflow-hidden rounded-lg border divide-y">
                <section class="min-w-0 p-3 sm:p-4">
                  <div class="grid min-w-0 gap-1.5">
                    <div class="flex min-w-0 flex-wrap items-center gap-2">
                      <Globe2 class="size-3.5 shrink-0 text-muted-foreground" />
                      <h3 class="text-sm font-medium">
                        {{ t("admin.certConfig.externalEndpointPublic") }}
                      </h3>
                      <Badge variant="secondary" class="text-[11px]">
                        {{
                          primaryBinding.public_deploy_status === "ready"
                            ? t("admin.certConfig.externalLanStatus_ready")
                            : t("admin.certConfig.externalEndpointPending")
                        }}
                      </Badge>
                    </div>
                    <p class="text-xs leading-5 text-muted-foreground sm:pl-5">
                      {{
                        t(publicDeployStatusDescription(primaryBinding), {
                          url: primaryBinding.public_deploy_url,
                          port: primaryBinding.deploy_port,
                        })
                      }}
                    </p>
                    <code
                      v-if="primaryBinding.public_deploy_url"
                      class="mt-0.5 block min-w-0 max-w-full rounded bg-muted px-2.5 py-1.5 text-xs whitespace-normal [overflow-wrap:anywhere]"
                      >{{
                        endpointTemplate(
                          primaryBinding.public_deploy_url,
                          primaryBinding,
                        )
                      }}</code
                    >
                  </div>
                </section>

                <section class="min-w-0 p-3 sm:p-4">
                  <div class="grid min-w-0 gap-1.5">
                    <div class="flex min-w-0 flex-wrap items-center gap-2">
                      <Network
                        class="size-3.5 shrink-0 text-muted-foreground"
                      />
                      <h3 class="text-sm font-medium">
                        {{ t("admin.certConfig.externalEndpointLan") }}
                      </h3>
                      <Badge variant="secondary" class="text-[11px]">
                        {{
                          t(
                            `admin.certConfig.externalLanStatus_${lanSettings?.status ?? "disabled"}`,
                          )
                        }}
                      </Badge>
                      <Button
                        size="sm"
                        variant="ghost"
                        class="h-7 px-2 text-xs"
                        :aria-expanded="lanEditorOpen"
                        aria-controls="external-lan-editor"
                        @click="lanEditorOpen = !lanEditorOpen"
                      >
                        <Settings2 class="mr-1 size-3" />
                        {{
                          lanEditorOpen
                            ? t("admin.certConfig.externalLanClose")
                            : t("admin.certConfig.externalLanEdit")
                        }}
                      </Button>
                    </div>
                    <p class="text-xs leading-5 text-muted-foreground sm:pl-5">
                      {{
                        t("admin.certConfig.externalLanDescription", {
                          port: lanSettings?.gateway_port ?? 7999,
                        })
                      }}
                    </p>
                    <code
                      v-for="address in lanSettings?.configured_addresses ?? []"
                      :key="address"
                      class="mt-0.5 block min-w-0 max-w-full rounded bg-muted px-2.5 py-1.5 text-xs whitespace-normal [overflow-wrap:anywhere]"
                      >{{ lanEndpointTemplate(address) }}</code
                    >

                    <ExternalCertificateLanEditor
                      v-if="lanEditorOpen"
                      id="external-lan-editor"
                      v-model:address-draft="lanAddressDraft"
                      class="mt-2 max-w-2xl border-t pt-3 sm:ml-5"
                      :settings="lanSettings"
                      :saving="isSavingLan"
                      @add-address="addDetectedLanAddress"
                      @save="saveLan"
                    />
                  </div>
                </section>

                <section class="min-w-0 p-3 sm:p-4">
                  <div class="grid min-w-0 gap-1.5">
                    <div class="flex min-w-0 flex-wrap items-center gap-2">
                      <Monitor
                        class="size-3.5 shrink-0 text-muted-foreground"
                      />
                      <h3 class="text-sm font-medium">
                        {{ t("admin.certConfig.externalEndpointLoopback") }}
                      </h3>
                      <Badge variant="secondary" class="text-[11px]">
                        {{ t("admin.certConfig.externalEndpointAlwaysReady") }}
                      </Badge>
                    </div>
                    <p class="text-xs leading-5 text-muted-foreground sm:pl-5">
                      {{
                        t("admin.certConfig.externalLoopbackDescription", {
                          port: primaryBinding.deploy_port,
                        })
                      }}
                    </p>
                    <code
                      class="mt-0.5 block min-w-0 max-w-full rounded bg-muted px-2.5 py-1.5 text-xs whitespace-normal [overflow-wrap:anywhere]"
                      >http://127.0.0.1:{{
                        primaryBinding.deploy_port
                      }}/api/integrations/certificates/{binding_id}</code
                    >
                  </div>
                </section>
              </div>
            </template>
          </TabsContent>
        </Tabs>
      </div>
    </template>

    <template #actions="{ collapse }">
      <Button variant="outline" @click="collapseAndClear(collapse)">
        {{ t("admin.certConfig.collapse") }}
      </Button>
    </template>
  </ConfigCollapsibleCard>
</template>
