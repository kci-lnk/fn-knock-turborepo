<script setup lang="ts">
import { computed, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  CheckCircle2,
  CircleStop,
  Link2,
  Loader2,
  Play,
  RadioTower,
} from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  type WOLLocalRelayInput,
  type WOLLocalRelayRuntime,
} from "@/lib/api/wol";

const props = defineProps<{
  model: WOLLocalRelayInput;
  pskConfigured: boolean;
  runtime: WOLLocalRelayRuntime | null;
  saving: boolean;
}>();

const emit = defineEmits<{
  pair: [pairingCode: string];
  save: [];
}>();
const { t } = useI18n();
const id = useId();
const pairingCode = ref("");
const showPairing = ref(false);

const splitLines = (value: string) =>
  value
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter(Boolean);

const broadcastDestinations = computed({
  get: () => props.model.broadcastDestinations.join("\n"),
  set: (value: string) => {
    props.model.broadcastDestinations = splitLines(value);
  },
});

const allowedSources = computed({
  get: () => props.model.allowedSources.join("\n"),
  set: (value: string) => {
    props.model.allowedSources = splitLines(value);
  },
});

watch(
  () => props.pskConfigured,
  (configured) => {
    if (configured) {
      pairingCode.value = "";
      showPairing.value = false;
    }
  },
);

const pair = () => {
  const code = pairingCode.value.trim();
  if (code) emit("pair", code);
};

const setEnabled = (enabled: boolean) => {
  props.model.enabled = enabled;
  emit("save");
};
</script>

