import { ref, type Ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "@/lib/api";
import type { SSLSharedFilesPayload } from "@/types";

type TranslationParams = Record<string, string | number>;

type CertFormData = {
  cert: string;
  key: string;
};

const defaultSSLSharedFiles: SSLSharedFilesPayload = {
  shareName: "fn-knock",
  available: false,
  files: [],
};

export const useSSLSharedFiles = ({
  formData,
  translate,
}: {
  formData: Ref<CertFormData>;
  translate: (key: string, params?: TranslationParams) => string;
}) => {
  const sharedFilesError = ref("");
  const sslSharedFiles = ref<SSLSharedFilesPayload>(defaultSSLSharedFiles);
  const hasLoadedSharedFiles = ref(false);

  const { isPending: isLoadingSharedFiles, run: runLoadSharedFiles } =
    useAsyncAction({
      onError: (error) => {
        const message = extractErrorMessage(
          error,
          translate("admin.certConfig.loadSharedDirFailed"),
        );
        sharedFilesError.value = message;
        toast.error(message);
      },
    });
  const { isPending: isReadingSharedFile, run: runReadSharedFile } =
    useAsyncAction({
      onError: (error) => {
        toast.error(
          extractErrorMessage(
            error,
            translate("admin.certConfig.loadSharedFileFailed"),
          ),
        );
      },
    });

  const loadSharedFiles = async (force = false) => {
    if (hasLoadedSharedFiles.value && !force) return;

    sharedFilesError.value = "";
    const nextFiles = await runLoadSharedFiles(async () =>
      ConfigAPI.getSSLSharedFiles(),
    );
    if (!nextFiles) return;

    sslSharedFiles.value = nextFiles;
    hasLoadedSharedFiles.value = true;
  };

  const handleSharedFilesRequest = async (payload: {
    field: "cert" | "sslKey";
    force?: boolean;
  }) => {
    await loadSharedFiles(Boolean(payload.force));
  };

  const applySharedFileSelection = async (
    target: CertFormData,
    payload: { field: "cert" | "sslKey"; relativePath: string },
  ) => {
    const result = await runReadSharedFile(async () =>
      ConfigAPI.readSSLSharedFile(payload.relativePath),
    );
    if (!result) return;

    if (payload.field === "cert") {
      target.cert = result.content;
    } else {
      target.key = result.content;
    }

    const label =
      payload.field === "cert"
        ? translate("admin.certConfig.certificateFile")
        : translate("admin.certConfig.privateKeyFile");
    toast.success(
      translate("admin.certConfig.sharedFileLoaded", {
        label,
        file: result.file.name,
      }),
    );
  };

  const handleCreateSharedFileSelect = async (payload: {
    field: "cert" | "sslKey";
    relativePath: string;
  }) => {
    await applySharedFileSelection(formData.value, payload);
  };

  return {
    handleCreateSharedFileSelect,
    handleSharedFilesRequest,
    isLoadingSharedFiles,
    isReadingSharedFile,
    sharedFilesError,
    sslSharedFiles,
  };
};
