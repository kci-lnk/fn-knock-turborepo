<template>
  <Card class="min-h-[600px]">
    <CardHeader
      class="gap-4 sm:flex sm:flex-row sm:items-center sm:justify-between"
    >
      <div class="min-w-0 space-y-1.5">
        <div class="flex items-center justify-between gap-3">
          <CardTitle>{{ t("admin.authSettings.title") }}</CardTitle>
          <DocsLinkButton class="sm:hidden" :href="docsUrls.guides.auth" />
        </div>
        <CardDescription>{{
          t("admin.authSettings.description")
        }}</CardDescription>
      </div>
      <div class="grid w-full gap-2 sm:flex sm:w-auto sm:items-center">
        <DocsLinkButton
          class="hidden sm:inline-flex"
          :href="docsUrls.guides.auth"
        />
        <Button
          class="w-full sm:w-auto"
          variant="outline"
          @click="goToOidcProviders"
        >
          {{ t("admin.authSettings.oidcLogin") }}
        </Button>
        <Button class="w-full sm:w-auto" @click="openSetupDialog">
          {{ t("admin.authSettings.bindNewToken") }}
        </Button>
      </div>
    </CardHeader>
    <CardContent v-if="isLoading && showLoadingSkeleton && !credentials.length">
      <div class="border rounded-md overflow-hidden">
        <Table :class="totpTableClass" container-class="overflow-x-auto">
          <colgroup>
            <col :class="showAdminPanelAccessColumn ? 'w-[30%]' : 'w-[36%]'" />
            <col :class="showAdminPanelAccessColumn ? 'w-[20%]' : 'w-[24%]'" />
            <col :class="showAdminPanelAccessColumn ? 'w-[20%]' : 'w-[25%]'" />
            <col v-if="showAdminPanelAccessColumn" class="w-[18%]" />
            <col :class="showAdminPanelAccessColumn ? 'w-[12%]' : 'w-[15%]'" />
          </colgroup>
          <TableHeader>
            <TableRow>
              <TableHead class="whitespace-normal">
                {{ t("admin.authSettings.comment") }}
              </TableHead>
              <TableHead class="whitespace-normal">{{
                t("admin.authSettings.boundAt")
              }}</TableHead>
              <TableHead class="whitespace-normal">
                {{ t("admin.authSettings.deviceAssociation") }}
              </TableHead>
              <TableHead
                v-if="showAdminPanelAccessColumn"
                class="whitespace-normal"
              >
                {{ t("admin.authSettings.adminPanelAccess") }}
              </TableHead>
              <TableHead class="text-right">
                {{ t("admin.authSettings.actions") }}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="n in 4" :key="n">
              <TableCell><Skeleton class="h-4 w-40 max-w-full" /></TableCell>
              <TableCell><Skeleton class="h-4 w-36 max-w-full" /></TableCell>
              <TableCell><Skeleton class="h-4 w-52 max-w-full" /></TableCell>
              <TableCell v-if="showAdminPanelAccessColumn">
                <Skeleton class="h-6 w-24 max-w-full" />
              </TableCell>
              <TableCell class="text-right"
                ><Skeleton class="h-8 w-16 rounded-md ml-auto sm:w-24"
              /></TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>
    </CardContent>
    <CardContent v-else-if="!isLoading || credentials.length">
      <Table :class="totpTableClass" container-class="overflow-x-auto">
        <colgroup>
          <col :class="showAdminPanelAccessColumn ? 'w-[30%]' : 'w-[36%]'" />
          <col :class="showAdminPanelAccessColumn ? 'w-[20%]' : 'w-[24%]'" />
          <col :class="showAdminPanelAccessColumn ? 'w-[20%]' : 'w-[25%]'" />
          <col v-if="showAdminPanelAccessColumn" class="w-[18%]" />
          <col :class="showAdminPanelAccessColumn ? 'w-[12%]' : 'w-[15%]'" />
        </colgroup>
        <TableHeader>
          <TableRow>
            <TableHead class="whitespace-normal">
              {{ t("admin.authSettings.comment") }}
            </TableHead>
            <TableHead class="whitespace-normal">{{
              t("admin.authSettings.boundAt")
            }}</TableHead>
            <TableHead class="whitespace-normal">
              {{ t("admin.authSettings.deviceAssociation") }}
            </TableHead>
            <TableHead
              v-if="showAdminPanelAccessColumn"
              class="whitespace-normal"
            >
              {{ t("admin.authSettings.adminPanelAccess") }}
            </TableHead>
            <TableHead class="text-right">
              {{ t("admin.authSettings.actions") }}
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="totp in credentials" :key="totp.id">
            <TableCell class="min-w-0 whitespace-normal">
              <InlineCommentEditor
                :text="totp.comment"
                :allow-empty="false"
                :validate="(value) => validateComment(value, totp.id)"
                :save="(value) => saveComment(totp.id, value)"
              />
            </TableCell>
            <TableCell><HumanFriendlyTime :value="totp.createdAt" /></TableCell>
            <TableCell class="whitespace-normal">
              <Button
                variant="link"
                class="h-auto whitespace-normal p-0 text-left"
                @click="goToPasskeys(totp.id)"
              >
                {{ t("admin.authSettings.managePasskey") }}
              </Button>
            </TableCell>
            <TableCell v-if="showAdminPanelAccessColumn">
              <TooltipProvider>
                <Tooltip
                  :open="isAdminPanelAccessTooltipOpen(totp.id)"
                  @update:open="
                    handleAdminPanelAccessTooltipOpenChange(totp.id, $event)
                  "
                >
                  <TooltipTrigger as-child>
                    <div
                      class="inline-flex cursor-help items-center gap-2"
                      tabindex="0"
                      @click="handleAdminPanelAccessTooltipClick(totp.id)"
                    >
                      <Switch
                        :model-value="hasDockerAdminPanelAccess(totp)"
                        :disabled="isAccessScopeUpdating(totp.id)"
                        :aria-label="t('admin.authSettings.adminPanelAccess')"
                        @update:model-value="
                          handleDockerAdminPanelAccessChange(
                            totp,
                            $event === true,
                          )
                        "
                      />
                      <span class="text-xs text-muted-foreground">
                        {{
                          hasDockerAdminPanelAccess(totp)
                            ? t("admin.authSettings.adminPanelAllowed")
                            : t("admin.authSettings.adminPanelDenied")
                        }}
                      </span>
                    </div>
                  </TooltipTrigger>
                  <TooltipContent class="max-w-72 text-left">
                    <p>{{ t("admin.authSettings.adminPanelAccessTooltip") }}</p>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
            </TableCell>
            <TableCell class="text-right">
              <ConfirmDangerPopover
                :title="t('admin.authSettings.deleteTitle')"
                :description="
                  t('admin.authSettings.deleteDescription', {
                    name: totp.comment || t('admin.authSettings.tokenFallback'),
                  })
                "
                :loading="isDeleting"
                :disabled="isDeleting"
                :on-confirm="() => handleDelete(totp.id)"
              >
                <template #trigger>
                  <Button
                    variant="destructive"
                    size="sm"
                    :disabled="isDeleting"
                  >
                    {{ t("admin.authSettings.delete") }}
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </TableCell>
          </TableRow>
          <TableEmpty
            v-if="credentials.length === 0"
            :colspan="totpTableColspan"
          >
            {{ t("admin.authSettings.empty") }}
          </TableEmpty>
        </TableBody>
      </Table>
    </CardContent>
    <CardContent v-else class="min-h-[180px]" aria-hidden="true"></CardContent>
  </Card>

  <Dialog
    :open="showSetupDialog"
    @update:open="
      showSetupDialog = $event;
      if (!$event) handleCancelSetup();
    "
  >
    <DialogContent
      class="max-w-md !top-[5vh] !translate-y-0 max-h-[85vh] overflow-y-auto overscroll-contain max-sm:!inset-x-0 max-sm:!top-auto max-sm:!bottom-0 max-sm:!translate-x-0 max-sm:!translate-y-0 max-sm:!max-w-none max-sm:max-h-[100dvh] max-sm:rounded-b-none max-sm:border-b-0 max-sm:pb-[calc(env(safe-area-inset-bottom)+1rem)]"
      @focusin="handleDialogFocusIn"
    >
      <DialogHeader>
        <DialogTitle>{{ t("admin.authSettings.bindDialogTitle") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.authSettings.bindDialogDescription") }}
        </DialogDescription>
      </DialogHeader>
      <div
        v-if="setupData && setupStep === 'BIND'"
        class="flex flex-col items-center gap-6 py-4 max-sm:gap-4 max-sm:py-2"
      >
        <div class="rounded-xl border bg-white p-4">
          <QrcodeVue :value="setupData.uri" :size="200" level="M" />
        </div>
        <div class="w-full space-y-4">
          <div
            ref="otpInputAreaRef"
            class="space-y-2 flex flex-col items-center scroll-mt-24"
          >
            <Label class="text-sm text-muted-foreground self-center">{{
              t("admin.authSettings.otpLabel")
            }}</Label>
            <div class="w-full flex justify-center py-2">
              <InputOTP
                inputmode="numeric"
                :maxlength="6"
                v-model="verifyToken"
                @complete="handleBind"
                :disabled="isBinding"
                :autofocus="true"
                autocomplete="off"
                data-form-type="other"
                data-1p-ignore="true"
                data-lpignore="true"
                data-bwignore="true"
              >
                <InputOTPGroup>
                  <InputOTPSlot v-for="i in 6" :key="i - 1" :index="i - 1" />
                </InputOTPGroup>
              </InputOTP>
            </div>
            <p v-if="isBinding" class="text-sm text-muted-foreground">
              {{ t("admin.authSettings.verifying") }}
            </p>
            <p v-if="bindErrorMessage" class="text-sm text-destructive">
              {{ bindErrorMessage }}
            </p>
          </div>
        </div>
      </div>
      <div v-else-if="setupStep === 'NAME'" class="flex flex-col gap-4 py-4">
        <div class="space-y-2">
          <Label>{{ t("admin.authSettings.nameSuccessLabel") }}</Label>
          <Input
            v-model="newTotpComment"
            :placeholder="t('admin.authSettings.namePlaceholder')"
            @keyup.enter="handleSaveSetupName"
          />
          <p class="text-xs text-muted-foreground">
            {{ t("admin.authSettings.nameHelp") }}
          </p>
        </div>
        <p v-if="bindErrorMessage" class="text-sm text-destructive">
          {{ bindErrorMessage }}
        </p>
        <div class="flex justify-end gap-2 mt-4">
          <Button @click="handleSaveSetupName" :disabled="isBinding">
            <span
              v-if="isBinding"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{ t("common.save") }}
          </Button>
        </div>
      </div>
      <div v-else class="flex items-center justify-center py-12">
        <span
          class="animate-spin h-5 w-5 border-2 border-primary border-t-transparent rounded-full mr-2"
        ></span
        >{{ t("admin.authSettings.generating") }}
      </div>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import {
  ref,
  onMounted,
  onBeforeUnmount,
  watch,
  nextTick,
  computed,
} from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  TableEmpty,
} from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import InlineCommentEditor from "@admin-shared/components/InlineCommentEditor.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import {
  InputOTP,
  InputOTPGroup,
  InputOTPSlot,
} from "@/components/ui/input-otp";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { ConfigAPI } from "../lib/api";
import { docsUrls } from "../lib/docs";
import { useDockerAdminAuthStore } from "../store/dockerAdminAuth";
import QrcodeVue from "qrcode.vue";
import { toast } from "@admin-shared/utils/toast";
import type { TOTPCredential, TOTPAccessScope } from "../types";

