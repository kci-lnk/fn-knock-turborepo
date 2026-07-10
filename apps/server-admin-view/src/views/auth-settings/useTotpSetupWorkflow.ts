import { computed, ref, type Ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "../../lib/api";
import type { AuthAccount, TOTPCredential } from "../../types";

type Translate = (key: string, params?: Record<string, unknown>) => string;
type SetupData = { secret: string; uri: string };

interface UseTotpSetupWorkflowOptions {
  credentials: Ref<TOTPCredential[]>;
  onReopenAuthModeSwitch: () => Promise<void>;
  refreshStatus: () => Promise<unknown>;
  replaceAuthAccount: (account: AuthAccount) => void;
  translate: Translate;
}

export function useTotpSetupWorkflow({
  credentials,
  onReopenAuthModeSwitch,
  refreshStatus,
  replaceAuthAccount,
  translate,
}: UseTotpSetupWorkflowOptions) {
  const showSetupDialog = ref(false);
  const setupData = ref<SetupData | null>(null);
  const bindingTotpAccount = ref<AuthAccount | null>(null);
  const reopenAuthModeSwitchAfterTotpBind = ref(false);
  const verifyToken = ref("");
  const newTotpComment = ref("");
  const bindErrorMessage = ref("");
  const setupStep = ref<"BIND" | "NAME">("BIND");
  const setupBindView = ref<"qr" | "manual">("qr");
  const setupBindMotionDirection = ref<"forward" | "back">("forward");
  const boundTotpId = ref<string | null>(null);
  const bindingMode = ref<"bind" | "rename">("bind");

  const { isPending: isBinding, run: runBindingAction } = useAsyncAction({
    onError: (error) => {
      const fallback =
        bindingMode.value === "bind"
          ? translate("admin.authSettings.bindError")
          : translate("admin.authSettings.renameError");
      bindErrorMessage.value = extractErrorMessage(error, fallback);
      if (bindingMode.value === "bind") {
        verifyToken.value = "";
      }
    },
  });
  const { run: runSetupInit } = useAsyncAction({
    onError: (error) => {
      console.error("Failed to setup TOTP:", error);
      bindErrorMessage.value = translate("admin.authSettings.setupFailed");
      setupData.value = null;
    },
  });

  const setupSecretDisplay = computed(() =>
    setupData.value?.secret.replace(/\s+/g, "").toUpperCase().match(/.{1,4}/g)?.join(" ") || "",
  );
  const setupDialogTitle = computed(() =>
    bindingTotpAccount.value
      ? translate("admin.authSettings.accountTotpBindDialogTitle")
      : translate("admin.authSettings.bindDialogTitle"),
  );
  const setupDialogDescription = computed(() =>
    bindingTotpAccount.value
      ? translate("admin.authSettings.accountTotpBindDialogDescription", {
          username: bindingTotpAccount.value.username,
        })
      : translate("admin.authSettings.bindDialogDescription"),
  );
  const setupBindTransitionEnterFromClass = computed(() =>
    setupBindMotionDirection.value === "forward"
      ? "translate-x-4 opacity-0"
      : "-translate-x-4 opacity-0",
  );
  const setupBindTransitionLeaveToClass = computed(() =>
    setupBindMotionDirection.value === "forward"
      ? "-translate-x-4 opacity-0"
      : "translate-x-4 opacity-0",
  );

  function resetSetupState() {
    setupData.value = null;
    bindingTotpAccount.value = null;
    reopenAuthModeSwitchAfterTotpBind.value = false;
    verifyToken.value = "";
    bindErrorMessage.value = "";
    setupStep.value = "BIND";
    setupBindView.value = "qr";
    setupBindMotionDirection.value = "forward";
    boundTotpId.value = null;
  }

  async function openSetupDialog() {
    resetSetupState();
    newTotpComment.value = "";
    showSetupDialog.value = true;
    await runSetupInit(async () => {
      setupData.value = await ConfigAPI.setupTOTP();
    });
  }

  async function openAccountTotpSetupDialog(
    account: AuthAccount,
    reopenSwitchAfterBind = false,
  ) {
    resetSetupState();
    bindingTotpAccount.value = account;
    reopenAuthModeSwitchAfterTotpBind.value = reopenSwitchAfterBind;
    newTotpComment.value = account.username;
    showSetupDialog.value = true;
    await runSetupInit(async () => {
      setupData.value = await ConfigAPI.setupAuthAccountTOTP(account.id);
    });
  }

  function handleCancelSetup() {
    resetSetupState();
  }

  function openManualSetupView() {
    setupBindMotionDirection.value = "forward";
    setupBindView.value = "manual";
  }

  function returnQRCodeSetupView() {
    setupBindMotionDirection.value = "back";
    setupBindView.value = "qr";
  }

  async function copySetupSecret() {
    const secret = setupData.value?.secret;
    if (!secret) return;

    try {
      const result = await copyTextToClipboard(secret);
      if (result.verified) {
        toast.success(translate("admin.authSettings.setupSecretCopied"));
        return;
      }
      toast.info(translate("admin.authSettings.setupSecretCopyUnverified"), {
        description: translate(
          "admin.authSettings.setupSecretCopyUnverifiedDescription",
        ),
      });
    } catch (error) {
      console.error("copySetupSecret:", error);
      toast.error(translate("admin.authSettings.setupSecretCopyFailed"), {
        description: translate("admin.authSettings.setupSecretManualCopyHint"),
      });
    }
  }

  async function handleBind() {
    const setup = setupData.value;
    if (!setup || verifyToken.value.length !== 6) return;

    bindingMode.value = "bind";
    bindErrorMessage.value = "";
    await runBindingAction(async () => {
      const account = bindingTotpAccount.value;
      if (account) {
        const updated = await ConfigAPI.bindAuthAccountTOTP(
          account.id,
          setup.secret,
          verifyToken.value,
        );
        const shouldReopenSwitch = reopenAuthModeSwitchAfterTotpBind.value;
        replaceAuthAccount(updated);
        await refreshStatus();
        showSetupDialog.value = false;
        bindingTotpAccount.value = null;
        reopenAuthModeSwitchAfterTotpBind.value = false;
        toast.success(translate("admin.authSettings.accountTotpBound"));
        if (shouldReopenSwitch) {
          await onReopenAuthModeSwitch();
        }
        return;
      }

      const randomSuffix = Math.random().toString(36).substring(2, 8);
      const randomName =
        translate("admin.authSettings.randomDevicePrefix") + randomSuffix;
      await ConfigAPI.bindTOTP(setup.secret, verifyToken.value, randomName);
      await refreshStatus();

      const newCredential = credentials.value.find(
        (credential) => credential.comment === randomName,
      );
      if (newCredential) {
        boundTotpId.value = newCredential.id;
        newTotpComment.value = randomName;
        setupStep.value = "NAME";
      } else {
        showSetupDialog.value = false;
      }
    });
  }

  async function handleSaveSetupName() {
    if (!newTotpComment.value.trim()) {
      bindErrorMessage.value = translate("admin.authSettings.commentRequired");
      return;
    }
    if (
      credentials.value.some(
        (credential) =>
          credential.comment === newTotpComment.value &&
          credential.id !== boundTotpId.value,
      )
    ) {
      bindErrorMessage.value = translate(
        "admin.authSettings.commentDuplicateDetailed",
      );
      return;
    }
    const totpId = boundTotpId.value;
    if (!totpId) return;

    bindingMode.value = "rename";
    bindErrorMessage.value = "";
    await runBindingAction(async () => {
      await ConfigAPI.updateTOTPComment(totpId, newTotpComment.value);
      showSetupDialog.value = false;
      await refreshStatus();
      toast.success(translate("admin.authSettings.deviceSaved"));
    });
  }

  return {
    bindErrorMessage,
    copySetupSecret,
    handleBind,
    handleCancelSetup,
    handleSaveSetupName,
    isBinding,
    newTotpComment,
    openAccountTotpSetupDialog,
    openManualSetupView,
    openSetupDialog,
    returnQRCodeSetupView,
    setupBindMotionDirection,
    setupBindTransitionEnterFromClass,
    setupBindTransitionLeaveToClass,
    setupBindView,
    setupData,
    setupDialogDescription,
    setupDialogTitle,
    setupSecretDisplay,
    setupStep,
    showSetupDialog,
    verifyToken,
  };
}
