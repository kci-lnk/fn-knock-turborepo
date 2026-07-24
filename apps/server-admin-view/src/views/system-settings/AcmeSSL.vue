<template>
  <Card class="w-full">
    <CardHeader>
      <div class="flex items-start justify-between gap-4">
        <div class="grid gap-1">
          <CardTitle class="flex items-center gap-2">
            ACME.sh
            <Badge :variant="statusBadgeVariant">{{ statusLabel }}</Badge>
          </CardTitle>
          <CardDescription>{{
            t("admin.acmeSsl.description")
          }}</CardDescription>
        </div>
        <RefreshButton
          :loading="isFetching"
          :disabled="isInitializing || isFetching || isSwitchingCa"
          @click="fetchStatus"
        />
      </div>
    </CardHeader>

    <CardContent
      v-if="isInitializing && showInitializingSkeleton"
      class="grid gap-6"
    >
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div class="border p-4 rounded-lg">
          <Skeleton class="h-4 w-20 mb-2" />
          <Skeleton class="h-5 w-24" />
          <Skeleton class="h-3 w-40 mt-3" />
        </div>
        <div class="border p-4 rounded-lg md:col-span-2">
          <div class="flex justify-between items-center">
            <Skeleton class="h-4 w-16" />
            <Skeleton class="h-5 w-12" />
          </div>
          <div class="mt-4">
            <Skeleton class="h-3 w-full" />
          </div>
        </div>
      </div>
    </CardContent>

    <CardContent v-else-if="!isInitializing" class="grid gap-6">
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div class="border p-4 rounded-lg">
          <div class="text-sm text-muted-foreground mb-2">
            {{ t("admin.acmeSsl.clientStatus") }}
          </div>
          <div class="font-medium">{{ statusLabel }}</div>
          <div class="mt-2 text-xs text-muted-foreground font-mono break-all">
            {{ state?.message || "-" }}
          </div>
        </div>

        <div class="border p-4 rounded-lg md:col-span-2">
          <div class="flex justify-between items-center">
            <div class="text-sm text-muted-foreground">
              {{ t("admin.acmeSsl.resourceStatus") }}
            </div>
            <div
              v-if="isInstalled"
              :class="[
                'px-2 py-0.5 rounded text-xs font-medium',
                'bg-green-100 text-green-700 border border-green-200',
              ]"
            >
              {{ t("admin.acmeSsl.downloaded") }}
            </div>
            <div
              v-else
              :class="[
                'px-2 py-0.5 rounded text-xs font-medium',
                'bg-yellow-100 text-yellow-700 border border-yellow-200',
              ]"
            >
              <span v-if="!isInstalling">{{
                t("admin.acmeSsl.notDownloaded")
              }}</span>
              <span v-else>{{ t("admin.acmeSsl.downloading") }}</span>
            </div>
          </div>
          <div class="mt-4">
            <Progress v-if="progress < 100" :model-value="progress" />
          </div>
          <div
            v-if="state?.status === 'error'"
            class="text-sm bg-destructive/10 text-destructive p-3 rounded-md border border-destructive/20 mt-3 break-all"
          >
            {{ t("admin.acmeSsl.errorPrefix") }}{{ state?.message }}
          </div>
          <div
            v-else-if="isInstalling"
            class="text-sm text-muted-foreground mt-3 animate-pulse"
          >
            {{ t("admin.acmeSsl.installingText") }}
          </div>
        </div>
      </div>

      <div v-if="isInstalled" class="rounded-lg border bg-muted/20 p-4">
        <div
          class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between sm:gap-4"
        >
          <div class="grid min-w-0 gap-1">
            <div class="text-sm font-medium">
              {{ t("admin.acmeSsl.caTitle") }}
            </div>
            <div class="text-xs text-muted-foreground">
              {{ t("admin.acmeSsl.caDescription") }}
            </div>
          </div>
          <Badge
            variant="outline"
            class="shrink-0 self-start whitespace-nowrap"
          >
            {{ currentCertificateAuthorityLabel }}
          </Badge>
        </div>

        <div class="mt-4 grid grid-cols-1 gap-3 md:grid-cols-2">
          <button
            v-for="option in certificateAuthorityOptions"
            :key="option.value"
            type="button"
            :disabled="
              isSwitchingCa ||
              isDeleting ||
              isFetching ||
              currentCertificateAuthority === option.value
            "
            :class="[
              'rounded-xl border p-4 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-60',
              currentCertificateAuthority === option.value
                ? 'border-primary bg-primary/5 shadow-sm'
                : 'border-border bg-background hover:border-primary/40 hover:bg-muted/40',
            ]"
            @click="switchCertificateAuthority(option.value)"
          >
            <div
              class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
            >
              <div class="grid min-w-0 gap-1">
                <div class="text-sm font-medium">{{ option.label }}</div>
                <div class="text-xs text-muted-foreground">
                  {{ option.description }}
                </div>
              </div>
              <span
                :class="[
                  'self-start shrink-0 whitespace-nowrap rounded-full border px-2 py-0.5 text-[11px] font-medium',
                  currentCertificateAuthority === option.value
                    ? 'border-primary/20 bg-primary/10 text-primary'
                    : 'border-border text-muted-foreground',
                ]"
              >
                {{
                  currentCertificateAuthority === option.value
                    ? t("admin.acmeSsl.current")
                    : t("admin.acmeSsl.switchAction")
                }}
              </span>
            </div>
          </button>
        </div>

        <div class="mt-3 text-xs text-muted-foreground">
          {{ t("admin.acmeSsl.caHint") }}
        </div>
        <div
          v-if="isSwitchingCa"
          class="mt-3 text-sm text-muted-foreground animate-pulse"
        >
          {{ t("admin.acmeSsl.switchingCa") }}
        </div>
      </div>

      <div
        v-if="!isInstalled && !isInstalling"
        class="rounded-lg border bg-muted/20 p-4"
      >
        <div class="flex items-start justify-between gap-4">
          <div class="grid gap-1">
            <div class="text-sm font-medium">
              {{ t("admin.acmeSsl.installConfigTitle") }}
            </div>
            <div class="text-xs text-muted-foreground">
              {{ t("admin.acmeSsl.installConfigDescription") }}
            </div>
          </div>
        </div>

        <div class="mt-3">
          <Button
            class="w-full md:w-auto"
            :disabled="isStartingInstall"
            @click="startInstall"
          >
            <span
              v-if="isStartingInstall"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{ t("admin.acmeSsl.startInstall") }}
          </Button>
        </div>
      </div>
    </CardContent>
    <CardContent v-else class="min-h-[220px]" aria-hidden="true"></CardContent>

    <CardFooter
      v-if="!isInitializing"
      class="flex justify-end gap-3 border-t pt-6"
    >
      <template v-if="isInstalling">
        <div
          class="text-sm text-muted-foreground animate-pulse flex items-center h-10 mr-auto"
        >
          {{ t("admin.acmeSsl.installingText") }}
        </div>
        <RefreshButton
          :loading="isFetching"
          :disabled="isFetching"
          @click="fetchStatus"
        />
      </template>
      <template v-else>
        <ConfirmDangerPopover
          v-if="isInstalled"
          :title="t('admin.acmeSsl.deleteTitle')"
          :description="t('admin.acmeSsl.deleteDescription')"
          :loading="isDeleting"
          :disabled="isDeleting || isSwitchingCa"
          :on-confirm="uninstall"
          content-class="w-80 text-left"
        >
          <template #trigger>
            <Button
              variant="destructive"
              :disabled="isDeleting || isSwitchingCa"
            >
              {{ t("admin.acmeSsl.delete") }}
            </Button>
          </template>
        </ConfirmDangerPopover>
      </template>
    </CardFooter>
  </Card>

  <Dialog
    :open="showSwitchCaDialog"
    @update:open="handleSwitchCaDialogOpenChange"
  >
    <DialogContent class="sm:max-w-[460px]">
      <DialogHeader>
        <DialogTitle>{{ t("admin.acmeSsl.switchDialogTitle") }}</DialogTitle>
        <DialogDescription>
          {{
            t("admin.acmeSsl.switchDialogDescription", {
              from: currentCertificateAuthorityLabel,
              to: pendingCertificateAuthorityLabel,
            })
          }}
        </DialogDescription>
      </DialogHeader>

      <div class="grid gap-3 text-sm text-muted-foreground">
        <p>{{ t("admin.acmeSsl.switchDialogParagraph1") }}</p>
        <p>{{ t("admin.acmeSsl.switchDialogParagraph2") }}</p>
      </div>

      <DialogFooter>
        <Button
          variant="outline"
          :disabled="isSwitchingCa"
          @click="handleSwitchCaDialogOpenChange(false)"
        >
          {{ t("common.cancel") }}
        </Button>
        <Button
          :disabled="isSwitchingCa"
          @click="confirmSwitchCertificateAuthority"
        >
          <span
            v-if="isSwitchingCa"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.acmeSsl.confirmSwitch") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import RefreshButton from "@/components/RefreshButton.vue";
