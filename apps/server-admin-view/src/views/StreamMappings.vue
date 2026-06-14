<template>
  <div class="space-y-6">
    <Card>
      <CardHeader>
        <CardTitle
          class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
        >
          <span>{{ t("admin.streamMappings.title") }}</span>
          <div class="flex flex-wrap items-center gap-2">
            <div class="flex">
              <Button class="rounded-r-none" @click="openCreateDialog">
                <Plus class="mr-2 h-4 w-4" />
                {{ t("admin.streamMappings.addMapping") }}
              </Button>
              <DropdownMenu>
                <DropdownMenuTrigger as-child>
                  <Button
                    variant="default"
                    size="icon"
                    class="rounded-l-none border-l border-primary-foreground/20 px-2"
                  >
                    <ChevronDown class="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem @click="syncRoutes" :disabled="isSyncing">
                    <RefreshCw
                      class="mr-2 h-4 w-4"
                      :class="{ 'animate-spin': isSyncing }"
                    />
                    {{
                      isSyncing
                        ? t("admin.streamMappings.syncing")
                        : t("admin.streamMappings.syncGateway")
                    }}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </CardTitle>
        <CardDescription>
          {{ t("admin.streamMappings.description") }}
        </CardDescription>
      </CardHeader>

      <CardContent class="space-y-4">
        <Alert
          class="items-start rounded-xl border-zinc-200 bg-zinc-50/70 text-zinc-900 shadow-none"
        >
          <Info class="mt-0.5 h-4 w-4 shrink-0" />
          <div class="space-y-1">
            <AlertTitle>{{ t("admin.streamMappings.accessTitle") }}</AlertTitle>
            <AlertDescription class="text-sm leading-6 text-zinc-700">
              {{ t("admin.streamMappings.accessDescription") }}
            </AlertDescription>
          </div>
        </Alert>

        <div
          class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
        >
          <SearchInput
            v-model="searchQuery"
            :placeholder="t('admin.streamMappings.searchPlaceholder')"
            class="max-w-xs"
          />
        </div>

        <div class="overflow-hidden rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{{ t("admin.streamMappings.protocol") }}</TableHead>
                <TableHead>{{
                  t("admin.streamMappings.listenPort")
                }}</TableHead>
                <TableHead>{{ t("admin.streamMappings.target") }}</TableHead>
                <TableHead>{{
                  t("admin.streamMappings.authStatus")
                }}</TableHead>
                <TableHead class="text-right">{{
                  t("admin.sessions.table.actions")
                }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="filteredMappings.length === 0">
                <TableCell
                  colspan="5"
                  class="py-8 text-center text-muted-foreground"
                >
                  {{ t("admin.streamMappings.empty") }}
                </TableCell>
              </TableRow>
              <TableRow
                v-for="mapping in filteredMappings"
                :key="getMappingKey(mapping)"
                class="group"
              >
                <TableCell>
                  <Badge
                    variant="outline"
                    class="font-mono uppercase tracking-[0.16em]"
                  >
                    {{ mapping.protocol }}
                  </Badge>
                </TableCell>
                <TableCell class="font-medium">
                  <div
                    class="inline-flex items-center gap-2 rounded-full border px-3 py-1 text-sm"
                  >
                    <span>{{ mapping.listen_port }}</span>
                  </div>
                </TableCell>
                <TableCell class="font-mono text-sm">{{
                  mapping.target
                }}</TableCell>
                <TableCell class="min-w-[15rem]">
                  <div
                    class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground"
                  >
                    <Badge v-if="mapping.use_auth" variant="default">
                      {{ t("admin.streamMappings.authRequired") }}
                    </Badge>
                    <Badge v-else variant="secondary">{{
                      t("admin.streamMappings.publicAccess")
                    }}</Badge>
                  </div>
                </TableCell>
                <TableCell class="text-right">
                  <div class="flex justify-end gap-2">
                    <Button
                      variant="ghost"
                      size="sm"
                      @click="openEditDialog(mapping)"
                    >
                      {{ t("admin.streamMappings.edit") }}
                    </Button>
                    <ConfirmDangerPopover
                      :title="
                        t('admin.streamMappings.deleteTitle', {
                          protocol: formatProtocolLabel(mapping.protocol),
                        })
                      "
                      :description="
                        t('admin.streamMappings.deleteDescription', {
                          mapping: formatMappingLabel(mapping),
                          target: mapping.target,
                        })
                      "
                      :loading="removingMappingKey === getMappingKey(mapping)"
                      :disabled="removingMappingKey === getMappingKey(mapping)"
                      :on-confirm="() => removeMapping(mapping)"
                      content-class="w-72 text-left"
                    >
                      <template #trigger>
                        <Button
                          variant="ghost"
                          size="sm"
                          class="text-destructive hover:bg-destructive/10 hover:text-destructive"
                          :disabled="
                            removingMappingKey === getMappingKey(mapping)
                          "
                        >
                          {{ t("admin.streamMappings.delete") }}
                        </Button>
                      </template>
                    </ConfirmDangerPopover>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </CardContent>
    </Card>

    <Dialog :open="isDialogOpen" @update:open="handleDialogOpenChange">
      <DialogContent class="sm:max-w-[520px]">
        <DialogHeader>
          <DialogTitle>
            {{
              isEditing
                ? t("admin.streamMappings.editTitle")
                : t("admin.streamMappings.createTitle")
            }}
          </DialogTitle>
          <DialogDescription>
            {{ t("admin.streamMappings.dialogDescription") }}
          </DialogDescription>
        </DialogHeader>

        <div class="grid gap-4 py-4">
          <div class="space-y-2">
            <Label for="stream-protocol">{{
              t("admin.streamMappings.transportProtocol")
            }}</Label>
            <StreamProtocolMultiSelect
              id="stream-protocol"
              v-model="form.protocols"
            />
            <p class="text-xs text-muted-foreground">
              {{ t("admin.streamMappings.protocolHint") }}
            </p>
          </div>

          <div class="space-y-2">
            <Label for="stream-listen-port">{{
              t("admin.streamMappings.listenPort")
            }}</Label>
            <Input
              id="stream-listen-port"
              v-model="form.listen_port"
              inputmode="numeric"
              :placeholder="t('admin.streamMappings.listenPortPlaceholder')"
              @blur="markPortBlurred"
            />
            <p class="text-xs text-muted-foreground">
              {{ t("admin.streamMappings.listenPortHint") }}
            </p>
          </div>

          <div class="space-y-2">
            <Label for="stream-target">{{
              t("admin.streamMappings.target")
            }}</Label>
            <Input
              id="stream-target"
              v-model="form.target"
              :placeholder="t('admin.streamMappings.targetPlaceholder')"
              @blur="markTargetBlurred"
            />
            <p class="text-xs text-muted-foreground">
              {{ t("admin.streamMappings.targetHint") }}
            </p>
          </div>

          <div
            class="flex items-center justify-between rounded-lg border px-4 py-3"
          >
            <div class="space-y-1">
              <Label for="stream-auth">{{
                t("admin.streamMappings.authRequiredLabel")
              }}</Label>
              <p class="text-xs text-muted-foreground">
                {{ t("admin.streamMappings.authRequiredHint") }}
              </p>
            </div>
            <Switch id="stream-auth" v-model="form.use_auth" />
          </div>

          <div
            v-if="showValidation && validationMessage"
            class="rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
          >
            {{ validationMessage }}
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" @click="closeDialog">{{
            t("common.cancel")
          }}</Button>
          <Button :disabled="isSaving" @click="saveMapping">
            <span
              v-if="isSaving"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{ t("admin.streamMappings.saveMapping") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, Info, Plus, RefreshCw } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { isValidHostPort } from "@admin-shared/utils/parseHostPort";
