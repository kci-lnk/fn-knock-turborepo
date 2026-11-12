import type { Ref } from "vue";
import type { AppConfig, StreamMapping } from "../types";
import { ConfigAPI } from "@/lib/api/config";
import { SystemAPI } from "@/lib/api/system";
import {
  STREAM_MAPPING_LEGACY_REPAIR_REQUIRED_CODE,
  STREAM_MAPPING_LIST_CHANGED_CODE,
} from "@/lib/api/config-stream-api";
import { streamMappingWritePayload } from "../lib/stream-mappings";
import { createSerialTaskQueue } from "../lib/serialTaskQueue";

const isStreamMappingConflict = (error: unknown, code: number): boolean =>
  (error as { response?: { status?: number; data?: { code?: number } } })
    ?.response?.status === 409 &&
  (error as { response?: { data?: { code?: number } } })?.response?.data
    ?.code === code;

export function createStreamMappingsSaver(
  config: Ref<AppConfig | null>,
  reloadConfig: () => Promise<AppConfig | null>,
  markCommitted: () => void,
) {
  const runStreamMappingsSave = createSerialTaskQueue();
  return async function saveStreamMappings(
    update: (current: readonly StreamMapping[]) => StreamMapping[],
    options: { disableFeatureOnLegacyRepairConflict?: boolean } = {},
  ) {
    return runStreamMappingsSave(async () => {
      let current =
        config.value?.stream_mappings ?? (await ConfigAPI.getStreamMappings());
      let next = update(current);
      let protocolMappingDisabled = false;
      const persist = async () => {
        try {
          await ConfigAPI.updateStreamMappings(
            streamMappingWritePayload(current, next),
            current,
          );
        } catch (error) {
          if (
            isStreamMappingConflict(error, STREAM_MAPPING_LIST_CHANGED_CODE)
          ) {
            await reloadConfig();
          }
          throw error;
        }
      };
      try {
        await persist();
      } catch (error) {
        if (
          !options.disableFeatureOnLegacyRepairConflict ||
          !isStreamMappingConflict(
            error,
            STREAM_MAPPING_LEGACY_REPAIR_REQUIRED_CODE,
          )
        ) {
          throw error;
        }
        await SystemAPI.updateProtocolMappingFeatureConfig({ enabled: false });
        protocolMappingDisabled = true;
        if (config.value) {
          config.value = {
            ...config.value,
            protocol_mapping_feature: {
              ...config.value.protocol_mapping_feature,
              enabled: false,
              availability:
                config.value.protocol_mapping_feature?.availability ?? null,
            },
          };
        }
        const refreshed = await reloadConfig();
        current =
          refreshed?.stream_mappings ?? (await ConfigAPI.getStreamMappings());
        next = update(current);
        await persist();
      }
      markCommitted();
      if (config.value) {
        config.value = {
          ...config.value,
          stream_mappings: next,
          ...(protocolMappingDisabled
            ? {
                protocol_mapping_feature: {
                  ...config.value.protocol_mapping_feature,
                  enabled: false,
                  availability:
                    config.value.protocol_mapping_feature?.availability ?? null,
                },
              }
            : {}),
        };
      }
      await reloadConfig();
      return { protocolMappingDisabled };
    });
  };
}
