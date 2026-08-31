<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import {
  CheckCircle2,
  KeyRound,
  LoaderCircle,
  PlugZap,
  ShieldCheck,
  Trash2,
  TriangleAlert,
} from "lucide-vue-next";
import type {
  TerminalAuthMethod,
  TerminalTargetRecord,
} from "@/lib/api/terminal";
import type { useTerminalTargetEditor } from "./useTerminalTargetEditor";

const props = defineProps<{
  activeSessionCount: number;
  editor: ReturnType<typeof useTerminalTargetEditor>;
  onDelete: (target: TerminalTargetRecord) => void | Promise<void>;
}>();

const { t } = useI18n();
const terminateActiveSessions = ref(false);
const requiresTerminationConfirmation = computed(
  () =>
    props.editor.forceConfirmationRequired.value &&
    props.editor.requiresSessionTermination.value,
);
const displayedActiveSessionCount = computed(
  () =>
    props.editor.conflictingActiveSessionCount.value ??
    props.activeSessionCount,
);
const showsActiveSessionWarning = computed(
  () =>
    displayedActiveSessionCount.value > 0 &&
    props.editor.requiresSessionTermination.value &&
    !requiresTerminationConfirmation.value,
);

watch(
  () => props.editor.open.value,
  (open) => {
    if (open) terminateActiveSessions.value = false;
  },
);

const updateAuthMethod = (value: unknown) => {
  if (value === "password" || value === "privateKey") {
    props.editor.setAuthMethod(value satisfies TerminalAuthMethod);
  }
};

const deleteEditingTarget = async () => {
  const target = props.editor.editingTarget.value;
  if (!target) return;
  props.editor.close();
  await props.onDelete(target);
};
</script>

