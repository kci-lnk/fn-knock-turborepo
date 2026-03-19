<template>
  <Card data-guide-auth-settings class="min-h-[600px]">
    <CardHeader class="flex flex-row items-center justify-between">
      <div>
        <CardTitle>TOTP 令牌管理</CardTitle>
        <CardDescription
          >管理管理员登录使用的所有 TOTP 双端验证器。</CardDescription
        >
      </div>
      <div class="flex items-center gap-2">
        <DocsLinkButton :href="docsUrls.guides.auth" />
        <Button data-guide-bind-token @click="openSetupDialog">
          绑定新令牌
        </Button>
      </div>
    </CardHeader>
    <CardContent v-if="isLoading && showLoadingSkeleton && !credentials.length">
      <div class="border rounded-md overflow-hidden">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>备注信息</TableHead>
              <TableHead>绑定时间</TableHead>
              <TableHead>设备关联</TableHead>
              <TableHead class="text-right">操作</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="n in 4" :key="n">
              <TableCell><Skeleton class="h-4 w-40" /></TableCell>
              <TableCell><Skeleton class="h-4 w-36" /></TableCell>
              <TableCell><Skeleton class="h-4 w-52" /></TableCell>
              <TableCell class="text-right"
                ><Skeleton class="h-8 w-24 rounded-md ml-auto"
              /></TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>
    </CardContent>
    <CardContent v-else-if="!isLoading || credentials.length">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>备注信息</TableHead>
            <TableHead>绑定时间</TableHead>
            <TableHead>设备关联</TableHead>
            <TableHead class="text-right">操作</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="totp in credentials" :key="totp.id">
            <TableCell>
              <InlineCommentEditor
                :text="totp.comment"
                :allow-empty="false"
                :validate="(value) => validateComment(value, totp.id)"
                :save="(value) => saveComment(totp.id, value)"
              />
            </TableCell>
            <TableCell><HumanFriendlyTime :value="totp.createdAt" /></TableCell>
            <TableCell>
              <!-- 导航到该 TOTP 的 Passkey 管理子页面 -->
              <Button
                variant="link"
                class="p-0 h-auto"
                @click="goToPasskeys(totp.id)"
              >
                管理 Passkeys
              </Button>
            </TableCell>
            <TableCell class="text-right">
              <ConfirmDangerPopover
                title="确认删除"
                :description="`确定要删除「${totp.comment || '该令牌'}」吗？删除后将无法使用该令牌进行双重验证，且关联的 Passkey 也会被删除。`"
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
                    删除
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </TableCell>
          </TableRow>
          <TableEmpty v-if="credentials.length === 0" :colspan="4">
            暂无绑定的 TOTP 令牌
          </TableEmpty>
        </TableBody>
      </Table>
    </CardContent>
    <CardContent v-else class="min-h-[180px]" aria-hidden="true"></CardContent>
  </Card>

  <!-- 绑定新令牌 Dialog -->
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
        <DialogTitle>绑定新 TOTP 令牌</DialogTitle>
        <DialogDescription
          >使用身份验证器应用扫描二维码，并输入验证码。</DialogDescription
        >
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
            <Label class="text-sm text-muted-foreground self-center"
              >输入 6 位验证码以验证并绑定</Label
            >
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
              正在验证...
            </p>
            <p v-if="bindErrorMessage" class="text-sm text-destructive">
              {{ bindErrorMessage }}
            </p>
          </div>
        </div>
      </div>
      <div v-else-if="setupStep === 'NAME'" class="flex flex-col gap-4 py-4">
        <div class="space-y-2">
          <Label>验证成功！请为该设备命名</Label>
          <Input
            v-model="newTotpComment"
            placeholder="例如：我的 iPhone"
            @keyup.enter="handleSaveSetupName"
          />
          <p class="text-xs text-muted-foreground">
            该名称将用于区分不同的登录设备
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
            保存
          </Button>
        </div>
      </div>
      <div v-else class="flex items-center justify-center py-12">
        <span
          class="animate-spin h-5 w-5 border-2 border-primary border-t-transparent rounded-full mr-2"
        ></span
        >正在生成...
      </div>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch, nextTick } from "vue";
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
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { ConfigAPI } from "../lib/api";
import { docsUrls } from "../lib/docs";
import QrcodeVue from "qrcode.vue";
import { toast } from "@admin-shared/utils/toast";
import type { TOTPCredential } from "../types";

const router = useRouter();

const credentials = ref<TOTPCredential[]>([]);
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
      bindingMode.value === "bind" ? "验证码不正确，请重试" : "更新备注失败";
    bindErrorMessage.value = extractErrorMessage(error, fallback);
    if (bindingMode.value === "bind") {
      verifyToken.value = "";
    }
  },
});
const { run: runSetupInit } = useAsyncAction({
  onError: (error) => {
    console.error("Failed to setup TOTP:", error);
    bindErrorMessage.value = "生成令牌失败";
    setupData.value = null;
  },
});
const { run: runSaveComment } = useAsyncAction({
  rethrow: true,
});

// Delete state
const { isPending: isDeleting, run: runDeleteTotp } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, "删除失败"));
  },
});

onMounted(async () => {
  window.visualViewport?.addEventListener("resize", handleVisualViewportResize);
  await fetchStatus();
});

onBeforeUnmount(() => {
  window.visualViewport?.removeEventListener(
    "resize",
    handleVisualViewportResize,
  );
  if (viewportResizeTimer) {
    window.clearTimeout(viewportResizeTimer);
    viewportResizeTimer = null;
  }
});

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
    credentials.value = res.credentials || [];
  });
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
    const randomName = "设备-" + randomSuffix;
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
    bindErrorMessage.value = "备注名称不能为空";
    return;
  }
  if (
    credentials.value.some(
      (t) => t.comment === newTotpComment.value && t.id !== boundTotpId.value,
    )
  ) {
    bindErrorMessage.value = "备注名称已存在，请使用其他名称";
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
    toast.success("设备已绑定并保存备注");
  });
}

function validateComment(newText: string, id: string) {
  if (credentials.value.some((t) => t.comment === newText && t.id !== id)) {
    return "备注名称已存在";
  }
}

async function saveComment(id: string, newText: string) {
  await runSaveComment(() => ConfigAPI.updateTOTPComment(id, newText), {
    onSuccess: () => {
      const target = credentials.value.find((t) => t.id === id);
      if (target) {
        target.comment = newText;
      }
      toast.success("备注已更新");
    },
    onError: (error) => {
      throw new Error(extractErrorMessage(error, "更新备注失败"));
    },
  });
}

async function handleDelete(totpId: string) {
  await runDeleteTotp(async () => {
    await ConfigAPI.deleteTOTP(totpId);
    await fetchStatus();
    toast.success("令牌已删除");
  });
}

function goToPasskeys(totpId: string) {
  router.push(`/auth/passkeys/${encodeURIComponent(totpId)}`);
}
</script>