const DOCKER_ADMIN_PANEL_ACCESS_SCOPE: TOTPAccessScope = "docker_admin_panel";

const { t } = useI18n();
const router = useRouter();
const dockerAdminAuthStore = useDockerAdminAuthStore();

const credentials = ref<TOTPCredential[]>([]);
const updatingAccessScopeIds = ref<Set<string>>(new Set());
const openAdminPanelAccessTooltipId = ref<string | null>(null);
const isTouchInteraction = ref(false);
let adminPanelAccessTooltipMediaQuery: MediaQueryList | null = null;
const { isPending: isLoading, run: runLoadStatus } = useAsyncAction({
  onError: (error) => {
    console.error("Failed to get TOTP status:", error);
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);

// Setup state
const showSetupDialog = ref(false);
const setupData = ref<{ secret: string; uri: string } | null>(null);
const verifyToken = ref("");
const newTotpComment = ref("");
const bindErrorMessage = ref("");
const setupStep = ref<"BIND" | "NAME">("BIND");
const boundTotpId = ref<string | null>(null);
const bindingMode = ref<"bind" | "rename">("bind");
const otpInputAreaRef = ref<HTMLElement | null>(null);
let viewportResizeTimer: ReturnType<typeof window.setTimeout> | null = null;
const { isPending: isBinding, run: runBindingAction } = useAsyncAction({
  onError: (error) => {
    const fallback =
      bindingMode.value === "bind"
        ? t("admin.authSettings.bindError")
        : t("admin.authSettings.renameError");
    bindErrorMessage.value = extractErrorMessage(error, fallback);
    if (bindingMode.value === "bind") {
      verifyToken.value = "";
    }
  },
});
const { run: runSetupInit } = useAsyncAction({
  onError: (error) => {
    console.error("Failed to setup TOTP:", error);
    bindErrorMessage.value = t("admin.authSettings.setupFailed");
    setupData.value = null;
  },
});
const { run: runSaveComment } = useAsyncAction({
  rethrow: true,
});

// Delete state
const { isPending: isDeleting, run: runDeleteTotp } = useAsyncAction({
  onError: (error) => {
    toast.error(
      extractErrorMessage(error, t("admin.authSettings.deleteFailed")),
    );
  },
});

const showAdminPanelAccessColumn = computed(() => {
  const target = dockerAdminAuthStore.state?.deployment_target;
  return target === "docker" || target === "openwrt";
});
const totpTableClass = computed(() =>
  showAdminPanelAccessColumn.value
    ? "min-w-[760px] table-fixed"
    : "min-w-[640px] table-fixed",
);
const totpTableColspan = computed(() =>
  showAdminPanelAccessColumn.value ? 5 : 4,
);

onMounted(async () => {
  setupAdminPanelAccessTooltipInteraction();
  window.visualViewport?.addEventListener("resize", handleVisualViewportResize);
  await fetchStatus();
});

onBeforeUnmount(() => {
  teardownAdminPanelAccessTooltipInteraction();
  window.visualViewport?.removeEventListener(
    "resize",
    handleVisualViewportResize,
  );
  if (viewportResizeTimer) {
    window.clearTimeout(viewportResizeTimer);
    viewportResizeTimer = null;
  }
});

function updateInteractionMode() {
  if (typeof window === "undefined") return;
  isTouchInteraction.value = window.matchMedia(
    "(hover: none), (pointer: coarse)",
  ).matches;
}

function setupAdminPanelAccessTooltipInteraction() {
  if (typeof window === "undefined") return;

  adminPanelAccessTooltipMediaQuery = window.matchMedia(
    "(hover: none), (pointer: coarse)",
  );
  updateInteractionMode();

  if (
    typeof adminPanelAccessTooltipMediaQuery.addEventListener === "function"
  ) {
    adminPanelAccessTooltipMediaQuery.addEventListener(
      "change",
      updateInteractionMode,
    );
    return;
  }

  adminPanelAccessTooltipMediaQuery.addListener(updateInteractionMode);
}

function teardownAdminPanelAccessTooltipInteraction() {
  if (!adminPanelAccessTooltipMediaQuery) return;

  if (
    typeof adminPanelAccessTooltipMediaQuery.removeEventListener === "function"
  ) {
    adminPanelAccessTooltipMediaQuery.removeEventListener(
      "change",
      updateInteractionMode,
    );
    adminPanelAccessTooltipMediaQuery = null;
    return;
  }

  adminPanelAccessTooltipMediaQuery.removeListener(updateInteractionMode);
  adminPanelAccessTooltipMediaQuery = null;
}

watch(
  () => [showSetupDialog.value, setupStep.value, setupData.value] as const,
  async ([isOpen, step, setup]) => {
    if (!isOpen || step !== "BIND" || !setup) return;
    await nextTick();
    scrollOtpIntoView("auto");
  },
);

async function fetchStatus() {
  await runLoadStatus(async () => {
    const res = await ConfigAPI.getTOTPStatus();
    credentials.value = (res.credentials || []).map((credential) => ({
      ...credential,
      access_scopes: credential.access_scopes || [],
    }));
  });
}

function hasDockerAdminPanelAccess(totp: TOTPCredential) {
  return (totp.access_scopes || []).includes(DOCKER_ADMIN_PANEL_ACCESS_SCOPE);
}

function isAdminPanelAccessTooltipOpen(totpId: string) {
  return openAdminPanelAccessTooltipId.value === totpId;
}

function handleAdminPanelAccessTooltipOpenChange(
  totpId: string,
  nextOpen: boolean,
) {
  openAdminPanelAccessTooltipId.value = nextOpen ? totpId : null;
}

function handleAdminPanelAccessTooltipClick(totpId: string) {
  if (!isTouchInteraction.value) return;
  openAdminPanelAccessTooltipId.value =
    openAdminPanelAccessTooltipId.value === totpId ? null : totpId;
}

function isAccessScopeUpdating(totpId: string) {
  return updatingAccessScopeIds.value.has(totpId);
}

function setAccessScopeUpdating(totpId: string, pending: boolean) {
  const next = new Set(updatingAccessScopeIds.value);
  if (pending) {
    next.add(totpId);
  } else {
    next.delete(totpId);
  }
  updatingAccessScopeIds.value = next;
}

async function handleDockerAdminPanelAccessChange(
  totp: TOTPCredential,
  enabled: boolean,
) {
  const previousScopes = [...(totp.access_scopes || [])];
  const nextScopeSet = new Set<TOTPAccessScope>(previousScopes);
  if (enabled) {
    nextScopeSet.add(DOCKER_ADMIN_PANEL_ACCESS_SCOPE);
  } else {
    nextScopeSet.delete(DOCKER_ADMIN_PANEL_ACCESS_SCOPE);
  }

  const nextScopes = [...nextScopeSet];
  totp.access_scopes = nextScopes;
  setAccessScopeUpdating(totp.id, true);

  try {
    const updated = await ConfigAPI.updateTOTPAccessScopes(totp.id, nextScopes);
    const target = credentials.value.find((item) => item.id === totp.id);
    if (target) {
      target.access_scopes = updated.access_scopes || [];
    }
    toast.success(t("admin.authSettings.adminPanelAccessUpdated"));
  } catch (error) {
    totp.access_scopes = previousScopes;
    toast.error(
      extractErrorMessage(
        error,
        t("admin.authSettings.adminPanelAccessUpdateFailed"),
      ),
    );
  } finally {
    setAccessScopeUpdating(totp.id, false);
  }
}

function scrollOtpIntoView(behavior: ScrollBehavior = "smooth") {
  otpInputAreaRef.value?.scrollIntoView({
    block: "center",
    inline: "nearest",
    behavior,
  });
}

function handleDialogFocusIn(event: FocusEvent) {
  if (setupStep.value !== "BIND") return;
  const target = event.target as HTMLElement | null;
  if (!target || !otpInputAreaRef.value?.contains(target)) return;
  window.setTimeout(() => {
    scrollOtpIntoView();
  }, 120);
}

function handleVisualViewportResize() {
  if (!showSetupDialog.value || setupStep.value !== "BIND") return;
  const viewport = window.visualViewport;
  if (!viewport) return;

  const keyboardHeight = window.innerHeight - viewport.height;
  if (keyboardHeight < 120) return;

  if (viewportResizeTimer) {
    window.clearTimeout(viewportResizeTimer);
  }

  viewportResizeTimer = window.setTimeout(() => {
    scrollOtpIntoView();
  }, 80);
}

async function openSetupDialog() {
  showSetupDialog.value = true;
  bindErrorMessage.value = "";
  verifyToken.value = "";
  newTotpComment.value = "";
  setupData.value = null;
  setupStep.value = "BIND";
  boundTotpId.value = null;
  await runSetupInit(async () => {
    setupData.value = await ConfigAPI.setupTOTP();
  });
}

function handleCancelSetup() {
  setupData.value = null;
  verifyToken.value = "";
  bindErrorMessage.value = "";
  setupStep.value = "BIND";
  boundTotpId.value = null;
}

async function handleBind() {
  const setup = setupData.value;
  if (!setup || verifyToken.value.length !== 6) return;
  bindingMode.value = "bind";
  bindErrorMessage.value = "";
  await runBindingAction(async () => {
    const randomSuffix = Math.random().toString(36).substring(2, 8);
    const randomName =
      t("admin.authSettings.randomDevicePrefix") + randomSuffix;
    await ConfigAPI.bindTOTP(setup.secret, verifyToken.value, randomName);
    await fetchStatus();

    const newCred = credentials.value.find((c) => c.comment === randomName);
    if (newCred) {
      boundTotpId.value = newCred.id;
      newTotpComment.value = randomName;
      setupStep.value = "NAME";
    } else {
      showSetupDialog.value = false;
    }
  });
}

async function handleSaveSetupName() {
  if (!newTotpComment.value.trim()) {
    bindErrorMessage.value = t("admin.authSettings.commentRequired");
    return;
  }
  if (
    credentials.value.some(
      (t) => t.comment === newTotpComment.value && t.id !== boundTotpId.value,
    )
  ) {
    bindErrorMessage.value = t("admin.authSettings.commentDuplicateDetailed");
    return;
  }
  const totpId = boundTotpId.value;
  if (!totpId) return;

  bindingMode.value = "rename";
  bindErrorMessage.value = "";
  await runBindingAction(async () => {
    await ConfigAPI.updateTOTPComment(totpId, newTotpComment.value);
    showSetupDialog.value = false;
    await fetchStatus();
    toast.success(t("admin.authSettings.deviceSaved"));
  });
}

function validateComment(newText: string, id: string) {
  if (credentials.value.some((t) => t.comment === newText && t.id !== id)) {
    return t("admin.authSettings.commentDuplicate");
  }
}

async function saveComment(id: string, newText: string) {
  await runSaveComment(() => ConfigAPI.updateTOTPComment(id, newText), {
    onSuccess: () => {
      const target = credentials.value.find((t) => t.id === id);
      if (target) {
        target.comment = newText;
      }
      toast.success(t("admin.authSettings.commentUpdated"));
    },
    onError: (error) => {
      throw new Error(
        extractErrorMessage(error, t("admin.authSettings.renameError")),
      );
    },
  });
}

async function handleDelete(totpId: string) {
  await runDeleteTotp(async () => {
    await ConfigAPI.deleteTOTP(totpId);
    await fetchStatus();
    toast.success(t("admin.authSettings.tokenDeleted"));
  });
}

function goToPasskeys(totpId: string) {
  router.push(`/auth/passkeys/${encodeURIComponent(totpId)}`);
}

function goToOidcProviders() {
  router.push("/auth/oidc-providers");
}
</script>
