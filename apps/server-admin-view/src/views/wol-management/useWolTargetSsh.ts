import { ref, type Ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { WOLAPI, type WOLTargetInput } from "@/lib/api/wol";
import type { WolTranslate } from "./wol-management-types";

export const useWolTargetSsh = ({
  editingTargetId,
  t,
  targetForm,
}: {
  editingTargetId: Ref<string>;
  t: WolTranslate;
  targetForm: WOLTargetInput;
}) => {
  const testingSsh = ref(false);

  const testSsh = async () => {
    const ssh = targetForm.ssh;
    if (!ssh || !editingTargetId.value) return;
    const targetId = editingTargetId.value;
    const testedDraft = { ...ssh };
    testingSsh.value = true;
    try {
      const result = await WOLAPI.testSsh(targetId, testedDraft);
      const current = targetForm.ssh;
      const testedFields = [
        "enabled",
        "host",
        "port",
        "username",
        "platform",
        "authMethod",
        "password",
        "privateKey",
        "privateKeyPassphrase",
        "clearCredential",
      ] as const;
      if (
        editingTargetId.value !== targetId ||
        !current ||
        testedFields.some((field) => current[field] !== testedDraft[field])
      ) {
        return;
      }
      current.hostKeyAlgorithm = result.hostKeyAlgorithm;
      current.hostKeyFingerprint = result.hostKeyFingerprint;
      toast.success(t("admin.wol.ssh.testSuccess"), {
        description: t("admin.wol.ssh.testSuccessDescription", {
          latency: result.latencyMs,
        }),
      });
    } catch (error) {
      toast.error(t("admin.wol.ssh.testFailed"), {
        description: extractErrorMessage(error, t("admin.wol.ssh.testFailed")),
      });
    } finally {
      testingSsh.value = false;
    }
  };

  return {
    testSsh,
    testingSsh,
  };
};
