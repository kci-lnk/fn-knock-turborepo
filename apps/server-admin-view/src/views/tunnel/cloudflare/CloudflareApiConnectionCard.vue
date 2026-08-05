<script setup lang="ts">
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  CheckCircle2,
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
  connectCloudflare,
  disconnectCloudflare,
  hasSubdomainRoot,
  isConnectingCloudflare,
  managedState,
  showApiToken,
  t,
} = controller;
</script>

<template>
  <Card>
    <CardHeader>
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div class="space-y-1">
          <CardTitle class="flex items-center gap-2">
            <Cloud class="size-5" />
            {{ t("admin.cloudflareTunnel.managed.connectionTitle") }}
          </CardTitle>
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
    </CardHeader>
    <CardContent class="space-y-5">
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
                  ? t('admin.cloudflareTunnel.managed.replaceTokenPlaceholder')
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

      <div class="rounded-lg border bg-muted/20 p-4">
        <div class="mb-2 text-sm font-medium">
          {{ t("admin.cloudflareTunnel.managed.permissionsTitle") }}
        </div>
        <ul class="grid gap-1 text-xs text-muted-foreground sm:grid-cols-2">
          <li
            v-for="permission in managedState?.permissions || [
              'Account / Cloudflare Tunnel / Edit',
              'Zone / Zone / Read',
              'Zone / DNS / Edit',
              'Zone / SSL and Certificates / Edit (optimization only)',
            ]"
            :key="permission"
            class="flex items-center gap-2"
          >
            <CheckCircle2 class="size-3.5 text-emerald-500" />
            {{ permission }}
          </li>
        </ul>
      </div>

      <div
        v-if="managedState?.connection.zoneName"
        class="grid gap-3 text-sm sm:grid-cols-3"
      >
        <div class="rounded-md border p-3">
          <div class="text-xs text-muted-foreground">
            {{ t("admin.cloudflareTunnel.managed.zone") }}
          </div>
          <div class="mt-1 break-all font-medium">
            {{ managedState.connection.zoneName }}
          </div>
        </div>
        <div class="rounded-md border p-3">
          <div class="text-xs text-muted-foreground">
            {{ t("admin.cloudflareTunnel.managed.accountId") }}
          </div>
          <code class="mt-1 block break-all text-xs">
            {{ managedState.connection.accountId }}
          </code>
        </div>
        <div class="rounded-md border p-3">
          <div class="text-xs text-muted-foreground">
            {{ t("admin.cloudflareTunnel.managed.zoneId") }}
          </div>
          <code class="mt-1 block break-all text-xs">
            {{ managedState.connection.zoneId }}
          </code>
        </div>
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
    </CardContent>
  </Card>
</template>
