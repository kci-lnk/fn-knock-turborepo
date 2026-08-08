<script setup lang="ts">
import { onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Cable,
  ChevronDown,
  Link2,
  Loader2,
  MonitorUp,
  Pencil,
  Plus,
  Power,
  Radar,
  RadioTower,
  RefreshCw,
  Settings2,
  Trash2,
} from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  ConfigAPI,
  WOLAPI,
  type WOLDiscoveredDevice,
  type WOLDiscoveryPollEvent,
  type WOLDiscoveryProgress,
  type WOLDiscoveryResult,
  type WOLLocalRelay,
  type WOLLocalRelayInput,
  type WOLRelay,
  type WOLRelayCredentialResult,
  type WOLRelayInput,
  type WOLTarget,
  type WOLTargetInput,
} from "@/lib/api";
import { normalizeGatewayPortalConfig } from "@/lib/gatewayPortal";
import { useConfigStore } from "@/store/config";
import WOLBootstrapDialog from "./wol-management/WOLBootstrapDialog.vue";
import WOLDiscoveryDialog from "./wol-management/WOLDiscoveryDialog.vue";
import WOLLocalRelaySettings from "./wol-management/WOLLocalRelaySettings.vue";
import WOLRelayDialog from "./wol-management/WOLRelayDialog.vue";
import WOLTargetDialog from "./wol-management/WOLTargetDialog.vue";

const { t } = useI18n();
const configStore = useConfigStore();
const relays = ref<WOLRelay[]>([]);
const targets = ref<WOLTarget[]>([]);
const localRelay = ref<WOLLocalRelay | null>(null);
const loading = ref(true);
const loadError = ref("");
const saving = ref(false);
const relayDialogOpen = ref(false);
const targetDialogOpen = ref(false);
const bootstrapOpen = ref(false);
const bootstrapCredential = ref<WOLRelayCredentialResult | null>(null);
const relayMode = ref<"create" | "edit">("create");
const targetMode = ref<"create" | "edit">("create");
const editingRelayId = ref("");
const editingTargetId = ref("");
const probingRelayIds = ref(new Set<string>());
const wakingTargetIds = ref(new Set<string>());
const deletingRelayIds = ref(new Set<string>());
const deletingTargetIds = ref(new Set<string>());
const rotatingRelayIds = ref(new Set<string>());
const savingLocalRelay = ref(false);
const discoveryOpen = ref(false);
const discoveryResult = ref<WOLDiscoveryResult | null>(null);
const discoveryProgress = ref<WOLDiscoveryProgress | null>(null);
const discovering = ref(false);
const addingDiscovered = ref(false);
const settingsOpen = ref(false);
const showWolInPortal = ref(false);
const savingPortalSetting = ref(false);
let discoveryAbortController: AbortController | null = null;

const relayForm = reactive<WOLRelayInput>({
  name: "",
  address: "",
  port: 40009,
  enabled: true,
});
const targetForm = reactive<WOLTargetInput>({
  name: "",
  mac: "",
  note: "",
  relayId: null,
  broadcastAddress: null,
  ipAddress: null,
  enabled: true,
});
const localRelayForm = reactive<WOLLocalRelayInput>({
  enabled: false,
  relayId: "",
  keyVersion: 1,
  listenAddress: "0.0.0.0",
  port: 40009,
  broadcastDestinations: ["255.255.255.255:9"],
  allowedSources: [],
  psk: "",
});

const applyLocalRelay = (result: WOLLocalRelay) => {
  localRelay.value = result;
  Object.assign(localRelayForm, {
    enabled: result.config.enabled,
    relayId: result.config.relayId,
    keyVersion: result.config.keyVersion,
    listenAddress: result.config.listenAddress,
    port: result.config.port,
    broadcastDestinations: [...result.config.broadcastDestinations],
    allowedSources: [...result.config.allowedSources],
    psk: "",
  });
};

const refreshLocalRelayRuntime = async () => {
  for (const delay of [100, 250, 500]) {
    await new Promise<void>((resolve) => globalThis.setTimeout(resolve, delay));
    try {
      const result = await WOLAPI.getLocalRelay();
      applyLocalRelay(result);
      if (
        result.runtime.lastError ||
        result.runtime.active === result.config.enabled
      ) {
        break;
      }
    } catch {
      break;
    }
  }
};

