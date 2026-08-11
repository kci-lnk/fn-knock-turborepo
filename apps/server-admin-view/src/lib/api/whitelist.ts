import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";

import { apiClient } from "./client";

type WhitelistSchemas = ApiContractComponents["schemas"];

export type WhiteListRecord = WhitelistSchemas["WhitelistRecordData"];
export type WhitelistRegionInput =
  WhitelistSchemas["WhitelistRegionInputData"];
export type WhitelistRegionGroupRecord =
  WhitelistSchemas["WhitelistRegionGroupData"];
export type WhitelistRegionAddResult =
  WhitelistSchemas["WhitelistRegionAddResultData"];

type WhitelistAddBody = WhitelistSchemas["WhitelistAddBodyData"];
type WhitelistRegionAddBody =
  WhitelistSchemas["WhitelistRegionAddBodyData"];
type WhitelistCommentBody =
  WhitelistSchemas["WhitelistCommentBodyData"];

type WhitelistRecordsResponse =
  ApiContractOperations["get_api_admin_whitelist"]["responses"][200]["content"]["application/json"];
type WhitelistAddResponse =
  ApiContractOperations["post_api_admin_whitelist"]["responses"][200]["content"]["application/json"];
type WhitelistRegionsResponse =
  ApiContractOperations["get_api_admin_whitelist_regions"]["responses"][200]["content"]["application/json"];
type WhitelistRegionAddResponse =
  ApiContractOperations["post_api_admin_whitelist_regions"]["responses"][200]["content"]["application/json"];
type WhitelistRegionDeleteResponse =
  ApiContractOperations["delete_api_admin_whitelist_regions__id_"]["responses"][200]["content"]["application/json"];
type WhitelistDeleteResponse =
  ApiContractOperations["delete_api_admin_whitelist__id_"]["responses"][200]["content"]["application/json"];
type WhitelistCommentResponse =
  ApiContractOperations["patch_api_admin_whitelist__id__comment"]["responses"][200]["content"]["application/json"];
type WhitelistRefreshResponse =
  ApiContractOperations["post_api_admin_whitelist__id__refresh"]["responses"][200]["content"]["application/json"];

export const WhitelistAPI = {
  async getRecords(): Promise<WhitelistRecordsResponse> {
    const response = await apiClient.get("/whitelist");
    return response.data;
  },
  async getRegions(): Promise<WhitelistRegionsResponse> {
    const response = await apiClient.get("/whitelist/regions");
    return response.data;
  },
  async addRecord(payload: WhitelistAddBody): Promise<WhitelistAddResponse> {
    const response = await apiClient.post("/whitelist", payload);
    return response.data;
  },
  async addRegions(
    payload: WhitelistRegionAddBody,
  ): Promise<WhitelistRegionAddResponse> {
    const response = await apiClient.post("/whitelist/regions", payload);
    return response.data;
  },
  async deleteRegion(id: string): Promise<WhitelistRegionDeleteResponse> {
    const response = await apiClient.delete(
      `/whitelist/regions/${encodeURIComponent(id)}`,
    );
    return response.data;
  },
  async deleteRecord(id: string): Promise<WhitelistDeleteResponse> {
    const response = await apiClient.delete(
      `/whitelist/${encodeURIComponent(id)}`,
    );
    return response.data;
  },
  async updateComment(
    id: string,
    comment: string,
  ): Promise<WhitelistCommentResponse> {
    const payload = { comment } satisfies WhitelistCommentBody;
    const response = await apiClient.patch(
      `/whitelist/${encodeURIComponent(id)}/comment`,
      payload,
    );
    return response.data;
  },
  async refreshRecord(id: string): Promise<WhitelistRefreshResponse> {
    const response = await apiClient.post(
      `/whitelist/${encodeURIComponent(id)}/refresh`,
    );
    return response.data;
  },
};