<template>
  <Card>
    <CardHeader class="gap-3 border-b">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div class="space-y-1">
          <CardTitle class="flex items-center gap-2 text-base">
            <RadioTower class="h-4 w-4" />
            {{ t("admin.wol.localRelay.title") }}
          </CardTitle>
          <p class="text-sm leading-6 text-muted-foreground">
            {{ t("admin.wol.localRelay.description") }}
          </p>
        </div>
        <Badge
          :variant="
            runtime?.active
              ? 'default'
              : pskConfigured
                ? 'secondary'
                : 'outline'
          "
        >
          {{
            runtime?.active
              ? t("admin.wol.localRelay.ready")
              : pskConfigured && model.enabled
                ? t("admin.wol.localRelay.starting")
                : pskConfigured
                  ? t("admin.wol.localRelay.paused")
                  : t("admin.wol.localRelay.notPaired")
          }}
        </Badge>
      </div>
    </CardHeader>

    <CardContent class="space-y-5 pt-5">
      <Alert v-if="runtime?.lastError" variant="destructive">
        <AlertTitle>{{ t("admin.wol.localRelay.runtimeError") }}</AlertTitle>
        <AlertDescription>{{ runtime.lastError }}</AlertDescription>
      </Alert>

      <div
        v-if="!pskConfigured || showPairing"
        class="space-y-4 rounded-xl border bg-muted/20 p-4"
      >
        <div class="flex items-start gap-3">
          <Link2 class="mt-0.5 h-5 w-5 text-primary" />
          <div>
            <p class="font-medium">{{ t("admin.wol.localRelay.pairTitle") }}</p>
            <p class="mt-1 text-sm leading-6 text-muted-foreground">
              {{ t("admin.wol.localRelay.pairDescription") }}
            </p>
          </div>
        </div>
        <Label :for="`${id}-pairing-code`" class="sr-only">
          {{ t("admin.wol.localRelay.pairTitle") }}
        </Label>
        <Textarea
          :id="`${id}-pairing-code`"
          v-model="pairingCode"
          class="min-h-24 font-mono text-xs"
          autocomplete="off"
          spellcheck="false"
          :placeholder="t('admin.wol.localRelay.pairingCodePlaceholder')"
        />
        <div class="flex justify-end gap-2">
          <Button
            v-if="pskConfigured"
            variant="outline"
            :disabled="saving"
            @click="showPairing = false"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button :disabled="saving || !pairingCode.trim()" @click="pair">
            <Loader2 v-if="saving" class="mr-1.5 h-4 w-4 animate-spin" />
            <Link2 v-else class="mr-1.5 h-4 w-4" />
            {{ t("admin.wol.localRelay.pairAndEnable") }}
          </Button>
        </div>
      </div>

      <div
        v-else
        class="space-y-4 rounded-xl border border-primary/20 bg-primary/5 p-4"
      >
        <div class="flex items-start gap-3">
          <CheckCircle2 class="mt-0.5 h-5 w-5 text-primary" />
          <div class="min-w-0 flex-1">
            <p class="font-medium">
              {{
                runtime?.active
                  ? t("admin.wol.localRelay.readyTitle")
                  : model.enabled
                    ? t("admin.wol.localRelay.startingTitle")
                    : t("admin.wol.localRelay.pausedTitle")
              }}
            </p>
            <p class="mt-1 text-sm leading-6 text-muted-foreground">
              {{
                runtime?.active
                  ? t("admin.wol.localRelay.readyDescription")
                  : model.enabled
                    ? t("admin.wol.localRelay.startingDescription")
                    : t("admin.wol.localRelay.pausedDescription")
              }}
            </p>
            <p
              v-if="runtime?.listenAddress"
              class="mt-1 text-xs text-muted-foreground"
            >
              {{
                t("admin.wol.localRelay.runtimeAddress", {
                  address: runtime.listenAddress,
                })
              }}
            </p>
          </div>
        </div>
        <div class="flex flex-wrap justify-end gap-2">
          <Button
            variant="outline"
            :disabled="saving"
            @click="showPairing = true"
          >
            <Link2 class="mr-1.5 h-4 w-4" />
            {{ t("admin.wol.localRelay.repair") }}
          </Button>
          <Button
            v-if="model.enabled"
            variant="outline"
            :disabled="saving"
            @click="setEnabled(false)"
          >
            <CircleStop class="mr-1.5 h-4 w-4" />
            {{ t("admin.wol.localRelay.stop") }}
          </Button>
          <Button v-else :disabled="saving" @click="setEnabled(true)">
            <Play class="mr-1.5 h-4 w-4" />
            {{ t("admin.wol.localRelay.start") }}
          </Button>
        </div>
      </div>

      <details v-if="pskConfigured" class="rounded-lg border px-4 py-3">
        <summary class="cursor-pointer text-sm text-muted-foreground">
          {{ t("admin.wol.advancedSettings") }}
        </summary>
        <form
          class="mt-4 space-y-4 border-t pt-4"
          autocomplete="off"
          @submit.prevent="emit('save')"
        >
          <p class="text-xs leading-5 text-muted-foreground">
            {{ t("admin.wol.localRelay.advancedHint") }}
          </p>
          <div class="grid gap-4 sm:grid-cols-[1fr_140px]">
            <div class="space-y-2">
              <Label :for="`${id}-listen-address`">{{
                t("admin.wol.localRelay.listenAddress")
              }}</Label>
              <Input
                :id="`${id}-listen-address`"
                v-model="model.listenAddress"
                autocomplete="off"
                spellcheck="false"
              />
            </div>
            <div class="space-y-2">
              <Label :for="`${id}-port`">{{ t("admin.wol.port") }}</Label>
              <Input
                :id="`${id}-port`"
                v-model.number="model.port"
                type="number"
                min="1"
                max="65535"
                inputmode="numeric"
              />
            </div>
          </div>
          <div class="grid gap-4 lg:grid-cols-2">
            <div class="space-y-2">
              <Label :for="`${id}-broadcasts`">{{
                t("admin.wol.localRelay.broadcastDestinations")
              }}</Label>
              <Textarea
                :id="`${id}-broadcasts`"
                v-model="broadcastDestinations"
                class="min-h-24 font-mono text-xs"
                spellcheck="false"
              />
            </div>
            <div class="space-y-2">
              <Label :for="`${id}-sources`">{{
                t("admin.wol.localRelay.allowedSources")
              }}</Label>
              <Textarea
                :id="`${id}-sources`"
                v-model="allowedSources"
                class="min-h-24 font-mono text-xs"
                spellcheck="false"
              />
            </div>
          </div>
          <div class="flex justify-end">
            <Button type="submit" :disabled="saving">
              <Loader2 v-if="saving" class="mr-1.5 h-4 w-4 animate-spin" />
              {{ saving ? t("admin.wol.saving") : t("common.save") }}
            </Button>
          </div>
        </form>
      </details>
    </CardContent>
  </Card>
</template>