const setPending = (
  target: typeof probingRelayIds,
  id: string,
  value: boolean,
) => {
  const next = new Set(target.value);
  if (value) next.add(id);
  else next.delete(id);
  target.value = next;
};

const statusLabel = (target: WOLTarget) =>
  t(`admin.wol.status.${target.status.state}`);

const checkedAtLabel = (target: WOLTarget) => {
  if (!target.status.checkedAt) return t("admin.wol.status.notChecked");
  return t("admin.wol.status.checkedAt", {
    time: new Date(target.status.checkedAt).toLocaleString(),
  });
};

const openSettings = async () => {
  try {
    const data = await ConfigAPI.getGatewaySettings();
    showWolInPortal.value = normalizeGatewayPortalConfig(data.portal).show_wol;
    settingsOpen.value = true;
  } catch (error) {
    toast.error(t("admin.wol.portal.loadFailed"), {
      description: extractErrorMessage(error, t("admin.wol.portal.loadFailed")),
    });
  }
};

const savePortalSetting = async () => {
  savingPortalSetting.value = true;
  try {
    const data = await ConfigAPI.updateGatewaySettings({
      portal: { show_wol: showWolInPortal.value },
    });
    showWolInPortal.value = normalizeGatewayPortalConfig(data.portal).show_wol;
    await configStore.loadConfig();
    settingsOpen.value = false;
    toast.success(t("admin.wol.portal.saved"));
  } catch (error) {
    toast.error(t("admin.wol.portal.saveFailed"), {
      description: extractErrorMessage(error, t("admin.wol.portal.saveFailed")),
    });
  } finally {
    savingPortalSetting.value = false;
  }
};

const load = async () => {
  loading.value = true;
  loadError.value = "";
  try {
    const [relayResult, targetResult, localRelayResult] = await Promise.all([
      WOLAPI.listRelays(),
      WOLAPI.listTargets(),
      WOLAPI.getLocalRelay(),
    ]);
    relays.value = relayResult.items;
    targets.value = targetResult.items;
    applyLocalRelay(localRelayResult);
  } catch (error) {
    loadError.value = extractErrorMessage(error, t("admin.wol.loadFailed"));
  } finally {
    loading.value = false;
  }
};

const saveLocalRelay = async () => {
  savingLocalRelay.value = true;
  try {
    const psk = localRelayForm.psk?.trim();
    const result = await WOLAPI.updateLocalRelay({
      ...localRelayForm,
      broadcastDestinations: [...localRelayForm.broadcastDestinations],
      allowedSources: [...localRelayForm.allowedSources],
      psk: psk || undefined,
    });
    applyLocalRelay(result);
    await refreshLocalRelayRuntime();
    toast.success(t("admin.wol.localRelay.saved"));
  } catch (error) {
    toast.error(t("admin.wol.localRelay.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.wol.localRelay.saveFailed"),
      ),
    });
  } finally {
    savingLocalRelay.value = false;
  }
};

const pairLocalRelay = async (pairingCode: string) => {
  savingLocalRelay.value = true;
  try {
    const result = await WOLAPI.pairLocalRelay(pairingCode);
    applyLocalRelay(result);
    await refreshLocalRelayRuntime();
    toast.success(t("admin.wol.localRelay.paired"));
  } catch (error) {
    toast.error(t("admin.wol.localRelay.pairFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.wol.localRelay.pairFailed"),
      ),
    });
  } finally {
    savingLocalRelay.value = false;
  }
};

const openCreateRelay = () => {
  relayMode.value = "create";
  editingRelayId.value = "";
  Object.assign(relayForm, {
    name: "",
    address: "",
    port: 40009,
    enabled: true,
  });
  relayDialogOpen.value = true;
};

const openEditRelay = (relay: WOLRelay) => {
  relayMode.value = "edit";
  editingRelayId.value = relay.id;
  Object.assign(relayForm, {
    name: relay.name,
    address: relay.address,
    port: relay.port,
    enabled: relay.enabled,
  });
  relayDialogOpen.value = true;
};

const saveRelay = async () => {
  saving.value = true;
  try {
    if (relayMode.value === "create") {
      const result = await WOLAPI.createRelay({ ...relayForm });
      bootstrapCredential.value = result;
      bootstrapOpen.value = true;
      toast.success(t("admin.wol.relayCreated"));
    } else {
      await WOLAPI.updateRelay(editingRelayId.value, { ...relayForm });
      toast.success(t("admin.wol.relayUpdated"));
    }
    relayDialogOpen.value = false;
    await load();
  } catch (error) {
    toast.error(t("admin.wol.saveFailed"), {
      description: extractErrorMessage(error, t("admin.wol.saveFailed")),
    });
  } finally {
    saving.value = false;
  }
};

