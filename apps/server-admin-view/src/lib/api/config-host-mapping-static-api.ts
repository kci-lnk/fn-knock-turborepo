import type { components as ApiContractComponents } from "@fn-knock/api-contract";
import { apiClient } from "./client";

type StaticPathProbeBody =
  ApiContractComponents["schemas"]["StaticPathProbeBodyData"];
export type StaticPathProbeTargetType = StaticPathProbeBody["target_type"];
export type StaticPathProbeResult =
  ApiContractComponents["schemas"]["StaticPathProbeResultData"];
export type StaticPathProbeErrorCode = Exclude<
  StaticPathProbeResult["error_code"],
  null
>;

type StaticPathBrowseBody =
  ApiContractComponents["schemas"]["StaticPathBrowseBodyData"];
export type StaticPathBrowseResult =
  ApiContractComponents["schemas"]["StaticPathBrowseResultData"];
export type StaticPathBrowseEntry = StaticPathBrowseResult["entries"][number];

export const configHostMappingStaticApi = {
  async browseHostMappingStaticPath(
    targetType: StaticPathProbeTargetType,
    path: string | null = null,
    cursor: string | null = null,
  ): Promise<StaticPathBrowseResult> {
    const body = {
      target_type: targetType,
      path,
      cursor,
    } satisfies StaticPathBrowseBody;
    const res = await apiClient.post(
      "/config/host_mappings/static_path_browse",
      body,
    );
    return res.data.data;
  },
  async probeHostMappingStaticPath(
    targetType: StaticPathProbeTargetType,
    path: string,
  ): Promise<StaticPathProbeResult> {
    const body = {
      target_type: targetType,
      path,
    } satisfies StaticPathProbeBody;
    const res = await apiClient.post(
      "/config/host_mappings/static_path_probe",
      body,
    );
    return res.data.data;
  },
};
