import { onMounted, type Ref } from "vue";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { ConfigAPI } from "../../lib/api";
import type {
  AuthAccount,
  AuthLoginMode,
  AuthLoginModeStatus,
  HostMapping,
  StreamMapping,
  TOTPCredential,
} from "../../types";

interface UseAuthSettingsResourceOptions {
  authAccounts: Ref<AuthAccount[]>;
  authLoginMode: Ref<AuthLoginMode>;
  authModeStatus: Ref<AuthLoginModeStatus | null>;
  credentials: Ref<TOTPCredential[]>;
  hostMappings: Ref<HostMapping[]>;
  streamMappings: Ref<StreamMapping[]>;
  normalizeAuthAccount: (account: AuthAccount) => AuthAccount;
  normalizeCredential: (credential: TOTPCredential) => TOTPCredential;
  translate: (key: string) => string;
}

export function useAuthSettingsResource({
  authAccounts,
  authLoginMode,
  authModeStatus,
  credentials,
  hostMappings,
  streamMappings,
  normalizeAuthAccount,
  normalizeCredential,
  translate,
}: UseAuthSettingsResourceOptions) {
  const { isPending: isLoading, run: runLoadStatus } = useAsyncAction({
    onError: (error) => {
      console.error("Failed to get TOTP status:", error);
    },
  });
  const showLoadingSkeleton = useDelayedLoading(isLoading);
  const { run: runSaveComment } = useAsyncAction({ rethrow: true });
  const { isPending: isDeleting, run: runDeleteCredential } = useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(error, translate("admin.authSettings.deleteFailed")),
      );
    },
  });

  const fetchStatus = async () => {
    await runLoadStatus(async () => {
      const [res, mappings, streams, modeStatus, accounts] = await Promise.all([
        ConfigAPI.getTOTPStatus(),
        ConfigAPI.getHostMappings()
          .then((snapshot) => snapshot.mappings)
          .catch((error) => {
            console.error("Failed to get host mappings:", error);
            return [] as HostMapping[];
          }),
        ConfigAPI.getStreamMappings().catch((error) => {
          console.error("Failed to get stream mappings:", error);
          return [] as StreamMapping[];
        }),
        ConfigAPI.getAuthLoginMode(),
        ConfigAPI.getAuthAccounts().catch((error) => {
          console.error("Failed to get auth accounts:", error);
          return [] as AuthAccount[];
        }),
      ]);
      hostMappings.value = mappings;
      streamMappings.value = streams;
      credentials.value = (res.credentials || []).map(normalizeCredential);
      authModeStatus.value = modeStatus;
      authLoginMode.value = modeStatus.mode || "totp";
      authAccounts.value = (accounts || []).map(normalizeAuthAccount);
    });
  };

  const saveComment = async (id: string, newText: string) => {
    await runSaveComment(() => ConfigAPI.updateTOTPComment(id, newText), {
      onSuccess: () => {
        const target = credentials.value.find((item) => item.id === id);
        if (target) target.comment = newText;
        toast.success(translate("admin.authSettings.commentUpdated"));
      },
      onError: (error) => {
        throw new Error(
          extractErrorMessage(error, translate("admin.authSettings.renameError")),
        );
      },
    });
  };

  const handleDelete = async (totpId: string) => {
    await runDeleteCredential(async () => {
      await ConfigAPI.deleteTOTP(totpId);
      await fetchStatus();
      toast.success(translate("admin.authSettings.tokenDeleted"));
    });
  };

  const handleDeleteAccount = async (accountId: string) => {
    await runDeleteCredential(async () => {
      await ConfigAPI.deleteAuthAccount(accountId);
      await fetchStatus();
      toast.success(translate("admin.authSettings.accountDeleted"));
    });
  };

  onMounted(fetchStatus);

  return {
    fetchStatus,
    handleDelete,
    handleDeleteAccount,
    isDeleting,
    isLoading,
    saveComment,
    showLoadingSkeleton,
  };
}
