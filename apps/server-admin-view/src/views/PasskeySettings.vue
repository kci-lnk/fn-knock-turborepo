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
                ><HumanFriendlyTime :value="passkey.createdAt" :locale="locale"
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
                  :description="
                    t('admin.passkeySettings.deleteOidcDescription')
                  "
                  :loading="isDeletingBinding"
                  :disabled="isDeletingBinding"
                  :on-confirm="() => deleteOidcBinding(binding.id)"
                >
                  <template #trigger>
                    <Button
                      variant="destructive"
                      size="sm"
                      :disabled="isDeletingBinding"
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

    <OidcInviteDialog
      v-model:open="showInviteDialog"
      :expires-at="inviteExpiresAt"
      :invite-url="inviteUrl"
      :is-creating="isInviteCreating"
      :provider-id="inviteProviderId"
      :providers="providers"
      @copy="copyInviteUrl"
      @create="createInvite"
      @provider-change="handleInviteProviderChange"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
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
import { Button } from "@/components/ui/button";
import { Link2 } from "lucide-vue-next";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import RefreshButton from "@/components/RefreshButton.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "../lib/api";
import type { PasskeyCredential } from "../types";
import OidcInviteDialog from "./passkey-settings/OidcInviteDialog.vue";
import { useOidcBindingWorkflow } from "./passkey-settings/useOidcBindingWorkflow";

const route = useRoute();
const { t, locale } = useI18n();
const totpId = route.params.totpId as string;

const passkeys = ref<PasskeyCredential[]>([]);
const errorMessage = ref("");
const totpName = ref("");
const {
  copyInviteUrl,
  createInvite,
  deleteOidcBinding,
  handleInviteProviderChange,
  handleRefreshOidcBindings,
  inviteExpiresAt,
  inviteProviderId,
  inviteUrl,
  isDeletingBinding,
  isInviteCreating,
  isOidcBindingsRefreshing,
  loadOidcData,
  oidcBindings,
  openInviteDialog,
  providers,
  showInviteDialog,
} = useOidcBindingWorkflow({
  totpId,
  setError: (message) => {
    errorMessage.value = message;
  },
});

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
onMounted(fetchCredentials);

async function fetchCredentials() {
  errorMessage.value = "";
  await runLoad(async () => {
    totpName.value = "";
    const [passkeysRes, statusRes] = await Promise.all([
      ConfigAPI.getPasskeys(totpId),
      ConfigAPI.getTOTPStatus().catch(() => null),
      loadOidcData(),
    ]);
    passkeys.value = passkeysRes;
    if (statusRes?.credentials) {
      const parentTotp = statusRes.credentials.find(
        (item) => item.id === totpId,
      );
      if (parentTotp?.comment) totpName.value = parentTotp.comment;
    }
  });
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
</script>