<template>
  <Dialog
    :open="editor.open.value"
    @update:open="$event ? undefined : editor.close()"
  >
    <DialogContent
      class="max-h-[calc(100dvh-2rem)] min-w-0 overflow-x-hidden overflow-y-auto sm:max-w-[620px]"
    >
      <DialogHeader>
        <DialogTitle>
          {{
            editor.editingTarget.value
              ? t("admin.webTerminal.editTarget", "Edit SSH target")
              : t("admin.webTerminal.addTarget", "Add SSH target")
          }}
        </DialogTitle>
        <DialogDescription>
          {{
            t(
              "admin.webTerminal.targetEditorDescription",
              "Credentials are encrypted locally. The server key must be explicitly trusted before authentication.",
            )
          }}
        </DialogDescription>
      </DialogHeader>

      <form
        class="min-w-0 space-y-5"
        @submit.prevent="
          editor.save(
            requiresTerminationConfirmation && terminateActiveSessions,
          )
        "
      >
        <div class="grid min-w-0 gap-4 sm:grid-cols-2">
          <div class="space-y-2 sm:col-span-2">
            <Label for="terminal-target-name">
              {{ t("common.name") }}
            </Label>
            <Input
              id="terminal-target-name"
              v-model="editor.draft.name"
              autocomplete="off"
              :placeholder="
                t('admin.webTerminal.targetNamePlaceholder', 'my nas')
              "
              :disabled="editor.saving.value"
            />
          </div>

          <div class="space-y-2">
            <Label for="terminal-target-host">
              {{ t("admin.webTerminal.host", "Host") }}
            </Label>
            <Input
              id="terminal-target-host"
              :model-value="editor.draft.host"
              autocomplete="off"
              spellcheck="false"
              placeholder="server.example.com"
              :disabled="editor.saving.value"
              @update:model-value="editor.setEndpoint('host', String($event))"
            />
          </div>
          <div class="space-y-2">
            <Label for="terminal-target-port">
              {{ t("common.port") }}
            </Label>
            <Input
              id="terminal-target-port"
              :model-value="editor.draft.port"
              type="number"
              min="1"
              max="65535"
              inputmode="numeric"
              :disabled="editor.saving.value"
              @update:model-value="editor.setEndpoint('port', Number($event))"
            />
          </div>
          <div class="space-y-2">
            <Label for="terminal-target-username">
              {{ t("admin.wol.ssh.username") }}
            </Label>
            <Input
              id="terminal-target-username"
              v-model="editor.draft.username"
              autocomplete="username"
              spellcheck="false"
              :disabled="editor.saving.value"
            />
          </div>
          <div class="space-y-2">
            <Label for="terminal-target-auth">
              {{ t("admin.wol.ssh.authMethod") }}
            </Label>
            <Select
              :model-value="editor.draft.authMethod"
              :disabled="editor.saving.value"
              @update:model-value="updateAuthMethod"
            >
              <SelectTrigger id="terminal-target-auth">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="password">
                  {{ t("admin.wol.ssh.passwordAuth") }}
                </SelectItem>
                <SelectItem value="privateKey">
                  {{ t("admin.wol.ssh.privateKeyAuth") }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div class="min-w-0 space-y-3 rounded-xl border border-border/70 p-4">
          <div class="flex items-center gap-2">
            <KeyRound class="h-4 w-4 text-muted-foreground" />
            <h3 class="text-sm font-medium">
              {{ t("admin.webTerminal.authentication", "Authentication") }}
            </h3>
          </div>

          <div v-if="editor.draft.authMethod === 'password'" class="space-y-2">
            <Label for="terminal-target-secret">
              {{ t("admin.wol.ssh.password") }}
            </Label>
            <Input
              id="terminal-target-secret"
              v-model="editor.draft.secret"
              type="password"
              autocomplete="new-password"
              :disabled="editor.saving.value || editor.draft.clearCredential"
              :placeholder="
                editor.credentialConfigured.value
                  ? t('admin.wol.ssh.credentialConfigured')
                  : ''
              "
            />
          </div>
          <template v-else>
            <div class="space-y-2">
              <Label for="terminal-target-secret">
                {{ t("admin.wol.ssh.privateKey") }}
              </Label>
              <Textarea
                id="terminal-target-secret"
                v-model="editor.draft.secret"
                class="field-sizing-fixed min-h-28 min-w-0 max-w-full resize-y overflow-x-auto whitespace-pre-wrap [overflow-wrap:anywhere] font-mono text-xs"
                autocomplete="off"
                spellcheck="false"
                :disabled="editor.saving.value || editor.draft.clearCredential"
                :placeholder="
                  editor.credentialConfigured.value
                    ? t('admin.wol.ssh.credentialConfigured')
                    : '-----BEGIN OPENSSH PRIVATE KEY-----'
                "
              />
            </div>
            <div class="space-y-2">
              <Label for="terminal-target-passphrase">
                {{ t("admin.wol.ssh.privateKeyPassphrase") }}
              </Label>
              <Input
                id="terminal-target-passphrase"
                v-model="editor.draft.passphrase"
                type="password"
                autocomplete="new-password"
                :disabled="
                  editor.saving.value ||
                  editor.draft.clearCredential ||
                  editor.draft.clearPassphrase
                "
                :placeholder="
                  editor.passphraseConfigured.value
                    ? t('admin.wol.ssh.passphraseConfigured')
                    : t('admin.wol.ssh.optional')
                "
              />
              <label
                v-if="editor.passphraseConfigured.value"
                class="mt-2 flex items-center gap-2 text-xs text-muted-foreground"
              >
                <Checkbox
                  v-model="editor.draft.clearPassphrase"
                  :disabled="
                    editor.saving.value || editor.draft.clearCredential
                  "
                />
                {{
                  t(
                    "admin.webTerminal.clearPassphrase",
                    "Clear private-key passphrase",
                  )
                }}
              </label>
            </div>
          </template>

          <label
            v-if="editor.editingTarget.value"
            class="flex items-center gap-2 text-xs text-muted-foreground"
          >
            <Checkbox
              v-model="editor.draft.clearCredential"
              :disabled="editor.saving.value"
            />
            {{ t("admin.wol.ssh.clearCredential") }}
          </label>
        </div>

        <div
          v-if="editor.pendingHostKey.value || editor.draft.trustedHostKey"
          class="min-w-0 space-y-3 rounded-xl border border-border/70 p-4"
        >
          <div class="flex items-center gap-2">
            <div class="flex items-center gap-2">
              <ShieldCheck class="h-4 w-4 text-muted-foreground" />
              <h3 class="text-sm font-medium">
                {{ t("admin.webTerminal.hostIdentity", "Host identity") }}
              </h3>
            </div>
          </div>

          <Alert v-if="editor.pendingHostKey.value">
            <TriangleAlert class="h-4 w-4" />
            <AlertTitle>
              {{
                t(
                  "admin.webTerminal.confirmHostKey",
                  "Confirm host fingerprint",
                )
              }}
            </AlertTitle>
            <AlertDescription class="space-y-3">
              <p class="break-all font-mono text-xs">
                {{ editor.pendingHostKey.value.algorithm }} ·
                {{ editor.pendingHostKey.value.fingerprint }}
              </p>
              <Button
                type="button"
                size="sm"
                :disabled="editor.testing.value"
                @click="editor.confirmHostKey"
              >
                <LoaderCircle
                  v-if="editor.testing.value"
                  class="mr-1.5 h-3.5 w-3.5 animate-spin"
                />
                {{
                  t(
                    "admin.webTerminal.trustAndTestHostKey",
                    "Confirm and test SSH",
                  )
                }}
              </Button>
            </AlertDescription>
          </Alert>

          <div
            v-else-if="editor.draft.trustedHostKey"
            class="rounded-lg border border-emerald-500/25 bg-emerald-500/5 p-3 text-xs"
          >
            <p
              class="flex items-center gap-1.5 font-medium text-emerald-700 dark:text-emerald-300"
            >
              <CheckCircle2 class="h-3.5 w-3.5" />
              {{ t("admin.webTerminal.hostKeyTrusted", "Fingerprint trusted") }}
            </p>
            <p class="mt-1 break-all font-mono text-muted-foreground">
              {{ editor.draft.trustedHostKey.algorithm }} ·
              {{ editor.draft.trustedHostKey.fingerprint }}
            </p>
          </div>
        </div>

        <Alert v-if="editor.error.value" variant="destructive">
          <TriangleAlert class="h-4 w-4" />
          <AlertTitle>{{
            t("admin.webTerminal.connectionErrorTitle")
          }}</AlertTitle>
          <AlertDescription>{{ editor.error.value }}</AlertDescription>
        </Alert>

        <Alert v-if="showsActiveSessionWarning">
          <TriangleAlert class="h-4 w-4" />
          <AlertTitle>
            {{ t("admin.webTerminal.activeSessionsAffected") }}
          </AlertTitle>
          <AlertDescription>
            {{
              t("admin.webTerminal.terminateSessionsOnSave", {
                count: displayedActiveSessionCount,
              })
            }}
          </AlertDescription>
        </Alert>

        <label
          v-if="requiresTerminationConfirmation"
          class="flex items-start gap-2 rounded-lg border border-amber-500/25 bg-amber-500/5 p-3 text-xs"
        >
          <Checkbox v-model="terminateActiveSessions" class="mt-0.5" />
          <span>
            {{
              displayedActiveSessionCount > 0
                ? t("admin.webTerminal.terminateSessionsOnSave", {
                    count: displayedActiveSessionCount,
                  })
                : t("admin.webTerminal.terminateSessionsOnSaveUnknown")
            }}
          </span>
        </label>

        <DialogFooter class="gap-2 sm:justify-between">
          <div class="flex gap-2">
            <ConfirmDangerPopover
              v-if="editor.editingTarget.value"
              :title="
                t('admin.webTerminal.deleteTargetTitle', 'Delete SSH target?')
              "
              :description="
                activeSessionCount
                  ? t('admin.webTerminal.deleteTargetActiveDescription', {
                      count: activeSessionCount,
                    })
                  : t(
                      'admin.webTerminal.deleteTargetDescription',
                      'The saved target and its encrypted credential will be removed.',
                    )
              "
              :confirm-text="t('common.delete')"
              :disabled="editor.saving.value || editor.testing.value"
              :on-confirm="deleteEditingTarget"
            >
              <template #trigger>
                <Button
                  type="button"
                  variant="destructive-outline"
                  :disabled="editor.saving.value || editor.testing.value"
                >
                  <Trash2 class="mr-1.5 h-4 w-4" />
                  {{ t("common.delete") }}
                </Button>
              </template>
            </ConfirmDangerPopover>
            <Button
              type="button"
              variant="outline"
              :disabled="
                editor.draft.clearCredential ||
                !editor.testable.value ||
                editor.testing.value ||
                editor.saving.value
              "
              @click="editor.testConnection"
            >
              <LoaderCircle
                v-if="editor.testing.value"
                class="mr-1.5 h-4 w-4 animate-spin"
              />
              <PlugZap v-else class="mr-1.5 h-4 w-4" />
              {{
                editor.tested.value
                  ? t(
                      "admin.webTerminal.connectionTested",
                      "Connection tested",
                    )
                  : t("admin.wol.ssh.testConnection")
              }}
            </Button>
          </div>
          <div class="flex gap-2">
            <Button
              type="button"
              variant="outline"
              :disabled="editor.saving.value"
              @click="editor.close"
            >
              {{ t("common.cancel") }}
            </Button>
            <Button
              type="submit"
              :disabled="
                !editor.canSave.value ||
                (requiresTerminationConfirmation && !terminateActiveSessions) ||
                editor.testing.value ||
                editor.saving.value
              "
            >
              <LoaderCircle
                v-if="editor.saving.value"
                class="mr-1.5 h-4 w-4 animate-spin"
              />
              {{ t("common.save") }}
            </Button>
          </div>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
