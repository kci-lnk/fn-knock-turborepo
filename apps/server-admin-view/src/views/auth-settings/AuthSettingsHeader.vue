<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import {
  ChevronDown,
  FileKey2,
  Plus,
  RefreshCw,
  Settings2,
} from "lucide-vue-next";
import { docsUrls } from "../../lib/docs";
import type { AuthSettingsPageController } from "./useAuthSettingsPage";

const props = defineProps<{ controller: AuthSettingsPageController }>();
const { t } = useI18n();
const {
  authSettingsDescription,
  authSettingsTitle,
  goToOidcProviders,
  handlePrimaryAuthAction,
  isAuthModeBusy,
  isCredentialTransferBusy,
  openAuthModeSwitchDialog,
  primaryAuthActionLabel,
  showCredentialTransferDialog,
} = props.controller;
</script>

<template>
  <div
    class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
  >
    <div class="min-w-0 space-y-1">
      <div class="flex items-center justify-between gap-3">
        <h2 class="text-lg font-semibold tracking-tight">
          {{ authSettingsTitle }}
        </h2>
        <DocsLinkButton class="sm:hidden" :href="docsUrls.guides.auth" />
      </div>
      <p class="text-sm text-muted-foreground">
        {{ authSettingsDescription }}
      </p>
    </div>
    <div class="flex w-full items-center gap-2 sm:w-auto">
      <DocsLinkButton
        class="hidden sm:inline-flex"
        :href="docsUrls.guides.auth"
        size="default"
      />
      <div class="grid flex-1 grid-cols-[minmax(0,1fr)_auto] gap-0 sm:flex-none">
        <Button
          class="h-11 min-w-0 rounded-r-none sm:h-9"
          @click="handlePrimaryAuthAction"
        >
          <Plus class="h-4 w-4" aria-hidden="true" />
          {{ primaryAuthActionLabel }}
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button
              size="icon"
              class="h-11 rounded-l-none border-l border-primary-foreground/25 px-2 sm:h-9"
              :aria-label="t('admin.authSettings.moreActions')"
              :title="t('admin.authSettings.moreActions')"
            >
              <ChevronDown class="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-56">
            <DropdownMenuItem
              :disabled="isCredentialTransferBusy"
              @select="showCredentialTransferDialog = true"
            >
              <FileKey2 class="mr-2 h-4 w-4" />
              {{ t("admin.authSettings.credentialTransfer") }}
            </DropdownMenuItem>
            <DropdownMenuItem
              :disabled="isAuthModeBusy"
              @select="openAuthModeSwitchDialog"
            >
              <RefreshCw
                class="mr-2 h-4 w-4"
                :class="{ 'animate-spin': isAuthModeBusy }"
              />
              {{ t("admin.authSettings.switchAuthMode") }}
            </DropdownMenuItem>
            <DropdownMenuItem @select="goToOidcProviders">
              <Settings2 class="mr-2 h-4 w-4" />
              {{ t("admin.authSettings.oidcLogin") }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  </div>
</template>
