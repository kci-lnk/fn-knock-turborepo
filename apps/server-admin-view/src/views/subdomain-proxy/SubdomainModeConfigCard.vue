<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import type { HostMapping } from "@/types";
import type { EdgeClientIpProvider } from "./model";

const props = defineProps<{
  activeEdgeClientIpProvider: EdgeClientIpProvider | null;
  authServiceMapping: HostMapping | null;
  authServicePublicPort: number;
  configured: boolean;
  edgeClientIpEnabled: boolean;
  edgeClientIpProviderOptions: Array<{
    value: EdgeClientIpProvider;
    label: string;
    description: string;
    headerHint: string;
  }>;
  formatAuthServiceHost: (host: string) => string;
  isEdgeClientIpModeEditable: boolean;
  isModeDirty: boolean;
  isModeValid: boolean;
  isSavingMappings: boolean;
  isSavingMode: boolean;
  ready: boolean;
  removeAuthService: () => Promise<unknown>;
  resetModeForm: () => void;
  rootDomain: string;
  rootDomainValidationMessage: string;
  saveMode: () => Promise<unknown>;
  savedEdgeClientIpProviderLabel: string;
  savedRootDomain: string;
  selectEdgeClientIpProvider: (provider: EdgeClientIpProvider) => void;
}>();

const emit = defineEmits<{
  "update:authServicePublicPort": [value: number];
  "update:edgeClientIpEnabled": [value: boolean];
  "update:rootDomain": [value: string];
}>();

const { t } = useI18n();

const authServicePublicPortModel = computed({
  get: () => props.authServicePublicPort,
  set: (value: number | string) => {
    emit("update:authServicePublicPort", Number(value) || 0);
  },
});

const edgeClientIpEnabledModel = computed({
  get: () => props.edgeClientIpEnabled,
  set: (value: boolean) => emit("update:edgeClientIpEnabled", value),
});

const rootDomainModel = computed({
  get: () => props.rootDomain,
  set: (value: string) => emit("update:rootDomain", value),
});

const confirmRemoveAuthService = async () => {
  await props.removeAuthService();
};
</script>