import { Progress } from "@/components/ui/progress";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/ui/badge";
import { toast } from "@admin-shared/utils/toast";
import { AcmeAPI } from "../../lib/api";
import { usePollingResourceStatus } from "@admin-shared/composables/usePollingResourceStatus";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";

type AcmeCertificateAuthority = "zerossl" | "letsencrypt";
type AcmeState = {
  status: "uninstalled" | "installing" | "installed" | "error";
  progress: number;
  message: string;
  certificateAuthority: AcmeCertificateAuthority;
  certificateAuthorityUpdatedAt?: string;
};

const { t } = useI18n();

const certificateAuthorityOptions = computed<
  Array<{
    value: AcmeCertificateAuthority;
    label: string;
    description: string;
  }>
>(() => [
  {
    value: "letsencrypt",
    label: "Let's Encrypt",
    description: t("admin.acmeSsl.caLetsEncryptDescription"),
  },
  {
    value: "zerossl",
    label: "ZeroSSL",
    description: t("admin.acmeSsl.caZeroSslDescription"),
  },
]);

const state = ref<AcmeState | null>(null);
const showSwitchCaDialog = ref(false);
const pendingCertificateAuthority = ref<AcmeCertificateAuthority | null>(null);
const { isPending: isFetching, run: runFetchStatus } = useAsyncAction();
const { isPending: isStartingInstall, run: runStartInstall } = useAsyncAction({
  onError: async (error) => {
    toast.error(extractErrorMessage(error, t("admin.acmeSsl.installFailed")));
    await fetchStatus();
  },
});
const { isPending: isDeleting, run: runUninstall } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t("admin.acmeSsl.deleteFailed")));
    void fetchStatus();
  },
});
const { isPending: isSwitchingCa, run: runSwitchCa } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t("admin.acmeSsl.switchFailed")));
    void fetchStatus();
  },
});