const openCreateTarget = () => {
  targetMode.value = "create";
  editingTargetId.value = "";
  Object.assign(targetForm, {
    name: "",
    mac: "",
    note: "",
    relayId: null,
    broadcastAddress: null,
    ipAddress: null,
    enabled: true,
  });
  targetDialogOpen.value = true;
};

const openEditTarget = (target: WOLTarget) => {
  targetMode.value = "edit";
  editingTargetId.value = target.id;
  Object.assign(targetForm, {
    name: target.name,
    mac: target.mac,
    note: target.note,
    relayId: target.relayId,
    broadcastAddress: target.broadcastAddress,
    ipAddress: target.ipAddress,
    enabled: target.enabled,
  });
  targetDialogOpen.value = true;
};

const saveTarget = async () => {
  saving.value = true;
  try {
    if (targetMode.value === "create") {
      await WOLAPI.createTarget({ ...targetForm });
      toast.success(t("admin.wol.targetCreated"));
    } else {
      await WOLAPI.updateTarget(editingTargetId.value, { ...targetForm });
      toast.success(t("admin.wol.targetUpdated"));
    }
    targetDialogOpen.value = false;
    await load();
  } catch (error) {
    toast.error(t("admin.wol.saveFailed"), {
      description: extractErrorMessage(error, t("admin.wol.saveFailed")),
    });
  } finally {
    saving.value = false;
  }
};

const applyDiscoveryEvent = (event: WOLDiscoveryPollEvent) => {
  if (event.type === "meta") {
    discoveryProgress.value = event.data.progress;
    discoveryResult.value = {
      devices: [],
      networks: event.data.networks,
      durationMs: 0,
      method: "icmp-neighbor",
    };
    return;
  }
  if (event.type === "progress") {
    discoveryProgress.value = event.data;
    return;
  }
  if (event.type === "device") {
    if (!discoveryResult.value) return;
    const devices = discoveryResult.value.devices.filter(
      (device) => device.mac !== event.data.mac,
    );
    devices.push(event.data);
    devices.sort((left, right) =>
      left.ip.localeCompare(right.ip, undefined, { numeric: true }),
    );
    discoveryResult.value = { ...discoveryResult.value, devices };
    return;
  }
  if (event.type === "done") {
    discoveryResult.value = event.data;
  }
};

const discoverDevices = async (targetCidrs: string[] = []) => {
  discoveryAbortController?.abort();
  const abortController = new AbortController();
  discoveryAbortController = abortController;
  discovering.value = true;
  discoveryProgress.value = null;
  discoveryResult.value = null;
  try {
    discoveryResult.value = await WOLAPI.discoverLocalDevices(targetCidrs, {
      signal: abortController.signal,
      onEvent: applyDiscoveryEvent,
    });
  } catch (error) {
    if ((error as Error)?.name === "AbortError") return;
    toast.error(t("admin.wol.discovery.failed"), {
      description: extractErrorMessage(error, t("admin.wol.discovery.failed")),
    });
  } finally {
    if (discoveryAbortController === abortController) {
      discoveryAbortController = null;
      discovering.value = false;
    }
  }
};

const openDiscovery = async () => {
  discoveryOpen.value = true;
  await discoverDevices();
};

const setDiscoveryOpen = (open: boolean) => {
  discoveryOpen.value = open;
  if (!open) {
    discoveryAbortController?.abort();
    discoveryAbortController = null;
    discovering.value = false;
  }
};

const addDiscoveredDevices = async (
  devices: Array<WOLDiscoveredDevice & { name: string; note: string }>,
) => {
  addingDiscovered.value = true;
  let added = 0;
  try {
    for (const device of devices) {
      await WOLAPI.createTarget({
        name: device.name,
        mac: device.mac,
        note: device.note,
        relayId: null,
        broadcastAddress: device.broadcastAddress,
        ipAddress: device.ip,
        enabled: true,
      });
      added += 1;
    }
    toast.success(t("admin.wol.discovery.addedCount", { count: added }));
    discoveryOpen.value = false;
    await load();
  } catch (error) {
    toast.error(t("admin.wol.discovery.addFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.wol.discovery.addFailed"),
      ),
    });
    if (added) await load();
  } finally {
    addingDiscovered.value = false;
  }
};

