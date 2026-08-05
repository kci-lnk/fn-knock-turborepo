<script setup lang="ts">
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import {
  Cloud,
  EyeIcon,
  EyeOffIcon,
  LoaderCircle,
  ShieldAlert,
} from "lucide-vue-next";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

const { controller } = defineProps<{
  controller: CloudflareTunnelController;
}>();
const {
  apiToken,
  apiTokenConfigured,
  configLoaded,
  connectCloudflare,
  disconnectCloudflare,
  hasSubdomainRoot,
  isConnectingCloudflare,
  isLoadingManagedState,
  managedState,
  showApiToken,
  t,
} = controller;
</script>

<template>
  <ConfigCollapsibleCard
    :title="t('admin.cloudflareTunnel.managed.connectionTitle')"
    :configured="
      apiTokenConfigured &&
      !managedState?.connection.remoteError &&
      !managedState?.connection.rootDomainDrift
    "
    :ready="configLoaded && !isLoadingManagedState"
    :edit-label="t('admin.cloudflareTunnel.managed.viewOrChange')"
    collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
    summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
    expanded-content-class="p-0 sm:p-0"
  >
    <template #summary>
      {{
        apiTokenConfigured
          ? managedState?.connection.zoneName
            ? t("admin.cloudflareTunnel.managed.connectionSummaryWithZone", {
                zone: managedState.connection.zoneName,
              })
            : t("admin.cloudflareTunnel.managed.connectionSummary")
          : t("admin.cloudflareTunnel.managed.connectionNotConfiguredSummary")
      }}
    </template>

    <template #default>
      <div class="space-y-5 p-4 sm:p-6">
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="max-w-3xl space-y-1">
            <div class="flex items-center gap-2 text-base font-semibold">
              <Cloud class="size-5" />
              {{ t("admin.cloudflareTunnel.managed.connectionHeading") }}
            </div>
            <p class="text-sm text-muted-foreground">
              {{ t("admin.cloudflareTunnel.managed.connectionDescription") }}
            </p>
          </div>
          <Badge :variant="apiTokenConfigured ? 'default' : 'secondary'">
            {{
              apiTokenConfigured
                ? t("admin.cloudflareTunnel.managed.connectedStatus")
                : t("admin.cloudflareTunnel.notConfigured")
            }}
          </Badge>
        </div>

        <div class="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto]">
          <div class="space-y-2">
            <Label for="cloudflare-api-token">Cloudflare API Token</Label>
            <div class="relative">
              <Input
                id="cloudflare-api-token"
                v-model.trim="apiToken"
                class="pr-10"
                :placeholder="
                  apiTokenConfigured
                    ? t(
                        'admin.cloudflareTunnel.managed.replaceTokenPlaceholder',
                      )
                    : 'cfat_... / cfut_...'
                "
                :type="showApiToken ? 'text' : 'password'"
                autocomplete="new-password"
                autocapitalize="off"
                autocorrect="off"
                :spellcheck="false"
                data-form-type="other"
                data-1p-ignore="true"
                data-lpignore="true"
                data-bwignore="true"
              />
              <button
                type="button"
                :aria-label="
                  showApiToken ? t('common.hideSecret') : t('common.showSecret')
                "
                class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                @click="showApiToken = !showApiToken"
              >
                <EyeIcon v-if="showApiToken" class="size-4" />
                <EyeOffIcon v-else class="size-4" />
              </button>
            </div>
          </div>
          <div class="flex items-end gap-2">
            <Button
              :disabled="
                !apiToken.trim() || isConnectingCloudflare || !hasSubdomainRoot
              "
              @click="connectCloudflare"
            >
              <LoaderCircle
                v-if="isConnectingCloudflare"
                class="mr-2 size-4 animate-spin"
              />
              {{
                apiTokenConfigured
                  ? t("admin.cloudflareTunnel.managed.replaceToken")
                  : t("admin.cloudflareTunnel.managed.connect")
              }}
            </Button>
            <Button
              v-if="apiTokenConfigured"
              variant="outline"
              :disabled="isConnectingCloudflare"
              @click="disconnectCloudflare"
            >
              {{ t("admin.cloudflareTunnel.managed.disconnect") }}
            </Button>
          </div>
        </div>

        <div
          v-if="managedState?.connection.zoneName"
          class="rounded-lg border bg-muted/20 px-4 py-3 text-sm"
        >
          <span class="text-muted-foreground">
            {{ t("admin.cloudflareTunnel.managed.zone") }}:
          </span>
          <span class="ml-2 font-medium">
            {{ managedState.connection.zoneName }}
          </span>
        </div>

        <Alert
          v-if="managedState?.connection.remoteError"
          variant="destructive"
          class="items-start"
        >
          <ShieldAlert class="size-4" />
          <AlertTitle>{{
            t("admin.cloudflareTunnel.managed.remoteErrorTitle")
          }}</AlertTitle>
          <AlertDescription>
            {{ managedState.connection.remoteError }}
          </AlertDescription>
        </Alert>

        <Alert
          v-if="managedState?.connection.rootDomainDrift"
          variant="destructive"
          class="items-start"
        >
          <ShieldAlert class="size-4" />
          <AlertTitle>{{
            t("admin.cloudflareTunnel.managed.driftTitle")
          }}</AlertTitle>
          <AlertDescription>
            {{ t("admin.cloudflareTunnel.managed.driftDescription") }}
          </AlertDescription>
        </Alert>
      </div>
    </template>

    <template #actions="{ collapse }">
      <div
        class="flex justify-end rounded-b-lg border-t bg-muted/30 p-4 sm:px-6"
      >
        <Button variant="outline" @click="collapse">
          {{ t("admin.cloudflareTunnel.collapse") }}
        </Button>
      </div>
    </template>
  </ConfigCollapsibleCard>
</template>
