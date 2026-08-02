<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Info, Loader2, Plus, ShieldCheck, Trash2 } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
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
import type { FirewallAdditionalPortsDetails } from "@/types";
import {
  areFirewallPortListsEqual,
  MAX_FIREWALL_ADDITIONAL_PORTS,
  validateFirewallAdditionalPortDraft,
} from "./firewallAdditionalPortsModel";

const props = defineProps<{
  autoManageFirewallEnabled: boolean;
  details: FirewallAdditionalPortsDetails | null;
  hasUnsavedModeChanges: boolean;
  loadFailed: boolean;
  loading: boolean;
  modeLabel: string;
  open: boolean;
  saving: boolean;
}>();

const emit = defineEmits<{
  retry: [];
  save: [ports: number[]];
  "update:open": [open: boolean];
}>();

type PortDraft = { id: number; value: string };

const { t } = useI18n();
const draft = ref<PortDraft[]>([]);
let nextDraftId = 0;

const resetDraft = () => {
  draft.value = (props.details?.additionalPorts ?? []).map((port) => ({
    id: ++nextDraftId,
    value: String(port),
  }));
};

watch(
  () => [props.open, props.details] as const,
  ([open, details]) => {
    if (open && details) resetDraft();
  },
  { immediate: true },
);

const validation = computed(() =>
  validateFirewallAdditionalPortDraft(draft.value.map((item) => item.value)),
);
const validationMessage = computed(() => {
  if (validation.value.valid) return "";
  return t(
    `admin.runModeSettings.additionalPorts.errors.${validation.value.code}`,
    {
      max: MAX_FIREWALL_ADDITIONAL_PORTS,
    },
  );
});
const unchanged = computed(
  () =>
    validation.value.valid &&
    areFirewallPortListsEqual(
      validation.value.ports,
      props.details?.additionalPorts ?? [],
    ),
);
const canSave = computed(
  () =>
    props.details !== null &&
    !props.loading &&
    !props.saving &&
    validation.value.valid &&
    !unchanged.value,
);