const wakeTarget = async (target: WOLTarget) => {
  setPending(wakingTargetIds, target.id, true);
  try {
    const result = await WOLAPI.wakeTarget(target.id);
    const local = result.deliveryMode === "local";
    toast.success(
      t(local ? "admin.wol.localWakeSent" : "admin.wol.wakeAccepted"),
      {
        description: t(
          local
            ? "admin.wol.localWakeSentDescription"
            : "admin.wol.wakeAcceptedDescription",
          { latency: result.latencyMs },
        ),
      },
    );
  } catch (error) {
    const status = (error as { response?: { status?: number } })?.response
      ?.status;
    const description = extractErrorMessage(error, t("admin.wol.wakeFailed"));
    if (status === 504) {
      toast.warning(t("admin.wol.wakeUnknown"), { description });
    } else {
      toast.error(t("admin.wol.wakeFailed"), { description });
    }
  } finally {
    setPending(wakingTargetIds, target.id, false);
  }
};

const probeRelay = async (relay: WOLRelay) => {
  setPending(probingRelayIds, relay.id, true);
  try {
    const result = await WOLAPI.probeRelay(relay.id);
    toast.success(t("admin.wol.probeSuccess"), {
      description: t("admin.wol.probeSuccessDescription", {
        latency: result.latencyMs,
      }),
    });
  } catch (error) {
    toast.error(t("admin.wol.probeFailed"), {
      description: extractErrorMessage(error, t("admin.wol.probeFailed")),
    });
  } finally {
    setPending(probingRelayIds, relay.id, false);
  }
};

const rotateRelay = async (relay: WOLRelay) => {
  setPending(rotatingRelayIds, relay.id, true);
  try {
    const result = await WOLAPI.rotateRelayPsk(relay.id);
    bootstrapCredential.value = result;
    bootstrapOpen.value = true;
    toast.success(t("admin.wol.pskRotated"));
    await load();
  } catch (error) {
    toast.error(t("admin.wol.rotateFailed"), {
      description: extractErrorMessage(error, t("admin.wol.rotateFailed")),
    });
  } finally {
    setPending(rotatingRelayIds, relay.id, false);
  }
};

const deleteRelay = async (relay: WOLRelay) => {
  setPending(deletingRelayIds, relay.id, true);
  try {
    await WOLAPI.deleteRelay(relay.id);
    toast.success(t("admin.wol.relayDeleted"));
    await load();
  } catch (error) {
    toast.error(t("admin.wol.deleteFailed"), {
      description: extractErrorMessage(error, t("admin.wol.deleteFailed")),
    });
  } finally {
    setPending(deletingRelayIds, relay.id, false);
  }
};

const deleteTarget = async (target: WOLTarget) => {
  setPending(deletingTargetIds, target.id, true);
  try {
    await WOLAPI.deleteTarget(target.id);
    toast.success(t("admin.wol.targetDeleted"));
    await load();
  } catch (error) {
    toast.error(t("admin.wol.deleteFailed"), {
      description: extractErrorMessage(error, t("admin.wol.deleteFailed")),
    });
  } finally {
    setPending(deletingTargetIds, target.id, false);
  }
};

const closeBootstrap = (open: boolean) => {
  bootstrapOpen.value = open;
  if (!open) bootstrapCredential.value = null;
};

const copyBootstrap = async (value: string) => {
  try {
    await navigator.clipboard.writeText(value);
    toast.success(t("admin.wol.bootstrap.codeCopied"));
  } catch {
    toast.error(t("admin.wol.copyFailed"));
  }
};

onMounted(load);
onBeforeUnmount(() => discoveryAbortController?.abort());
</script>

