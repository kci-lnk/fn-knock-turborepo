import { getCidrRegionSelectionLabel } from "@/types/cidr";
import type {
  WhiteListRecord,
  WhitelistRegionGroupRecord,
  WhitelistRegionInput,
} from "@/lib/api/whitelist";

export type WhitelistTranslate = (
  key: string,
  params?: Record<string, unknown>,
) => string;

export const getWhitelistResolveStatusLabel = (
  record: WhiteListRecord,
  translate: WhitelistTranslate,
) => {
  switch (record.resolveStatus) {
    case "resolved":
      return translate("admin.ipWhitelist.resolveSuccess");
    case "empty":
      return translate("admin.ipWhitelist.resolveEmpty");
    case "error":
      return translate("admin.ipWhitelist.resolveError");
    default:
      return translate("admin.ipWhitelist.resolvePending");
  }
};

export const getWhitelistTargetTypeLabel = (
  type: WhiteListRecord["targetType"],
) => {
  if (type === "cidr") return "CIDR";
  if (type === "cname") return "CNAME";
  return "IP";
};

export const getWhitelistResolveStatusVariant = (
  record: WhiteListRecord,
): "default" | "secondary" | "destructive" | "outline" => {
  switch (record.resolveStatus) {
    case "resolved":
      return "default";
    case "empty":
      return "secondary";
    case "error":
      return "destructive";
    default:
      return "outline";
  }
};

export const formatWhitelistRemaining = (
  expireAt: number,
  translate: WhitelistTranslate,
  nowSeconds = Math.floor(Date.now() / 1000),
) => {
  const diff = expireAt - nowSeconds;
  if (diff <= 0) return translate("admin.ipWhitelist.expired");

  const days = Math.floor(diff / 86_400);
  const hours = Math.floor((diff % 86_400) / 3_600);
  const minutes = Math.floor((diff % 3_600) / 60);
  const parts: string[] = [];
  if (days > 0) {
    parts.push(translate("admin.ipWhitelist.days", { count: days }));
  }
  if (hours > 0) {
    parts.push(translate("admin.ipWhitelist.hours", { count: hours }));
  }
  if (minutes > 0 || (days === 0 && hours === 0)) {
    parts.push(
      translate("admin.ipWhitelist.minutesCount", { count: minutes }),
    );
  }
  return translate("admin.ipWhitelist.remaining", { value: parts.join("") });
};

export const formatWhitelistRegionInput = (region: WhitelistRegionInput) =>
  getCidrRegionSelectionLabel(region, { includeProvince: true });

export const getWhitelistRegionGroupLabel = (
  group: WhitelistRegionGroupRecord,
) => group.regions.map(formatWhitelistRegionInput).join(", ");
