<template>
  <div class="space-y-4">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/auth">
            {{ t("admin.authSettings.title") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{ pageTitle }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card>
      <CardHeader>
        <CardTitle>{{ t("admin.passkeySettings.title") }}</CardTitle>
        <CardDescription>{{
          t("admin.passkeySettings.description")
        }}</CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <div
          v-if="isLoading"
          class="flex items-center justify-center py-10 text-sm text-muted-foreground"
        >
          <span
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent"
          ></span>
          {{ t("admin.passkeySettings.loading") }}
        </div>
        <Table v-else>
          <TableHeader>
            <TableRow>
              <TableHead>Passkey</TableHead>
              <TableHead>{{ t("admin.passkeySettings.device") }}</TableHead>
              <TableHead>{{ t("admin.passkeySettings.boundAt") }}</TableHead>
              <TableHead class="text-right">{{
                t("admin.sessions.table.actions")
              }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="passkey in passkeys" :key="passkey.id">
              <TableCell class="font-mono text-xs text-muted-foreground">
                {{ formatId(passkey.id) }}
              </TableCell>
              <TableCell>{{ passkey.deviceName }}</TableCell>
              <TableCell
                ><HumanFriendlyTime
                  :value="passkey.createdAt"
                  :locale="locale"
              /></TableCell>
              <TableCell class="text-right">
                <ConfirmDangerPopover
                  :title="t('admin.passkeySettings.deletePasskeyTitle')"
                  :description="
                    t('admin.passkeySettings.deletePasskeyDescription')
                  "
                  :loading="isDeleting"
                  :disabled="isDeleting"
                  :on-confirm="() => handleDeletePasskey(passkey.id)"
                >
                  <template #trigger>
                    <Button
                      variant="destructive"
                      size="sm"
                      :disabled="isDeleting"
                    >
                      {{ t("admin.passkeySettings.delete") }}
                    </Button>
                  </template>
                </ConfirmDangerPopover>
              </TableCell>
            </TableRow>
            <TableEmpty v-if="passkeys.length === 0" :colspan="4">
              {{ t("admin.passkeySettings.emptyPasskeys") }}
            </TableEmpty>
          </TableBody>
        </Table>
      </CardContent>
    </Card>

    <Card>
      <CardHeader
        class="gap-4 sm:flex sm:flex-row sm:items-center sm:justify-between"
      >
        <div>
          <CardTitle>{{ t("admin.passkeySettings.oidcTitle") }}</CardTitle>
          <CardDescription>
            {{ t("admin.passkeySettings.oidcDescription") }}
          </CardDescription>
        </div>
        <div
          class="flex w-full flex-col gap-2 sm:w-auto sm:flex-row sm:items-center"
        >
          <RefreshButton
            class="w-full sm:w-auto"
            :loading="isLoading || isOidcBindingsRefreshing"
            :disabled="isLoading || isOidcBindingsRefreshing"
            @click="handleRefreshOidcBindings"
          />
          <Button
            variant="outline"
            class="w-full sm:w-auto"
            :disabled="providers.length === 0 || isInviteCreating"
            @click="openInviteDialog"
          >
            <Link2 class="h-4 w-4" />
            {{ t("admin.passkeySettings.generateInvite") }}
          </Button>
        </div>
      </CardHeader>
      <CardContent class="space-y-4">
        <div
          v-if="providers.length === 0"
          class="rounded-md border border-dashed px-4 py-3 text-sm text-muted-foreground"
        >
          {{ t("admin.passkeySettings.noProviders") }}
        </div>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{{ t("admin.passkeySettings.provider") }}</TableHead>
              <TableHead>{{ t("admin.passkeySettings.account") }}</TableHead>
              <TableHead>Subject</TableHead>
              <TableHead>{{ t("admin.passkeySettings.lastUsed") }}</TableHead>
              <TableHead class="text-right">{{
                t("admin.sessions.table.actions")
              }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="binding in oidcBindings" :key="binding.id">
              <TableCell>{{
                binding.provider_name || binding.provider_type
              }}</TableCell>
              <TableCell>
                <div class="font-medium">{{ binding.display_name || "-" }}</div>
                <div class="text-xs text-muted-foreground">
                  {{ binding.email || "" }}
                </div>
              </TableCell>
              <TableCell class="font-mono text-xs text-muted-foreground">
                {{ formatId(binding.subject) }}
              </TableCell>
              <TableCell>
                <HumanFriendlyTime
                  v-if="binding.last_used_at"
                  :value="binding.last_used_at"
                  :locale="locale"
                />
                <span v-else class="text-muted-foreground">-</span>
              </TableCell>
              <TableCell class="text-right">
                <ConfirmDangerPopover
                  :title="t('admin.passkeySettings.deleteOidcTitle')"
                  :description="t('admin.passkeySettings.deleteOidcDescription')"
                  :loading="isDeleting"
                  :disabled="isDeleting"
                  :on-confirm="() => handleDeleteOidcBinding(binding.id)"
                >
                  <template #trigger>
                    <Button
                      variant="destructive"
                      size="sm"
                      :disabled="isDeleting"
                    >
                      {{ t("admin.passkeySettings.delete") }}
                    </Button>
                  </template>
                </ConfirmDangerPopover>
              </TableCell>
            </TableRow>
            <TableEmpty v-if="oidcBindings.length === 0" :colspan="5">
              {{ t("admin.passkeySettings.emptyOidc") }}
            </TableEmpty>
          </TableBody>
        </Table>
        <div v-if="errorMessage" class="text-sm text-destructive">
          {{ errorMessage }}
        </div>
      </CardContent>
    </Card>

    <Dialog :open="showInviteDialog" @update:open="handleInviteDialogOpenChange">
      <DialogContent class="max-h-[88vh] overflow-y-auto sm:max-w-[560px]">
        <DialogHeader>
          <DialogTitle>{{ t("admin.passkeySettings.inviteTitle") }}</DialogTitle>
          <DialogDescription>
            {{ t("admin.passkeySettings.inviteDescription") }}
          </DialogDescription>
        </DialogHeader>
        <div class="overflow-hidden rounded-lg border divide-y divide-border">
          <div class="space-y-2 p-4 transition-colors hover:bg-muted/10 sm:p-5">
            <Label for="oidc-invite-provider">{{
              t("admin.passkeySettings.provider")
            }}</Label>
            <Select
              :model-value="inviteProviderId"
              @update:model-value="handleInviteProviderChange"
            >
              <SelectTrigger id="oidc-invite-provider" class="w-full">
                <SelectValue
                  :placeholder="t('admin.passkeySettings.providerPlaceholder')"
                />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="provider in providers"
                  :key="provider.id"
                  :value="provider.id"
                >
                  {{ provider.name }}
                </SelectItem>
              </SelectContent>
            </Select>
            <p class="text-[11px] text-muted-foreground">
              {{ t("admin.passkeySettings.inviteExpiresIn") }}
            </p>
          </div>
          <div
            v-if="inviteUrl"
            class="space-y-3 p-4 transition-colors hover:bg-muted/10 sm:p-5"
          >
            <Label>{{ t("admin.passkeySettings.inviteLink") }}</Label>
            <div
              class="flex items-start gap-2 rounded-md border bg-muted/30 px-2.5 py-2"
            >
              <p
                class="min-w-0 flex-1 whitespace-normal break-all font-mono text-xs leading-5 text-muted-foreground"
              >
                {{ inviteUrl }}
              </p>
              <Button
                variant="ghost"
                size="icon-sm"
                class="size-7 shrink-0"
                :title="t('admin.passkeySettings.copyInviteLink')"
                :aria-label="t('admin.passkeySettings.copyInviteLink')"
                @click="copyInviteUrl"
              >
                <Copy class="h-4 w-4" />
              </Button>
            </div>
            <p class="text-xs text-muted-foreground">
              {{
                t("admin.passkeySettings.expiresAt", {
                  time: inviteExpiresAt || "-",
                })
              }}
            </p>
          </div>
        </div>
        <DialogFooter class="gap-2">
          <Button variant="outline" @click="showInviteDialog = false">
            {{ t("admin.passkeySettings.close") }}
          </Button>
          <Button
            v-if="inviteUrl"
            variant="outline"
            @click="copyInviteUrl"
          >
            <Copy class="h-4 w-4" />
            {{ t("admin.passkeySettings.copyLink") }}
          </Button>
          <Button
            :disabled="isInviteCreating || !inviteProviderId"
            @click="createInvite"
          >
            <LoaderCircle
              v-if="isInviteCreating"
              class="h-4 w-4 animate-spin"
            />
            <Link2 v-else class="h-4 w-4" />
            {{ t("admin.passkeySettings.generate") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableEmpty,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Copy, Link2, LoaderCircle } from "lucide-vue-next";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import RefreshButton from "@/components/RefreshButton.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "../lib/api";
import type {
  OIDCBinding,
  OIDCProviderView,
  PasskeyCredential,
} from "../types";

const route = useRoute();
const { t, locale } = useI18n();
const totpId = route.params.totpId as string;
const OIDC_BINDINGS_AUTO_REFRESH_INTERVAL_MS = 5000;

const passkeys = ref<PasskeyCredential[]>([]);
const oidcBindings = ref<OIDCBinding[]>([]);
const providers = ref<OIDCProviderView[]>([]);
const errorMessage = ref("");
const totpName = ref("");
const showInviteDialog = ref(false);
const inviteProviderId = ref("");
const inviteUrl = ref("");
const inviteExpiresAt = ref("");
const isOidcBindingsRefreshing = ref(false);
let oidcBindingsAutoRefreshTimer: ReturnType<typeof window.setInterval> | null =
  null;

const pageTitle = computed(() =>
  totpName.value
    ? t("admin.passkeySettings.titleWithName", { name: totpName.value })
    : t("admin.passkeySettings.title"),
);

const { isPending: isLoading, run: runLoad } = useAsyncAction({
  onError: (error) => {
    errorMessage.value = extractErrorMessage(
      error,
      t("admin.passkeySettings.loadFailed"),
    );
  },
});
const { isPending: isDeleting, run: runDelete } = useAsyncAction({
  onError: (error) => {
    const message = extractErrorMessage(
      error,
      t("admin.passkeySettings.deleteFailed"),
    );
    errorMessage.value = message;
    toast.error(t("admin.passkeySettings.deleteErrorTitle"), {
      description: message,
    });
  },
});
const { isPending: isInviteCreating, run: runCreateInvite } = useAsyncAction({
  onError: (error) => {
    toast.error(
      extractErrorMessage(error, t("admin.passkeySettings.createInviteFailed")),
    );
  },
});

onMounted(fetchCredentials);
onBeforeUnmount(stopOidcBindingsAutoRefresh);

watch(showInviteDialog, (isOpen) => {
  if (isOpen) {
    startOidcBindingsAutoRefresh();
    return;
  }
  stopOidcBindingsAutoRefresh();
});

async function fetchCredentials() {
  errorMessage.value = "";
  await runLoad(async () => {
    totpName.value = "";
    const [passkeysRes, statusRes, bindingsRes, providersRes] =
      await Promise.all([
        ConfigAPI.getPasskeys(totpId),
        ConfigAPI.getTOTPStatus().catch(() => null),
        ConfigAPI.getOIDCBindings(totpId),
        ConfigAPI.getOIDCProviders(),
      ]);
    passkeys.value = passkeysRes;
    oidcBindings.value = bindingsRes;
    providers.value = providersRes.filter((provider) => provider.enabled);
    if (statusRes?.credentials) {
      const parentTotp = statusRes.credentials.find(
        (item) => item.id === totpId,
      );
      if (parentTotp?.comment) totpName.value = parentTotp.comment;
    }
  });
}

async function refreshOidcBindings(options?: {
  notifyOnAdded?: boolean;
  showSuccessToast?: boolean;
  showErrorToast?: boolean;
}) {
  if (isOidcBindingsRefreshing.value) return;

  const previousIds = new Set(oidcBindings.value.map((binding) => binding.id));
  isOidcBindingsRefreshing.value = true;
  try {
    const nextBindings = await ConfigAPI.getOIDCBindings(totpId);
    const addedBindings = nextBindings.filter(
      (binding) => !previousIds.has(binding.id),
    );
    oidcBindings.value = nextBindings;
    errorMessage.value = "";

    if (options?.notifyOnAdded && addedBindings.length > 0) {
      const firstBinding = addedBindings[0];
      if (!firstBinding) return;
      toast.success(
        addedBindings.length > 1
          ? t("admin.passkeySettings.addedBindingsMany", {
              count: addedBindings.length,
            })
          : t("admin.passkeySettings.addedBindingOne"),
        {
          description: formatOidcBindingLabel(firstBinding),
        },
      );
      return;
    }

    if (options?.showSuccessToast) {
      toast.success(t("admin.passkeySettings.bindingsRefreshed"));
    }
  } catch (error) {
    const message = extractErrorMessage(
      error,
      t("admin.passkeySettings.refreshFailed"),
    );
    errorMessage.value = message;
    if (options?.showErrorToast) {
      toast.error(t("admin.passkeySettings.refreshErrorTitle"), {
        description: message,
      });
    } else {
      console.error("refreshOidcBindings:", error);
    }
  } finally {
    isOidcBindingsRefreshing.value = false;
  }
}

function formatOidcBindingLabel(binding: OIDCBinding) {
  return (
    binding.display_name ||
    binding.email ||
    binding.provider_name ||
    binding.provider_type
  );
}

function handleRefreshOidcBindings() {
  void refreshOidcBindings({
    notifyOnAdded: showInviteDialog.value,
    showSuccessToast: !showInviteDialog.value,
    showErrorToast: true,
  });
}

function startOidcBindingsAutoRefresh() {
  stopOidcBindingsAutoRefresh();
  oidcBindingsAutoRefreshTimer = window.setInterval(() => {
    void refreshOidcBindings({ notifyOnAdded: true });
  }, OIDC_BINDINGS_AUTO_REFRESH_INTERVAL_MS);
}

function stopOidcBindingsAutoRefresh() {
  if (oidcBindingsAutoRefreshTimer === null) return;
  window.clearInterval(oidcBindingsAutoRefreshTimer);
  oidcBindingsAutoRefreshTimer = null;
}

function formatId(id: string) {
  if (id.length <= 12) return id;
  return `${id.slice(0, 6)}...${id.slice(-6)}`;
}

async function handleDeletePasskey(passkeyId: string) {
  errorMessage.value = "";
  await runDelete(async () => {
    await ConfigAPI.deletePasskey(passkeyId);
    await fetchCredentials();
    toast.success(t("admin.passkeySettings.passkeyDeleted"));
  });
}

async function handleDeleteOidcBinding(bindingId: string) {
  errorMessage.value = "";
  await runDelete(async () => {
    await ConfigAPI.deleteOIDCBinding(bindingId);
    await fetchCredentials();
    toast.success(t("admin.passkeySettings.oidcDeleted"));
  });
}

function openInviteDialog() {
  inviteProviderId.value = providers.value[0]?.id || "";
  inviteUrl.value = "";
  inviteExpiresAt.value = "";
  showInviteDialog.value = true;
}

function handleInviteDialogOpenChange(open: boolean) {
  showInviteDialog.value = open;
}

function handleInviteProviderChange(value: unknown) {
  inviteProviderId.value = String(value ?? "");
  inviteUrl.value = "";
  inviteExpiresAt.value = "";
}

async function createInvite() {
  if (!inviteProviderId.value) {
    toast.error(t("admin.passkeySettings.selectProvider"));
    return;
  }

  await runCreateInvite(async () => {
    const result = await ConfigAPI.createOIDCInvite({
      totp_id: totpId,
      provider_id: inviteProviderId.value,
    });
    inviteUrl.value = result.invite_url;
    inviteExpiresAt.value = result.expires_at;
    try {
      await copyTextToClipboard(result.invite_url);
      toast.success(t("admin.passkeySettings.inviteCreatedCopied"), {
        description: result.invite_url,
      });
    } catch (error) {
      console.error("createInvite copy:", error);
      toast.warning(t("admin.passkeySettings.inviteCreatedCopyFailed"), {
        description: t("admin.passkeySettings.manualCopyHint"),
      });
    }
  });
}

async function copyInviteUrl() {
  if (!inviteUrl.value) return;
  try {
    await copyTextToClipboard(inviteUrl.value);
    toast.success(t("admin.passkeySettings.inviteCopied"), {
      description: inviteUrl.value,
    });
  } catch (error) {
    console.error("copyInviteUrl:", error);
    toast.error(t("admin.passkeySettings.copyInviteFailed"), {
      description: t("admin.passkeySettings.manualCopyHint"),
    });
  }
}

async function copyTextToClipboard(text: string) {
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      return;
    } catch {
      // Fall back below for non-secure or embedded browser contexts.
    }
  }

  if (typeof document === "undefined") {
    throw new Error("Clipboard API unavailable");
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.top = "0";
  textarea.style.left = "0";
  textarea.style.opacity = "0";

  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();
  textarea.setSelectionRange(0, textarea.value.length);

  const copied = document.execCommand("copy");
  document.body.removeChild(textarea);

  if (!copied) {
    throw new Error("execCommand copy failed");
  }
}
</script>