<template>
  <ConfigCollapsibleCard
    :title="t('admin.subdomainProxy.configTitle')"
    :configured="configured"
    :ready="ready"
    :edit-label="t('admin.subdomainProxy.editConfig')"
    summary-class="text-xs text-muted-foreground truncate max-w-full"
    expanded-content-class="p-0 sm:p-0"
    actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col-reverse items-stretch gap-2 rounded-b-lg sm:flex-row sm:items-center sm:justify-end"
  >
    <template #summary>
      <template v-if="savedRootDomain">
        {{
          t("admin.subdomainProxy.rootDomainSummary", {
            domain: savedRootDomain,
          })
        }}
        <span v-if="authServiceMapping">
          ·
          {{
            t("admin.subdomainProxy.authServiceSummary", {
              host: authServiceMapping.host,
            })
          }}
        </span>
        <span v-else>
          · {{ t("admin.subdomainProxy.authServiceMissingSummary") }}
        </span>
        <span v-if="savedEdgeClientIpProviderLabel">
          · {{ savedEdgeClientIpProviderLabel }}
        </span>
      </template>
      <template v-else>
        {{ t("admin.subdomainProxy.notConfiguredSummary") }}
      </template>
    </template>

    <template #default>
      <div class="divide-y divide-border">
        <div class="p-4 sm:p-6">
          <div class="space-y-1">
            <h3 class="text-base font-semibold">
              {{ t("admin.subdomainProxy.configTitle") }}
            </h3>
            <p class="text-sm text-muted-foreground">
              {{ t("admin.subdomainProxy.sectionDescription") }}
            </p>
          </div>
        </div>

        <div class="grid gap-4 p-4 sm:p-6">
          <div class="max-w-xs space-y-2">
            <Label for="root-domain">
              {{ t("admin.subdomainProxy.domainLabel") }}
            </Label>
            <Input
              id="root-domain"
              v-model="rootDomainModel"
              :aria-describedby="
                rootDomainValidationMessage
                  ? 'root-domain-validation'
                  : 'root-domain-hint'
              "
              :aria-invalid="Boolean(rootDomainValidationMessage)"
              placeholder="example.com"
            />
            <p
              v-if="rootDomainValidationMessage"
              id="root-domain-validation"
              class="text-xs text-destructive"
            >
              {{ rootDomainValidationMessage }}
            </p>
            <p
              v-else
              id="root-domain-hint"
              class="text-xs text-muted-foreground"
            >
              {{ t("admin.subdomainProxy.domainHint") }}
            </p>
          </div>

          <div class="rounded-lg border px-4 py-3">
            <div
              class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
            >
              <div class="space-y-1">
                <Label>
                  {{ t("admin.subdomainProxy.currentAuthService") }}
                </Label>
                <div class="text-sm">
                  <template v-if="authServiceMapping">
                    <div class="break-all font-medium">
                      {{ formatAuthServiceHost(authServiceMapping.host) }}
                    </div>
                    <div class="mt-1 text-xs text-muted-foreground">
                      {{
                        t("admin.subdomainProxy.authRedirectHint", {
                          url: `https://${formatAuthServiceHost(
                            authServiceMapping.host,
                          )}`,
                        })
                      }}
                    </div>
                  </template>
                  <p v-else class="text-muted-foreground">
                    {{ t("admin.subdomainProxy.noAuthService") }}
                  </p>
                </div>
              </div>

              <div class="flex flex-col items-end gap-2">
                <Badge :variant="authServiceMapping ? 'secondary' : 'outline'">
                  {{
                    authServiceMapping
                      ? t("admin.subdomainProxy.configured")
                      : t("admin.subdomainProxy.notConfigured")
                  }}
                </Badge>

                <ConfirmDangerPopover
                  v-if="authServiceMapping"
                  :title="t('admin.subdomainProxy.deleteAuthTitle')"
                  :description="
                    t('admin.subdomainProxy.deleteAuthDescription', {
                      host: authServiceMapping.host,
                    })
                  "
                  :confirm-text="t('admin.subdomainProxy.deleteAuthAction')"
                  :loading="isSavingMappings"
                  :disabled="isSavingMappings"
                  :on-confirm="confirmRemoveAuthService"
                  content-class="w-72 text-left"
                >
                  <template #trigger>
                    <Button
                      variant="ghost"
                      size="sm"
                      class="h-auto p-0 text-destructive hover:bg-transparent hover:text-destructive/90"
                      :disabled="isSavingMappings"
                    >
                      {{ t("admin.subdomainProxy.deleteAuthAction") }}
                    </Button>
                  </template>
                </ConfirmDangerPopover>
              </div>
            </div>

            <div
              v-if="!edgeClientIpEnabledModel"
              class="mt-4 grid gap-3 border-t pt-4 sm:grid-cols-[minmax(0,1fr)_12rem] sm:items-end"
            >
              <div class="space-y-1">
                <Label for="auth-service-public-port">
                  {{ t("admin.subdomainProxy.authServicePort") }}
                </Label>
                <p class="text-xs leading-5 text-muted-foreground">
                  {{ t("admin.subdomainProxy.authServicePortHint") }}
                </p>
              </div>
              <Input
                id="auth-service-public-port"
                v-model.number="authServicePublicPortModel"
                type="number"
                min="1"
                max="65535"
                inputmode="numeric"
                class="sm:max-w-48"
              />
            </div>
          </div>

          <div class="rounded-lg border px-4 py-4">
            <div class="flex flex-col gap-4">
              <div class="flex items-start justify-between gap-4">
                <div class="space-y-1">
                  <Label for="edge-client-ip-enabled">
                    {{ t("admin.subdomainProxy.edgeClientIpTitle") }}
                  </Label>
                  <p class="text-xs text-muted-foreground">
                    {{ t("admin.subdomainProxy.edgeClientIpDescription") }}
                  </p>
                  <p class="text-xs text-muted-foreground">
                    {{
                      t("admin.subdomainProxy.edgeClientIpProviderDescription")
                    }}
                  </p>
                  <p
                    v-if="!isEdgeClientIpModeEditable"
                    class="text-xs text-amber-600"
                  >
                    {{ t("admin.subdomainProxy.edgeClientIpNotEditable") }}
                  </p>
                </div>
                <Switch
                  id="edge-client-ip-enabled"
                  v-model="edgeClientIpEnabledModel"
                  :disabled="!isEdgeClientIpModeEditable"
                />
              </div>

              <div v-if="edgeClientIpEnabledModel">
                <div
                  class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between sm:gap-4"
                ></div>

                <div class="mt-4 grid grid-cols-1 gap-3 md:grid-cols-2">
                  <button
                    v-for="option in edgeClientIpProviderOptions"
                    :key="option.value"
                    type="button"
                    :disabled="!isEdgeClientIpModeEditable"
                    :class="[
                      'rounded-xl border p-4 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-60',
                      activeEdgeClientIpProvider === option.value
                        ? 'border-primary bg-primary/5 shadow-sm'
                        : 'border-border bg-background hover:border-primary/40 hover:bg-muted/40',
                    ]"
                    @click="selectEdgeClientIpProvider(option.value)"
                  >
                    <div
                      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
                    >
                      <div class="grid min-w-0 gap-1">
                        <div class="text-sm font-medium">
                          {{ option.label }}
                        </div>
                        <div class="text-xs text-muted-foreground">
                          {{ option.description }}
                        </div>
                        <div class="text-[11px] text-muted-foreground">
                          {{ option.headerHint }}
                        </div>
                      </div>
                      <span
                        :class="[
                          'self-start shrink-0 whitespace-nowrap rounded-full border px-2 py-0.5 text-[11px] font-medium',
                          activeEdgeClientIpProvider === option.value
                            ? 'border-primary/20 bg-primary/10 text-primary'
                            : 'border-border text-muted-foreground',
                        ]"
                      >
                        {{
                          activeEdgeClientIpProvider === option.value
                            ? t("admin.subdomainProxy.current")
                            : t("admin.subdomainProxy.switch")
                        }}
                      </span>
                    </div>
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>

    <template #actions="{ collapse }">
      <Button variant="outline" @click="collapse">
        {{ t("admin.subdomainProxy.collapse") }}
      </Button>
      <Button
        variant="outline"
        :disabled="isSavingMode || !isModeDirty"
        @click="resetModeForm"
      >
        {{ t("admin.subdomainProxy.discardChanges") }}
      </Button>
      <Button
        :disabled="isSavingMode || !isModeValid || !isModeDirty"
        @click="saveMode"
      >
        <span
          v-if="isSavingMode"
          class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
        ></span>
        {{ t("admin.subdomainProxy.saveConfig") }}
      </Button>
    </template>
  </ConfigCollapsibleCard>
</template>