import { toast } from "@admin-shared/utils/toast";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ConfigAPI } from "../lib/api";
import { useConfigStore } from "../store/config";
import type { StreamMapping, StreamMappingProtocol } from "../types";
import StreamProtocolMultiSelect from "../components/StreamProtocolMultiSelect.vue";

const configStore = useConfigStore();
const { t, locale } = useI18n();
const DEFAULT_STREAM_PROTOCOL: StreamMappingProtocol = "tcp";
const STREAM_PROTOCOLS: StreamMappingProtocol[] = ["tcp", "udp"];

const searchQuery = ref("");
const isDialogOpen = ref(false);
const isSaving = ref(false);
const isSyncing = ref(false);
const editingMappingKey = ref<string | null>(null);
const removingMappingKey = ref<string | null>(null);
const hasAttemptedSubmit = ref(false);
const hasPortBlurred = ref(false);
const hasTargetBlurred = ref(false);

const form = reactive<{
  protocols: StreamMappingProtocol[];
  listen_port: string;
  target: string;
  use_auth: boolean;
}>({
  protocols: [DEFAULT_STREAM_PROTOCOL],
  listen_port: "",
  target: "",
  use_auth: true,
});

const allMappings = computed(() =>
  [...(configStore.config?.stream_mappings ?? [])]
    .map(normalizeStreamMapping)
    .sort(compareStreamMappings),
);

