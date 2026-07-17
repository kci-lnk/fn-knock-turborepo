import { computed, type Ref } from "vue";
import { toast } from "@admin-shared/utils/toast";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import { buildDDNSTimestampTooltipLines } from "../../lib/ddns-time";
import type { DDNSTargetSummaryPayload } from "../../lib/api";
import type { LastCheck, LastIP } from "./model";

type Translate = (
  key: string,
  params?: Record<string, string | number>,
) => string;

interface UseDDNSStatusPresentationOptions {
  lastCheck: Ref<LastCheck>;
  lastIP: Ref<LastIP>;
  locale: Ref<string>;
  translate: Translate;
  updateIntervalMinutes: Ref<number>;
}

export function useDDNSStatusPresentation({
  lastCheck,
  lastIP,
  locale,
  translate,
  updateIntervalMinutes,
}: UseDDNSStatusPresentationOptions) {
  const timestampLabels = () => ({
    lastSuccessfulUpdate: translate("admin.ddns.lastSuccessfulUpdate"),
    lastCheck: translate("admin.ddns.lastCheck"),
    never: translate("admin.ddns.never"),
  });

  const getTargetLastCheckTooltipLines = (
    target: DDNSTargetSummaryPayload,
  ) =>
    buildDDNSTimestampTooltipLines({
      updatedAt: target.lastIP.updated_at,
      checkedAt: target.lastCheck.checked_at,
      locale: String(locale.value),
      labels: timestampLabels(),
    });

  const lastCheckTooltipLines = computed(() =>
    buildDDNSTimestampTooltipLines({
      updatedAt: lastIP.value.updated_at,
      checkedAt: lastCheck.value.checked_at,
      locale: String(locale.value),
      labels: timestampLabels(),
    }),
  );

  const updateIntervalLabel = computed(() =>
    translate("admin.ddns.updateIntervalLabel", {
      minutes: updateIntervalMinutes.value,
    }),
  );

  const copyIpAddress = async (
    versionLabel: "IPv4" | "IPv6",
    value: string | null,
  ) => {
    const address = value?.trim();
    if (!address) {
      toast.error(
        translate("admin.ddns.copyUnavailable", { version: versionLabel }),
      );
      return;
    }

    try {
      await copyTextToClipboard(address);
      toast.success(
        translate("admin.ddns.copySuccess", { version: versionLabel }),
        { description: address },
      );
    } catch (error) {
      console.error("copyIpAddress:", error);
      toast.error(
        translate("admin.ddns.copyFailed", { version: versionLabel }),
        { description: translate("admin.ddns.copyFailedDescription") },
      );
    }
  };

  return {
    copyIpAddress,
    getTargetLastCheckTooltipLines,
    lastCheckTooltipLines,
    updateIntervalLabel,
  };
}