const addPort = () => {
  if (draft.value.length >= MAX_FIREWALL_ADDITIONAL_PORTS) return;
  draft.value.push({ id: ++nextDraftId, value: "" });
};
const removePort = (id: number) => {
  draft.value = draft.value.filter((item) => item.id !== id);
};
const submit = () => {
  if (!canSave.value || !validation.value.valid) return;
  emit("save", validation.value.ports);
};
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[620px]">
      <DialogHeader>
        <DialogTitle>{{
          t("admin.runModeSettings.additionalPorts.title")
        }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.runModeSettings.additionalPorts.description") }}
        </DialogDescription>
      </DialogHeader>

      <div
        v-if="loading"
        class="flex items-center justify-center gap-2 py-12 text-sm text-muted-foreground"
      >
        <Loader2 class="h-4 w-4 animate-spin" />
        {{ t("admin.runModeSettings.additionalPorts.loading") }}
      </div>

      <div
        v-else-if="loadFailed"
        class="rounded-lg border border-dashed px-4 py-10 text-center"
      >
        <p class="text-sm text-muted-foreground">
          {{ t("admin.runModeSettings.additionalPorts.loadFailedInline") }}
        </p>
        <Button class="mt-4" variant="outline" @click="emit('retry')">
          {{ t("admin.runModeSettings.additionalPorts.retry") }}
        </Button>
      </div>

      <div v-else-if="details" class="space-y-5 py-2">
        <Alert class="items-start">
          <ShieldCheck class="mt-0.5 h-4 w-4" />
          <AlertTitle>
            {{
              t("admin.runModeSettings.additionalPorts.currentMode", {
                mode: modeLabel,
              })
            }}
          </AlertTitle>
          <AlertDescription class="space-y-2">
            <p v-if="details.appliedNow">
              {{
                t("admin.runModeSettings.additionalPorts.applyImmediatelyHint")
              }}
            </p>
            <p v-else>
              {{
                t(
                  autoManageFirewallEnabled
                    ? "admin.runModeSettings.additionalPorts.reverseModeHint"
                    : "admin.runModeSettings.additionalPorts.reverseModeManualHint",
                )
              }}
            </p>
            <p
              v-if="hasUnsavedModeChanges"
              class="text-amber-700 dark:text-amber-300"
            >
              {{ t("admin.runModeSettings.additionalPorts.unsavedModeHint") }}
            </p>
          </AlertDescription>
        </Alert>

        <section class="space-y-2">
          <h3 class="text-sm font-medium">
            {{ t("admin.runModeSettings.additionalPorts.automaticTitle") }}
          </h3>
          <p class="text-xs leading-5 text-muted-foreground">
            {{
              t("admin.runModeSettings.additionalPorts.automaticDescription")
            }}
          </p>
          <div
            v-if="details.automaticPorts.length"
            class="flex flex-wrap gap-2"
          >
            <Badge
              v-for="port in details.automaticPorts"
              :key="port"
              variant="secondary"
            >
              {{ port }}
            </Badge>
          </div>
          <p v-else class="text-sm text-muted-foreground">
            {{ t("admin.runModeSettings.additionalPorts.noAutomaticPorts") }}
          </p>
        </section>

        <section class="space-y-3">
          <div>
            <h3 class="text-sm font-medium">
              {{ t("admin.runModeSettings.additionalPorts.customTitle") }}
            </h3>
            <p class="mt-1 text-xs leading-5 text-muted-foreground">
              {{
                t("admin.runModeSettings.additionalPorts.customDescription", {
                  max: MAX_FIREWALL_ADDITIONAL_PORTS,
                })
              }}
            </p>
          </div>

          <div class="max-h-[38vh] space-y-2 overflow-y-auto pr-1">
            <div
              v-for="(item, index) in draft"
              :key="item.id"
              class="flex items-center gap-2"
            >
              <Input
                v-model="item.value"
                inputmode="numeric"
                min="1"
                max="65535"
                :aria-label="
                  t('admin.runModeSettings.additionalPorts.portAria', {
                    number: index + 1,
                  })
                "
                :placeholder="
                  t('admin.runModeSettings.additionalPorts.portPlaceholder')
                "
                :disabled="saving"
              />
              <Button
                type="button"
                variant="ghost"
                size="icon"
                :disabled="saving"
                :aria-label="
                  t('admin.runModeSettings.additionalPorts.deletePort', {
                    port: item.value || index + 1,
                  })
                "
                @click="removePort(item.id)"
              >
                <Trash2 class="h-4 w-4" />
              </Button>
            </div>
          </div>

          <div
            v-if="draft.length === 0"
            class="rounded-md border border-dashed px-4 py-8 text-center text-sm text-muted-foreground"
          >
            {{ t("admin.runModeSettings.additionalPorts.empty") }}
          </div>

          <p
            v-if="validationMessage"
            role="alert"
            class="text-xs text-destructive"
          >
            {{ validationMessage }}
          </p>

          <Button
            type="button"
            variant="outline"
            :disabled="saving || draft.length >= MAX_FIREWALL_ADDITIONAL_PORTS"
            @click="addPort"
          >
            <Plus class="h-4 w-4" />
            {{ t("admin.runModeSettings.additionalPorts.addPort") }}
          </Button>
        </section>

        <Alert
          class="items-start border-amber-300/70 bg-amber-50/60 dark:bg-amber-950/20"
        >
          <Info class="mt-0.5 h-4 w-4" />
          <AlertTitle>{{
            t("admin.runModeSettings.additionalPorts.protocolTitle")
          }}</AlertTitle>
          <AlertDescription>
            {{ t("admin.runModeSettings.additionalPorts.protocolDescription") }}
          </AlertDescription>
        </Alert>
      </div>

      <DialogFooter>
        <Button
          type="button"
          variant="outline"
          :disabled="saving"
          @click="emit('update:open', false)"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button type="button" :disabled="!canSave" @click="submit">
          <Loader2 v-if="saving" class="h-4 w-4 animate-spin" />
          {{ t("admin.runModeSettings.additionalPorts.saveAndApply") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