const filteredMappings = computed(() => {
  const query = searchQuery.value.trim().toLowerCase();
  if (!query) return allMappings.value;

  return allMappings.value.filter((mapping) => {
    const authStatus = mapping.use_auth
      ? t("admin.streamMappings.authRequired")
      : t("admin.streamMappings.publicAccess");
    return (
      mapping.protocol.includes(query) ||
      formatProtocolLabel(mapping.protocol).toLowerCase().includes(query) ||
      String(mapping.listen_port).includes(query) ||
      mapping.target.toLowerCase().includes(query) ||
      authStatus.includes(query)
    );
  });
});

const isEditing = computed(() => editingMappingKey.value !== null);
const parsedListenPort = computed(() => {
  const value = Number.parseInt(form.listen_port.trim(), 10);
  if (!Number.isFinite(value)) return null;
  return value;
});
const selectedProtocols = computed(() =>
  normalizeProtocolSelection(form.protocols),
);

const duplicateProtocols = computed(() => {
  const port = parsedListenPort.value;
  if (port === null) return [];

  return selectedProtocols.value.filter((protocol) =>
    allMappings.value.some(
      (mapping) =>
        getMappingKey(mapping) === createMappingKey(protocol, port) &&
        getMappingKey(mapping) !== editingMappingKey.value,
    ),
  );
});

const isTargetValid = computed(() => isValidStreamTarget(form.target));

function getPortValidationMessage(showRequired: boolean): string {
  const rawPort = form.listen_port.trim();
  if (!rawPort) {
    return showRequired ? t("admin.streamMappings.portRequired") : "";
  }

  const port = parsedListenPort.value;
  if (port === null) return t("admin.streamMappings.portInteger");
  if (port <= 0 || port > 65535) {
    return t("admin.streamMappings.portRange");
  }
  if (duplicateProtocols.value.length > 0) {
    return t("admin.streamMappings.duplicatePort", {
      protocols: formatProtocolList(duplicateProtocols.value),
      port,
    });
  }
  return "";
}

function getTargetValidationMessage(showRequired: boolean): string {
  const rawTarget = form.target.trim();
  if (!rawTarget) {
    return showRequired ? t("admin.streamMappings.targetRequired") : "";
  }
  if (!isTargetValid.value) {
    return t("admin.streamMappings.targetInvalid");
  }
  return "";
}

const validationMessage = computed(() => {
  if (hasAttemptedSubmit.value) {
    const submitMessage = submitValidationMessage.value;
    if (submitMessage) return submitMessage;
    return "";
  }

  const shouldValidatePort =
    hasPortBlurred.value && form.listen_port.trim() !== "";
  if (shouldValidatePort) {
    const portMessage = getPortValidationMessage(false);
    if (portMessage) return portMessage;
  }

  const shouldValidateTarget =
    hasTargetBlurred.value && form.target.trim() !== "";
  if (shouldValidateTarget) {
    const targetMessage = getTargetValidationMessage(false);
    if (targetMessage) return targetMessage;
  }

  return "";
});

const showValidation = computed(() => Boolean(validationMessage.value));
const submitValidationMessage = computed(() => {
  if (selectedProtocols.value.length === 0) {
    return t("admin.streamMappings.protocolRequired");
  }
  const portMessage = getPortValidationMessage(true);
  if (portMessage) return portMessage;
  return getTargetValidationMessage(true);
});

function isValidStreamTarget(target: string): boolean {
  return isValidHostPort(target);
}

function normalizeProtocol(
  protocol?: StreamMappingProtocol | string | null,
): StreamMappingProtocol {
  return protocol === "udp" ? "udp" : DEFAULT_STREAM_PROTOCOL;
}

function normalizeProtocolSelection(
  protocols: StreamMappingProtocol[] | undefined,
): StreamMappingProtocol[] {
  const selected = new Set(
    (protocols ?? []).map((protocol) => normalizeProtocol(protocol)),
  );
  const normalized = STREAM_PROTOCOLS.filter((protocol) =>
    selected.has(protocol),
  );
  return normalized.length > 0 ? normalized : [DEFAULT_STREAM_PROTOCOL];
}

function normalizeStreamMapping(mapping: StreamMapping): StreamMapping {
  return {
    ...mapping,
    protocol: normalizeProtocol(mapping.protocol),
  };
}

function createMappingKey(
  protocol: StreamMappingProtocol,
  listenPort: number,
): string {
  return `${protocol}:${listenPort}`;
}

function getMappingKey(mapping: StreamMapping): string {
  return createMappingKey(
    normalizeProtocol(mapping.protocol),
    mapping.listen_port,
  );
}

function compareStreamMappings(a: StreamMapping, b: StreamMapping): number {
  if (a.listen_port !== b.listen_port) {
    return a.listen_port - b.listen_port;
  }
  return a.protocol.localeCompare(b.protocol);
}

