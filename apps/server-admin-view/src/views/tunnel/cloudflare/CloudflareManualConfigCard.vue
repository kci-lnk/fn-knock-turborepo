<script setup lang="ts">
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
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import { EyeIcon, EyeOffIcon } from "lucide-vue-next";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

const { controller } = defineProps<{
  controller: CloudflareTunnelController;
}>();
const {
  cloudflaredProtocolDescription,
  cloudflaredProtocolLabel,
  cloudflaredProtocolOptions,
  configLoaded,
  isSaving,
  protocol,
  saveConfig,
  showToken,
  t,
  token,
  tunnelTokenConfigured,
} = controller;
</script>

<template>
  <ConfigCollapsibleCard
    :title="t('admin.cloudflareTunnel.manual.title')"
    :configured="true"
    :ready="configLoaded"
    expanded-content-class="p-0 sm:p-0"
  >
    <template #summary>
      {{
        t("admin.cloudflareTunnel.configSummary", {
          token: tunnelTokenConfigured
            ? "********"
            : t("admin.cloudflareTunnel.notConfigured"),
          protocol: cloudflaredProtocolLabel,
        })
      }}
    </template>

    <template #default>
      <div class="divide-y divide-border">
        <div class="grid items-start gap-2 p-4 sm:grid-cols-[220px_1fr] sm:p-6">
          <div class="space-y-1">
            <Label for="cloudflared-token">
              {{ t("admin.cloudflareTunnel.manual.tunnelTokenLabel") }}
            </Label>
            <p class="pr-4 text-xs leading-relaxed text-muted-foreground">
              {{ t("admin.cloudflareTunnel.manual.tokenDescription") }}
            </p>
          </div>
          <div class="relative w-full max-w-xl">
            <Input
              id="cloudflared-token"
              v-model.trim="token"
              class="pr-10"
              :placeholder="
                tunnelTokenConfigured
                  ? t('admin.cloudflareTunnel.manual.replaceTokenPlaceholder')
                  : 'eyJh...'
              "
              :type="showToken ? 'text' : 'password'"
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
                showToken ? t('common.hideSecret') : t('common.showSecret')
              "
              class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
              @click="showToken = !showToken"
            >
              <EyeIcon v-if="showToken" class="size-4" />
              <EyeOffIcon v-else class="size-4" />
            </button>
          </div>
        </div>

        <div class="grid items-start gap-2 p-4 sm:grid-cols-[220px_1fr] sm:p-6">
          <div class="space-y-1">
            <Label for="cloudflared-protocol">
              {{ t("admin.cloudflareTunnel.protocolLabel") }}
            </Label>
            <p class="pr-4 text-xs leading-relaxed text-muted-foreground">
              {{ t("admin.cloudflareTunnel.protocolDescription") }}
            </p>
          </div>
          <div class="w-full max-w-xl space-y-2">
            <Select v-model="protocol">
              <SelectTrigger id="cloudflared-protocol"
                ><SelectValue
              /></SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="option in cloudflaredProtocolOptions"
                  :key="option.value"
                  :value="option.value"
                >
                  {{ option.label }}
                </SelectItem>
              </SelectContent>
            </Select>
            <p class="text-xs text-muted-foreground">
              {{ cloudflaredProtocolDescription }}
            </p>
          </div>
        </div>
      </div>
    </template>

    <template #actions="{ collapse }">
      <div
        class="flex items-center justify-end gap-3 rounded-b-lg border-t bg-muted/30 p-4 sm:px-6"
      >
        <Button variant="outline" @click="collapse">
          {{ t("admin.cloudflareTunnel.collapse") }}
        </Button>
        <Button :disabled="isSaving" @click="saveConfig">
          {{ t("common.save") }}
        </Button>
      </div>
    </template>
  </ConfigCollapsibleCard>
</template>