<template>
  <div
    class="dynamic-white-page-card dynamic-white-settings-surface flex h-full flex-col gap-4"
  >
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="space-y-1">
        <h1 class="text-xl font-semibold tracking-tight">
          {{ t("admin.wol.title") }}
        </h1>
        <p class="text-sm leading-6 text-muted-foreground">
          {{ t("admin.wol.description") }}
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Button
          variant="outline"
          size="icon"
          :aria-label="t('admin.wol.portal.settings')"
          @click="openSettings"
        >
          <Settings2 class="h-4 w-4" />
        </Button>
        <Button variant="outline" :disabled="loading" @click="load">
          <RefreshCw :class="['mr-2 h-4 w-4', loading && 'animate-spin']" />
          {{ t("admin.wol.refresh") }}
        </Button>
      </div>
    </div>

    <div
      v-if="loading"
      class="flex flex-1 items-center justify-center py-16 text-sm text-muted-foreground"
    >
      <Loader2 class="mr-2 h-5 w-5 animate-spin" />{{ t("admin.wol.loading") }}
    </div>
    <div
      v-else-if="loadError"
      class="rounded-xl border border-destructive/40 bg-destructive/5 p-5"
    >
      <p class="text-sm text-destructive">{{ loadError }}</p>
      <Button class="mt-3" size="sm" variant="outline" @click="load">{{
        t("admin.wol.retry")
      }}</Button>
    </div>

    <Tabs v-else default-value="targets" class="flex min-h-0 flex-1 flex-col">
      <TabsList class="w-fit">
        <TabsTrigger value="targets">
          <Power class="mr-1.5 h-4 w-4" />{{ t("admin.wol.tabs.targets") }}
        </TabsTrigger>
        <TabsTrigger value="relays">
          <RadioTower class="mr-1.5 h-4 w-4" />{{ t("admin.wol.tabs.relays") }}
        </TabsTrigger>
      </TabsList>

      <TabsContent value="targets" class="space-y-4 pt-2">
        <div class="flex items-center justify-between gap-3">
          <p class="text-sm text-muted-foreground">
            {{ t("admin.wol.targetsDescription") }}
          </p>
          <div class="flex items-center justify-end">
            <Button size="sm" class="rounded-r-none" @click="openDiscovery">
              <Radar class="mr-1.5 h-4 w-4" />{{
                t("admin.wol.discoverDevices")
              }}
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <Button
                  data-testid="wol-device-actions-menu-trigger"
                  size="sm"
                  class="w-8 rounded-l-none border-l border-primary-foreground/20 px-0"
                  :aria-label="t('common.moreActions')"
                >
                  <ChevronDown class="h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem @select="openCreateTarget">
                  <Plus class="mr-2 h-4 w-4" />
                  {{ t("admin.wol.addTarget") }}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
        <div
          v-if="!targets.length"
          class="rounded-xl border border-dashed px-5 py-12 text-center text-sm text-muted-foreground"
        >
          {{ t("admin.wol.noTargets") }}
        </div>
        <div v-else class="grid gap-3 xl:grid-cols-2">
          <Card
            v-for="target in targets"
            :key="target.id"
            class="gap-0 overflow-hidden"
          >
            <CardHeader class="pb-4">
              <div class="flex items-start justify-between gap-4">
                <div data-testid="wol-target-primary" class="min-w-0 flex-1">
                  <div class="flex items-center gap-2">
                    <span
                      class="h-2.5 w-2.5 shrink-0 rounded-full"
                      :class="
                        target.status.state === 'online'
                          ? 'bg-emerald-500'
                          : target.status.state === 'offline'
                            ? 'bg-zinc-400'
                            : 'bg-amber-400'
                      "
                      aria-hidden="true"
                    />
                    <CardTitle class="break-words text-lg leading-6">
                      {{ target.name }}
                    </CardTitle>
                    <span class="sr-only">{{ statusLabel(target) }}</span>
                  </div>
                  <p
                    v-if="target.note"
                    class="mt-2 whitespace-pre-wrap break-words text-sm leading-6 text-foreground/80"
                  >
                    {{ target.note }}
                  </p>
                </div>
                <Badge
                  class="shrink-0"
                  :variant="target.enabled ? 'default' : 'secondary'"
                >
                  {{
                    target.enabled
                      ? t("admin.wol.active")
                      : t("admin.wol.disabled")
                  }}
                </Badge>
              </div>
            </CardHeader>
            <CardContent class="space-y-4">
              <div
                data-testid="wol-target-technical"
                class="grid gap-2 sm:grid-cols-2"
              >
                <div class="rounded-lg bg-muted/40 px-3 py-2.5">
                  <p class="text-xs text-muted-foreground">
                    {{ t("admin.wol.mac") }}
                  </p>
                  <p class="mt-1 break-all font-mono text-sm">
                    {{ target.mac }}
                  </p>
                </div>
                <div class="rounded-lg bg-muted/40 px-3 py-2.5 sm:col-span-2">
                  <p class="text-xs text-muted-foreground">
                    {{ t("admin.wol.status.label") }}
                  </p>
                  <div
                    class="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm"
                  >
                    <span>{{ statusLabel(target) }}</span>
                    <span class="text-xs text-muted-foreground">{{
                      checkedAtLabel(target)
                    }}</span>
                    <span
                      v-if="target.status.observedIp || target.ipAddress"
                      class="font-mono text-xs"
                    >
                      {{ target.status.observedIp || target.ipAddress }}
                    </span>
                  </div>
                </div>
                <div class="rounded-lg bg-muted/40 px-3 py-2.5">
                  <p class="text-xs text-muted-foreground">
                    {{ t("admin.wol.deliveryPath") }}
                  </p>
                  <div class="mt-1 text-sm">
                    <template v-if="target.deliveryMode === 'local'">
                      <p>{{ t("admin.wol.localDelivery") }}</p>
                      <p
                        v-if="target.broadcastAddress"
                        class="mt-0.5 break-all font-mono text-xs text-muted-foreground"
                      >
                        {{ target.broadcastAddress }}:9
                      </p>
                    </template>
                    <p v-else-if="target.relay">{{ target.relay.name }}</p>
                    <p v-else class="text-destructive">
                      {{ t("admin.wol.relayMissing") }}
                    </p>
                  </div>
                </div>
              </div>
              <div
                class="flex flex-wrap items-center justify-between gap-2 border-t pt-3"
              >
                <div class="flex flex-wrap gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    @click="openEditTarget(target)"
                    ><Pencil class="mr-1.5 h-3.5 w-3.5" />{{
                      t("admin.wol.edit")
                    }}</Button
                  >
                  <ConfirmDangerPopover
                    :title="t('admin.wol.deleteTargetTitle')"
                    :description="t('admin.wol.deleteTargetDescription')"
                    :loading="deletingTargetIds.has(target.id)"
                    :on-confirm="() => deleteTarget(target)"
                  >
                    <template #trigger
                      ><Button
                        variant="outline"
                        size="sm"
                        :aria-label="t('admin.wol.deleteTargetTitle')"
                        ><Trash2 class="h-3.5 w-3.5 text-destructive" /></Button
                    ></template>
                  </ConfirmDangerPopover>
                </div>
                <Button
                  size="sm"
                  :disabled="
                    !target.enabled ||
                    (target.deliveryMode === 'relay' &&
                      !target.relay?.enabled) ||
                    wakingTargetIds.has(target.id)
                  "
                  @click="wakeTarget(target)"
                >
                  <Loader2
                    v-if="wakingTargetIds.has(target.id)"
                    class="mr-1.5 h-3.5 w-3.5 animate-spin"
                  />
                  <MonitorUp v-else class="mr-1.5 h-3.5 w-3.5" />{{
                    t("admin.wol.wake")
                  }}
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      </TabsContent>

      <TabsContent value="relays" class="space-y-4 pt-2">
        <div class="flex items-center justify-between gap-3">
          <p class="text-sm text-muted-foreground">
            {{ t("admin.wol.relaysDescription") }}
          </p>
          <Button size="sm" @click="openCreateRelay"
            ><Plus class="mr-1.5 h-4 w-4" />{{
              t("admin.wol.addRelay")
            }}</Button
          >
        </div>
        <div
          v-if="!relays.length"
          class="rounded-xl border border-dashed px-5 py-12 text-center text-sm text-muted-foreground"
        >
          {{ t("admin.wol.noRelays") }}
        </div>
        <div v-else class="grid gap-3 xl:grid-cols-2">
          <Card v-for="relay in relays" :key="relay.id" class="gap-3">
            <CardHeader class="pb-0">
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <CardTitle class="truncate text-base">{{
                    relay.name
                  }}</CardTitle>
                  <p class="mt-1 font-mono text-xs text-muted-foreground">
                    {{ relay.address
                    }}<span v-if="relay.port !== 40009">:{{ relay.port }}</span>
                  </p>
                </div>
                <Badge :variant="relay.enabled ? 'default' : 'secondary'">{{
                  relay.enabled
                    ? t("admin.wol.active")
                    : t("admin.wol.disabled")
                }}</Badge>
              </div>
            </CardHeader>
            <CardContent class="space-y-4">
              <Badge
                class="w-fit"
                :variant="relay.pskConfigured ? 'outline' : 'secondary'"
              >
                <Link2 class="mr-1 h-3 w-3" />{{
                  relay.pskConfigured
                    ? t("admin.wol.relayPaired")
                    : t("admin.wol.relayWaitingForPairing")
                }}
              </Badge>
              <div class="flex flex-wrap justify-end gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  @click="openEditRelay(relay)"
                  ><Pencil class="mr-1.5 h-3.5 w-3.5" />{{
                    t("admin.wol.edit")
                  }}</Button
                >
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="
                    !relay.enabled ||
                    !relay.pskConfigured ||
                    probingRelayIds.has(relay.id)
                  "
                  @click="probeRelay(relay)"
                >
                  <Loader2
                    v-if="probingRelayIds.has(relay.id)"
                    class="mr-1.5 h-3.5 w-3.5 animate-spin"
                  /><Cable v-else class="mr-1.5 h-3.5 w-3.5" />{{
                    t("admin.wol.probe")
                  }}
                </Button>
                <ConfirmDangerPopover
                  :title="t('admin.wol.rotateTitle')"
                  :description="t('admin.wol.rotateDescription')"
                  :confirm-text="t('admin.wol.repair')"
                  :loading="rotatingRelayIds.has(relay.id)"
                  :on-confirm="() => rotateRelay(relay)"
                >
                  <template #trigger
                    ><Button variant="outline" size="sm"
                      ><Link2 class="mr-1.5 h-3.5 w-3.5" />{{
                        t("admin.wol.repair")
                      }}</Button
                    ></template
                  >
                </ConfirmDangerPopover>
                <ConfirmDangerPopover
                  :title="t('admin.wol.deleteRelayTitle')"
                  :description="t('admin.wol.deleteRelayDescription')"
                  :loading="deletingRelayIds.has(relay.id)"
                  :on-confirm="() => deleteRelay(relay)"
                >
                  <template #trigger
                    ><Button
                      variant="outline"
                      size="sm"
                      :aria-label="t('admin.wol.deleteRelayTitle')"
                      ><Trash2 class="h-3.5 w-3.5 text-destructive" /></Button
                  ></template>
                </ConfirmDangerPopover>
              </div>
            </CardContent>
          </Card>
        </div>
        <div class="border-t pt-5">
          <WOLLocalRelaySettings
            :model="localRelayForm"
            :psk-configured="localRelay?.config.pskConfigured ?? false"
            :runtime="localRelay?.runtime ?? null"
            :saving="savingLocalRelay"
            @pair="pairLocalRelay"
            @save="saveLocalRelay"
          />
        </div>
      </TabsContent>
    </Tabs>

    <WOLRelayDialog
      v-model:open="relayDialogOpen"
      :mode="relayMode"
      :model="relayForm"
      :saving="saving"
      @confirm="saveRelay"
    />
    <WOLDiscoveryDialog
      :open="discoveryOpen"
      :result="discoveryResult"
      :progress="discoveryProgress"
      :existing-macs="
        targets
          .filter((target) => target.deliveryMode === 'local')
          .map((target) => target.mac)
      "
      :scanning="discovering"
      :adding="addingDiscovered"
      @update:open="setDiscoveryOpen"
      @scan="discoverDevices"
      @add="addDiscoveredDevices"
    />
    <WOLTargetDialog
      v-model:open="targetDialogOpen"
      :mode="targetMode"
      :model="targetForm"
      :relays="relays"
      :saving="saving"
      @confirm="saveTarget"
    />
    <WOLBootstrapDialog
      :open="bootstrapOpen"
      :credential="bootstrapCredential"
      @update:open="closeBootstrap"
      @copy="copyBootstrap"
    />
    <Dialog v-model:open="settingsOpen">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t("admin.wol.portal.title") }}</DialogTitle>
          <DialogDescription>{{
            t("admin.wol.portal.description")
          }}</DialogDescription>
        </DialogHeader>
        <div
          class="flex items-center justify-between gap-4 rounded-lg border p-4"
        >
          <Label for="wol-portal-shortcut" class="leading-6">
            {{ t("admin.wol.portal.showShortcut") }}
          </Label>
          <Switch id="wol-portal-shortcut" v-model="showWolInPortal" />
        </div>
        <DialogFooter>
          <Button variant="outline" @click="settingsOpen = false">{{
            t("common.cancel")
          }}</Button>
          <Button :disabled="savingPortalSetting" @click="savePortalSetting">
            {{ savingPortalSetting ? t("admin.wol.saving") : t("common.save") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
