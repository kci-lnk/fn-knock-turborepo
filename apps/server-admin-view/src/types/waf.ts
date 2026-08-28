import type { components as ApiContractComponents } from "@fn-knock/api-contract";

type WafSchemas = ApiContractComponents["schemas"];

export type WAFMode = WafSchemas["WafStatusData"]["mode"];
export type WAFConfig = WafSchemas["WafConfigData"];
export type WAFBlockBehavior = WAFConfig["block_behavior"];
export type WAFStatus = WafSchemas["WafStatusData"];

// The gateway validates bundles internally; this remains a transport-neutral
// model until a management HTTP endpoint exposes the result.
export interface WAFValidationResult {
  ok: boolean;
  bundle_id?: string;
  bundle_path?: string;
  bundle_hash?: string;
  error?: string;
}

export type WAFRuleSource = WafSchemas["WafRuleFileData"]["source"];
export type WAFManifestRule = WafSchemas["WafManifestRuleData"];
export type WAFRemoteManifest = WafSchemas["WafRemoteManifestData"];
export type WAFRuleFile = WafSchemas["WafRuleFileData"];
export type WAFRuleFileContent = WafSchemas["WafRuleFileContentData"];
export type WAFSystemSyncState = WafSchemas["WafSystemSyncStateData"];
export type WAFDetails = WafSchemas["WafDetailsData"];
export type WAFMatchedVariable = WafSchemas["WafMatchedVariableData"];
export type WAFRuleMatch = WafSchemas["WafRuleMatchData"];
export type WAFInterruptionInfo = WafSchemas["WafInterruptionData"];
export type WAFEvent = WafSchemas["WafEventData"];
export type WAFDrainResult = WafSchemas["WafDrainResultData"];
export type WAFLogEntriesPayload = WafSchemas["WafLogEntriesData"];
export type WAFLogDeletePayload = WafSchemas["WafLogDeleteData"];
