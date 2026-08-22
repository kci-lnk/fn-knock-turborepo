import type { AppConfig } from "@/types";

export const isProtocolMappingVisible = (
  config: AppConfig | null | undefined,
): boolean =>
  config?.run_type === 3 &&
  (config.protocol_mapping_feature?.enabled === true ||
    config.protocol_mapping_feature?.runtime_issue != null ||
    (config.stream_mappings?.length ?? 0) > 0);