function formatProtocolLabel(protocol: StreamMappingProtocol): string {
  return protocol.toUpperCase();
}

function formatProtocolList(protocols: StreamMappingProtocol[]): string {
  const separator = String(locale.value).startsWith("en") ? ", " : "、";
  return protocols.map(formatProtocolLabel).join(separator);
}

function formatMappingLabel(mapping: StreamMapping): string {
  return `${formatProtocolLabel(normalizeProtocol(mapping.protocol))}/${mapping.listen_port}`;
}

function resetForm() {
  form.protocols = [DEFAULT_STREAM_PROTOCOL];
  form.listen_port = "";
  form.target = "";
  form.use_auth = true;
  editingMappingKey.value = null;
  hasAttemptedSubmit.value = false;
  hasPortBlurred.value = false;
  hasTargetBlurred.value = false;
}

function handleDialogOpenChange(nextOpen: boolean) {
  if (!nextOpen) {
    closeDialog();
  }
}

function openCreateDialog() {
  resetForm();
  isDialogOpen.value = true;
}

function openEditDialog(mapping: StreamMapping) {
  const normalized = normalizeStreamMapping(mapping);
  form.protocols = [normalized.protocol];
  form.listen_port = String(mapping.listen_port);
  form.target = mapping.target;
  form.use_auth = mapping.use_auth;
  editingMappingKey.value = getMappingKey(normalized);
  isDialogOpen.value = true;
}

function closeDialog() {
  isDialogOpen.value = false;
  resetForm();
}

function markPortBlurred() {
  hasPortBlurred.value = true;
}

function markTargetBlurred() {
  hasTargetBlurred.value = true;
}

async function saveMapping() {
  hasAttemptedSubmit.value = true;
  if (submitValidationMessage.value || parsedListenPort.value === null) return;

  const nextMappings: StreamMapping[] = selectedProtocols.value.map(
    (protocol) => ({
      protocol,
      listen_port: parsedListenPort.value!,
      target: form.target.trim(),
      use_auth: form.use_auth,
    }),
  );

  isSaving.value = true;
  try {
    const next = [...allMappings.value];
    const existingIndex = next.findIndex(
      (mapping) => getMappingKey(mapping) === editingMappingKey.value,
    );

    if (existingIndex >= 0) {
      next.splice(existingIndex, 1, ...nextMappings);
    } else {
      next.push(...nextMappings);
    }

    await configStore.saveStreamMappings(next);
    toast.success(getSaveSuccessMessage(nextMappings.length));
    closeDialog();
  } catch (error: any) {
    toast.error(t("admin.streamMappings.saveFailed"), {
      description: extractErrorMessage(error, t("common.tryLater")),
    });
  } finally {
    isSaving.value = false;
  }
}

function getSaveSuccessMessage(savedCount: number): string {
  const action = isEditing.value
    ? t("admin.streamMappings.actionUpdate")
    : t("admin.streamMappings.actionCreate");
  return savedCount > 1
    ? t("admin.streamMappings.saveMany", { action, count: savedCount })
    : t("admin.streamMappings.saveOne", { action });
}

async function removeMapping(mapping: StreamMapping) {
  removingMappingKey.value = getMappingKey(mapping);
  try {
    await configStore.saveStreamMappings(
      allMappings.value.filter(
        (item) => getMappingKey(item) !== getMappingKey(mapping),
      ),
    );
    toast.success(
      t("admin.streamMappings.removeSuccess", {
        mapping: formatMappingLabel(mapping),
      }),
    );
  } catch (error: any) {
    toast.error(t("admin.streamMappings.deleteFailed"), {
      description: extractErrorMessage(error, t("common.tryLater")),
    });
  } finally {
    removingMappingKey.value = null;
  }
}

async function syncRoutes() {
  isSyncing.value = true;
  try {
    const result = await ConfigAPI.syncRoutes();
    if (result.success) {
      toast.success(t("admin.streamMappings.syncSuccess"), {
        description: t("admin.streamMappings.syncDescription", {
          pathRules: result.data?.synced_rules ?? 0,
          hostRules: result.data?.synced_host_rules ?? 0,
          streamRules: result.data?.synced_stream_rules ?? 0,
        }),
      });
      return;
    }

    toast.error(t("admin.streamMappings.syncFailed"), {
      description: result.message || t("admin.streamMappings.syncNoSuccess"),
    });
  } catch (error: any) {
    toast.error(t("admin.streamMappings.syncFailed"), {
      description: extractErrorMessage(error, t("common.tryLater")),
    });
  } finally {
    isSyncing.value = false;
  }
}
</script>
