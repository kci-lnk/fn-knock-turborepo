<script setup lang="ts">
import {
  CheckCircle2,
  ExternalLink,
  Link2,
  LoaderCircle,
  RotateCcw,
  Save,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { InputGroup, InputGroupInput } from "@/components/ui/input-group";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import { useIpLocationSettings } from "./ip-location/useIpLocationSettings";

const {
  cidrDockerUrl,
  cidrUrlInput,
  form,
  ipLookupDockerUrl,
  ipLookupUrlInput,
  isDirty,
  isLoading,
  isSaving,
  isTestingCidr,
  isTestingIpLookup,
  resetForm,
  saveSettings,
  showLoadingSkeleton,
  t,
  testCidrService,
  testIpLookupService,
} = useIpLocationSettings();
</script>

<template>
  <div class="w-full space-y-4">
    <div v-if="isLoading && showLoadingSkeleton" class="grid gap-4">
      <section class="rounded-xl border bg-card p-5 shadow-sm">
        <div class="flex gap-3">
          <Skeleton class="size-10 rounded-lg" />
          <div class="flex-1 space-y-2">
            <Skeleton class="h-5 w-32" />
            <Skeleton class="h-4 w-4/5" />
          </div>
        </div>
        <div class="mt-6 space-y-3">
          <Skeleton class="h-4 w-24" />
          <Skeleton class="h-9 w-full" />
          <Skeleton class="h-20 w-full" />
        </div>
      </section>
      <section class="rounded-xl border bg-card p-5 shadow-sm">
        <div class="flex gap-3">
          <Skeleton class="size-10 rounded-lg" />
          <div class="flex-1 space-y-2">
            <Skeleton class="h-5 w-32" />
            <Skeleton class="h-4 w-4/5" />
          </div>
        </div>
        <div class="mt-6 space-y-3">
          <Skeleton class="h-4 w-24" />
          <Skeleton class="h-9 w-full" />
          <Skeleton class="h-20 w-full" />
        </div>
      </section>
    </div>

    <div v-else-if="!isLoading" class="grid gap-4">
      <section
        class="flex min-h-full flex-col overflow-hidden rounded-xl border bg-card shadow-sm"
      >
        <div class="border-b bg-muted/10 p-4 sm:p-5">
          <div class="flex gap-3">
            <div class="min-w-0 space-y-1">
              <h3 class="text-base font-semibold tracking-normal">
                {{ t("admin.ipLocationSettings.ipLookupTitle") }}
              </h3>
              <p class="text-sm leading-6 text-muted-foreground">
                {{ t("admin.ipLocationSettings.ipLookupDescription") }}
              </p>
            </div>
          </div>
        </div>

        <div class="flex w-full flex-1 flex-col gap-5 p-4 sm:p-5">
          <div class="space-y-2">
            <div class="flex items-center justify-between gap-3">
              <Label for="ip-location-lookup-mode" class="text-sm font-medium">
                {{ t("admin.ipLocationSettings.serviceSource") }}
              </Label>
            </div>
            <Select v-model="form.ip_lookup_mode" :disabled="isSaving">
              <SelectTrigger id="ip-location-lookup-mode" class="w-full">
                <SelectValue
                  :placeholder="
                    t('admin.ipLocationSettings.chooseServiceSource')
                  "
                />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="online">
                  {{ t("admin.ipLocationSettings.officialOnlineService") }}
                </SelectItem>
                <SelectItem value="custom">
                  {{ t("admin.ipLocationSettings.customService") }}
                </SelectItem>
              </SelectContent>
            </Select>
            <p class="text-sm leading-6 text-muted-foreground">
              {{ t("admin.ipLocationSettings.ipLookupModeHint") }}
            </p>
          </div>

          <div
            v-if="form.ip_lookup_mode === 'online'"
            class="mt-auto rounded-lg border border-dashed bg-muted/20 p-4"
          >
            <div class="flex gap-3">
              <CheckCircle2 class="mt-0.5 size-4 shrink-0 text-emerald-600" />
              <div class="min-w-0 space-y-1">
                <p class="text-sm font-medium">
                  {{ t("admin.ipLocationSettings.usingOfficialService") }}
                </p>
              </div>
            </div>
          </div>

          <div
            v-if="form.ip_lookup_mode === 'custom'"
            class="animate-in fade-in slide-in-from-top-2 space-y-4 duration-200"
          >
            <div class="rounded-lg border bg-muted/20 p-4">
              <div class="flex gap-3">
                <div class="space-y-1 text-sm">
                  <p class="font-medium">
                    {{ t("admin.ipLocationSettings.selfHostedService") }}
                  </p>
                  <p class="leading-6 text-muted-foreground">
                    {{ t("admin.ipLocationSettings.canUse") }}
                    <Button
                      as-child
                      variant="link"
                      data-affordance="details"
                      class="h-auto gap-1 p-0 align-baseline text-sm"
                    >
                      <a
                        :href="ipLookupDockerUrl"
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        go-ipaddress-api
                        <ExternalLink class="size-3.5" aria-hidden="true" />
                      </a>
                    </Button>
                    {{ t("admin.ipLocationSettings.deploySuffix") }}
                  </p>
                </div>
              </div>
            </div>

            <div class="space-y-2">
              <Label for="ip-location-lookup-url" class="text-sm font-medium">
                Base URL
              </Label>
              <div class="flex flex-col gap-2 sm:flex-row">
                <InputGroup class="sm:flex-1">
                  <InputGroupInput
                    id="ip-location-lookup-url"
                    v-model="ipLookupUrlInput"
                    :placeholder="
                      t('admin.ipLocationSettings.ipLookupPlaceholder')
                    "
                    :disabled="isSaving"
                  />
                </InputGroup>
                <Button
                  variant="outline"
                  class="w-full sm:w-auto"
                  :disabled="isTestingIpLookup || !ipLookupUrlInput.trim()"
                  @click="testIpLookupService"
                >
                  <LoaderCircle
                    v-if="isTestingIpLookup"
                    class="size-4 animate-spin"
                  />
                  <Link2 v-else class="size-4" />
                  {{
                    isTestingIpLookup
                      ? t("admin.ipLocationSettings.testing")
                      : t("admin.ipLocationSettings.testConnection")
                  }}
                </Button>
              </div>
              <p class="text-xs text-muted-foreground">
                {{ t("admin.ipLocationSettings.baseUrlHint") }}
              </p>
            </div>
          </div>
        </div>
      </section>

      <section
        class="flex min-h-full flex-col overflow-hidden rounded-xl border bg-card shadow-sm"
      >
        <div class="border-b bg-muted/10 p-4 sm:p-5">
          <div class="flex gap-3">
            <div class="min-w-0 space-y-1">
              <h3 class="text-base font-semibold tracking-normal">
                {{ t("admin.ipLocationSettings.cidrTitle") }}
              </h3>
              <p class="text-sm leading-6 text-muted-foreground">
                {{ t("admin.ipLocationSettings.cidrDescription") }}
              </p>
            </div>
          </div>
        </div>

        <div class="flex w-full flex-1 flex-col gap-5 p-4 sm:p-5">
          <div class="space-y-2">
            <div class="flex items-center justify-between gap-3">
              <Label for="ip-location-cidr-mode" class="text-sm font-medium">
                {{ t("admin.ipLocationSettings.serviceSource") }}
              </Label>
            </div>
            <Select v-model="form.cidr_mode" :disabled="isSaving">
              <SelectTrigger id="ip-location-cidr-mode" class="w-full">
                <SelectValue
                  :placeholder="
                    t('admin.ipLocationSettings.chooseServiceSource')
                  "
                />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="online">
                  {{ t("admin.ipLocationSettings.officialOnlineService") }}
                </SelectItem>
                <SelectItem value="custom">
                  {{ t("admin.ipLocationSettings.customService") }}
                </SelectItem>
              </SelectContent>
            </Select>
            <p class="text-sm leading-6 text-muted-foreground">
              {{ t("admin.ipLocationSettings.cidrModeHint") }}
            </p>
          </div>

          <div
            v-if="form.cidr_mode === 'online'"
            class="mt-auto rounded-lg border border-dashed bg-muted/20 p-4"
          >
            <div class="flex gap-3">
              <CheckCircle2 class="mt-0.5 size-4 shrink-0 text-emerald-600" />
              <div class="min-w-0 space-y-1">
                <p class="text-sm font-medium">
                  {{ t("admin.ipLocationSettings.usingOfficialService") }}
                </p>
              </div>
            </div>
          </div>

          <div
            v-if="form.cidr_mode === 'custom'"
            class="animate-in fade-in slide-in-from-top-2 space-y-4 duration-200"
          >
            <div class="rounded-lg border bg-muted/20 p-4">
              <div class="flex gap-3">
                <div class="space-y-1 text-sm">
                  <p class="font-medium">
                    {{ t("admin.ipLocationSettings.selfHostedService") }}
                  </p>
                  <p class="leading-6 text-muted-foreground">
                    {{ t("admin.ipLocationSettings.canUse") }}
                    <Button
                      as-child
                      variant="link"
                      data-affordance="details"
                      class="h-auto gap-1 p-0 align-baseline text-sm"
                    >
                      <a
                        :href="cidrDockerUrl"
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        go-cidr-api
                        <ExternalLink class="size-3.5" aria-hidden="true" />
                      </a>
                    </Button>
                    {{ t("admin.ipLocationSettings.deploySuffix") }}
                  </p>
                </div>
              </div>
            </div>

            <div class="space-y-2">
              <Label for="ip-location-cidr-url" class="text-sm font-medium">
                Base URL
              </Label>
              <div class="flex flex-col gap-2 sm:flex-row">
                <InputGroup class="sm:flex-1">
                  <InputGroupInput
                    id="ip-location-cidr-url"
                    v-model="cidrUrlInput"
                    :placeholder="t('admin.ipLocationSettings.cidrPlaceholder')"
                    :disabled="isSaving"
                  />
                </InputGroup>
                <Button
                  variant="outline"
                  class="w-full sm:w-auto"
                  :disabled="isTestingCidr || !cidrUrlInput.trim()"
                  @click="testCidrService"
                >
                  <LoaderCircle
                    v-if="isTestingCidr"
                    class="size-4 animate-spin"
                  />
                  <Link2 v-else class="size-4" />
                  {{
                    isTestingCidr
                      ? t("admin.ipLocationSettings.testing")
                      : t("admin.ipLocationSettings.testConnection")
                  }}
                </Button>
              </div>
              <p class="text-xs text-muted-foreground">
                {{ t("admin.ipLocationSettings.baseUrlHint") }}
              </p>
            </div>
          </div>
        </div>
      </section>
    </div>

    <div
      v-else
      class="min-h-[220px] rounded-xl border bg-card"
      aria-hidden="true"
    />

    <FloatingActionDock
      :active="isDirty"
      inline-class="rounded-xl border bg-card px-4 py-4 shadow-sm sm:px-6"
    >
      <template #inline>
        <div
          class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-end"
        >
          <div class="flex gap-3">
            <Button
              variant="outline"
              class="flex-1 sm:flex-none"
              :disabled="!isDirty || isSaving"
              @click="resetForm"
            >
              <RotateCcw class="size-4" />
              {{ t("admin.ipLocationSettings.discard") }}
            </Button>
            <Button
              class="flex-1 sm:flex-none"
              :disabled="!isDirty || isSaving"
              @click="saveSettings"
            >
              <LoaderCircle v-if="isSaving" class="size-4 animate-spin" />
              <Save v-else class="size-4" />
              {{
                isSaving
                  ? t("admin.ipLocationSettings.saving")
                  : t("admin.ipLocationSettings.saveChanges")
              }}
            </Button>
          </div>
        </div>
      </template>

      <template #floating>
        <Button
          variant="outline"
          :disabled="!isDirty || isSaving"
          @click="resetForm"
        >
          <RotateCcw class="size-4" />
          {{ t("admin.ipLocationSettings.discard") }}
        </Button>
        <Button :disabled="!isDirty || isSaving" @click="saveSettings">
          <LoaderCircle v-if="isSaving" class="size-4 animate-spin" />
          <Save v-else class="size-4" />
          {{
            isSaving
              ? t("admin.ipLocationSettings.saving")
              : t("admin.ipLocationSettings.saveChanges")
          }}
        </Button>
      </template>
    </FloatingActionDock>
  </div>
</template>
