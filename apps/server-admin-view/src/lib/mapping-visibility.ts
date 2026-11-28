import { parseCidrTextarea } from "@admin-shared/utils/cidr";
import type { HostVisibilityMode } from "@/types";

export type MappingVisibilityValidationIssue =
  { kind: "invalid_cidrs"; invalid: string[] } | { kind: "empty" } | null;

export const getMappingVisibilityValidationIssue = ({
  available,
  mode,
  selectionsCount,
  customCidrsText,
}: {
  available: boolean;
  mode: HostVisibilityMode;
  selectionsCount: number;
  customCidrsText: string;
}): MappingVisibilityValidationIssue => {
  if (!available || mode !== "custom") return null;
  const parsed = parseCidrTextarea(customCidrsText);
  if (parsed.invalid.length > 0) {
    return { kind: "invalid_cidrs", invalid: parsed.invalid };
  }
  if (selectionsCount === 0 && parsed.cidrs.length === 0) {
    return { kind: "empty" };
  }
  return null;
};