const isInstalling = computed(() => state.value?.status === "installing");
const isInstalled = computed(() => state.value?.status === "installed");
const currentCertificateAuthority = computed<AcmeCertificateAuthority>(
  () => state.value?.certificateAuthority || "zerossl",
);
const currentCertificateAuthorityLabel = computed(() => {
  return (
    certificateAuthorityOptions.value.find(
      (option) => option.value === currentCertificateAuthority.value,
    )?.label || "ZeroSSL"
  );
});
const pendingCertificateAuthorityLabel = computed(() => {
  return (
    certificateAuthorityOptions.value.find(
      (option) => option.value === pendingCertificateAuthority.value,
    )?.label || "-"
  );
});

const progress = computed(() => {
  const v = state.value?.progress ?? 0;
  return Math.max(0, Math.min(100, Number.isFinite(v) ? v : 0));
});

const statusLabel = computed(() => {
  const s = state.value?.status;
  if (!s) return t("admin.acmeSsl.statusUnknown");
  if (s === "installed") return t("admin.acmeSsl.statusInstalled");
  if (s === "installing") return t("admin.acmeSsl.statusInstalling");
  if (s === "error") return t("admin.acmeSsl.statusError");
  return t("admin.acmeSsl.statusUninstalled");
});

const statusBadgeVariant = computed(() => {
  const s = state.value?.status;
  if (s === "installed") return "default";
  if (s === "installing") return "secondary";
  if (s === "error") return "destructive";
  return "outline";
});

const { isInitializing, refresh: fetchStatus } =
  usePollingResourceStatus<AcmeState | null>({
    fetcher: async () => {
      const data = await runFetchStatus(() => AcmeAPI.status());
      return data ?? state.value;
    },
    onData: (data) => {
      state.value = data;
    },
    isDownloading: (data) => data?.status === "installing",
  });
const showInitializingSkeleton = useDelayedLoading(isInitializing);

async function startInstall() {
  if (isInstalling.value) return;
  await runStartInstall(() => AcmeAPI.init(), {
    onSuccess: async () => {
      toast.success(t("admin.acmeSsl.installStarted"));
      await fetchStatus();
    },
  });
}

async function uninstall() {
  if (!isInstalled.value) return;
  await runUninstall(async () => {
    await AcmeAPI.uninstall();
    toast.success(t("admin.acmeSsl.deleted"));
    await fetchStatus();
  });
}

function switchCertificateAuthority(next: AcmeCertificateAuthority) {
  if (!isInstalled.value) return;
  if (currentCertificateAuthority.value === next) return;
  pendingCertificateAuthority.value = next;
  showSwitchCaDialog.value = true;
}

async function confirmSwitchCertificateAuthority() {
  const next = pendingCertificateAuthority.value;
  if (!next) return;
  await runSwitchCa(async () => {
    await AcmeAPI.updateClientSettings({ certificateAuthority: next });
    toast.success(
      t("admin.acmeSsl.switchedTo", {
        name:
          certificateAuthorityOptions.value.find(
            (option) => option.value === next,
          )?.label || next,
      }),
    );
    showSwitchCaDialog.value = false;
    pendingCertificateAuthority.value = null;
    await fetchStatus();
  });
}

function handleSwitchCaDialogOpenChange(open: boolean) {
  showSwitchCaDialog.value = open;
  if (!open && !isSwitchingCa.value) {
    pendingCertificateAuthority.value = null;
  }
}
</script>
