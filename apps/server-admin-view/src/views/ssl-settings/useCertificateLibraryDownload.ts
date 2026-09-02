import { ref } from "vue";
import type { SSLCertificateSummary } from "@/types";
import { ConfigAPI } from "@/lib/api/config";
import { acmeCertificateArchiveFilename } from "@/lib/acme-download";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { downloadBlob } from "@admin-shared/utils/downloadBlob";
import { toast } from "@admin-shared/utils/toast";

const archiveLabel = (certificate: SSLCertificateSummary) =>
  certificate.primary_domain ||
  certificate.certInfo?.dnsNames?.[0] ||
  certificate.label ||
  certificate.id;

export function useCertificateLibraryDownload(
  translate: (key: string) => string,
) {
  const downloadingCertificateId = ref<string | null>(null);
  const { isPending: isDownloading, run: runDownload } = useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(
          error,
          translate("admin.certConfig.downloadFailed"),
        ),
      );
    },
  });

  const downloadCertificate = async (certificate: SSLCertificateSummary) => {
    if (isDownloading.value) return;
    downloadingCertificateId.value = certificate.id;
    try {
      await runDownload(async () => {
        const blob = await ConfigAPI.downloadSSLCertificate(certificate.id);
        downloadBlob(
          blob,
          acmeCertificateArchiveFilename(archiveLabel(certificate)),
        );
      });
    } finally {
      downloadingCertificateId.value = null;
    }
  };

  return {
    downloadCertificate,
    downloadingCertificateId,
    isDownloading,
  };
}
