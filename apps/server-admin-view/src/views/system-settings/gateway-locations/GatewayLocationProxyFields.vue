<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { CircleAlert } from "lucide-vue-next";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import ProxyTargetInputField from "@admin-shared/components/common/ProxyTargetInputField.vue";
import {
  buildProxyPathForwardingPreview,
  type ProxyPathForwardingMode,
} from "@admin-shared/utils/proxyPathForwarding";
import type { GatewayLocationForm } from "./gatewayLocationModel";

const props = defineProps<{
  form: GatewayLocationForm;
  isWebSocketTarget: boolean;
}>();
const { t } = useI18n();

const pathForwardingMode = computed<ProxyPathForwardingMode>({
  get: () => (props.form.strip_path ? "strip" : "keep"),
  set: (mode) => {
    props.form.strip_path = mode === "strip";
  },
});

const pathForwardingPreview = computed(() =>
  buildProxyPathForwardingPreview({
    routePath: props.form.path,
    target: props.form.target,
    mode: pathForwardingMode.value,
  }),
);
</script>

<template>
  <div class="space-y-4">
    <div class="space-y-2">
      <Label for="location-target">
        {{ t("admin.gatewayLocationsSettings.target") }}
      </Label>
      <ProxyTargetInputField
        v-model="form.target"
        input-id="location-target"
        protocol-id="location-target-protocol"
        placeholder="127.0.0.1:8080"
      />
    </div>

    <div
      class="grid gap-3"
      :class="isWebSocketTarget ? 'sm:grid-cols-1' : 'sm:grid-cols-2'"
    >
      <div class="space-y-3 rounded-md border border-border/60 px-4 py-3">
        <div class="space-y-2">
          <Label for="location-path-forwarding">
            {{ t("admin.gatewayLocationsSettings.pathForwarding") }}
          </Label>
          <Select v-model="pathForwardingMode">
            <SelectTrigger id="location-path-forwarding" class="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="strip">
                {{ t("admin.gatewayLocationsSettings.pathForwardingStrip") }}
              </SelectItem>
              <SelectItem value="keep">
                {{ t("admin.gatewayLocationsSettings.pathForwardingKeep") }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="space-y-1 text-xs text-muted-foreground">
          <div class="font-medium text-foreground/80">
            {{ t("admin.gatewayLocationsSettings.pathPreview") }}
          </div>
          <div class="grid gap-1 font-mono">
            <span class="break-all">
              {{ pathForwardingPreview.requestPath }}
            </span>
            <span class="text-foreground">-&gt;</span>
            <span class="break-all">
              {{ pathForwardingPreview.upstreamPath }}
            </span>
          </div>
        </div>
      </div>

      <div
        v-if="!isWebSocketTarget"
        class="flex items-start justify-between gap-4 rounded-md border border-border/60 px-4 py-3"
      >
        <div class="flex min-w-0 items-center gap-1.5 pt-0.5">
          <Label for="location-rewrite-html">
            {{ t("admin.gatewayLocationsSettings.rewriteHtmlPath") }}
          </Label>
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger as-child>
                <button
                  type="button"
                  class="inline-flex h-6 w-6 shrink-0 cursor-help items-center justify-center rounded-full text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                  :aria-label="
                    t('admin.gatewayLocationsSettings.rewriteHtmlPathHelpAria')
                  "
                >
                  <CircleAlert aria-hidden="true" class="h-4 w-4" />
                </button>
              </TooltipTrigger>
              <TooltipContent class="max-w-80 text-left leading-5">
                <p>
                  {{ t("admin.gatewayLocationsSettings.rewriteHtmlPathHelp") }}
                </p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
        <Switch id="location-rewrite-html" v-model="form.rewrite_html" />
      </div>
    </div>
  </div>
</template>
