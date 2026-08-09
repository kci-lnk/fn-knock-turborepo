import assert from "node:assert/strict";
import { beforeEach, describe, it } from "node:test";
import { createPinia, setActivePinia } from "pinia";
import {
  ConfigAPI,
  STREAM_MAPPING_LEGACY_REPAIR_REQUIRED_CODE,
  SystemAPI,
  type RevisionedConfig,
} from "../src/lib/api";
import { useConfigStore } from "../src/store/config";
import type { AppConfig, DailyAvailability, StreamMapping } from "../src/types";

const legacyUdp: StreamMapping = {
  protocol: "udp",
  listen_port: 12333,
  target: "127.0.0.1:12333",
  use_auth: true,
  comment: "legacy UDP",
};
const retainedTcp: StreamMapping = {
  protocol: "tcp",
  listen_port: 24444,
  target: "192.0.2.20:24444",
  use_auth: false,
  comment: "retained",
};
const concurrentTcp: StreamMapping = {
  protocol: "tcp",
  listen_port: 35555,
  target: "192.0.2.30:35555",
  use_auth: false,
  comment: "concurrent",
};

const appConfig = (
  mappings: StreamMapping[],
  enabled: boolean,
  availability: DailyAvailability | null = null,
): AppConfig =>
  ({
    host_mappings: [],
    host_mapping_groups: [],
    host_mapping_grouped_view: false,
    stream_mappings: mappings,
    protocol_mapping_feature: { enabled, availability },
  }) as AppConfig;

const revisioned = (config: AppConfig): RevisionedConfig => ({
  config,
  hostMappingsRevision: null,
  hostMappingCatalogRevision: null,
});

const repairConflict = (code = STREAM_MAPPING_LEGACY_REPAIR_REQUIRED_CODE) =>
  Object.assign(new Error("repair conflict"), {
    response: { status: 409, data: { code } },
  });

const removeLegacyUdp = (current: readonly StreamMapping[]) =>
  current.filter(
    (mapping) =>
      mapping.protocol !== legacyUdp.protocol ||
      mapping.listen_port !== legacyUdp.listen_port,
  );

describe("config store stream mapping repair", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("disables, reloads, rebases, and retries only for the dedicated repair code", async (t) => {
    const store = useConfigStore();
    const availability: DailyAvailability = {
      enabled: true,
      start_time: "22:00",
      end_time: "06:00",
    };
    store.config = appConfig([legacyUdp, retainedTcp], true, availability);
    let updateCalls = 0;
    const updateMock = t.mock.method(
      ConfigAPI,
      "updateStreamMappings",
      async (mappings) => {
        updateCalls += 1;
        if (updateCalls === 1) throw repairConflict();
        assert.deepEqual(mappings, [retainedTcp, concurrentTcp]);
      },
    );
    const disableMock = t.mock.method(
      SystemAPI,
      "updateProtocolMappingFeatureConfig",
      async () => ({ enabled: false, availability }),
    );
    let configReads = 0;
    t.mock.method(ConfigAPI, "getConfig", async () => {
      configReads += 1;
      return revisioned(
        appConfig(
          [
            retainedTcp,
            concurrentTcp,
            ...(configReads === 1 ? [legacyUdp] : []),
          ],
          false,
          availability,
        ),
      );
    });

    const result = await store.saveStreamMappings(removeLegacyUdp, {
      disableFeatureOnLegacyRepairConflict: true,
    });

    assert.deepEqual(result, { protocolMappingDisabled: true });
    assert.equal(updateMock.mock.callCount(), 2);
    assert.equal(disableMock.mock.callCount(), 1);
    assert.equal(configReads, 2);
    assert.deepEqual(store.config?.stream_mappings, [
      retainedTcp,
      concurrentTcp,
    ]);
    assert.equal(store.config?.protocol_mapping_feature?.enabled, false);
    assert.deepEqual(
      store.config?.protocol_mapping_feature?.availability,
      availability,
    );
  });

  it("does not disable protocol mappings for an unrelated 409 response", async (t) => {
    const store = useConfigStore();
    store.config = appConfig([legacyUdp], true);
    t.mock.method(ConfigAPI, "updateStreamMappings", async () => {
      throw repairConflict(40_999);
    });
    const disableMock = t.mock.method(
      SystemAPI,
      "updateProtocolMappingFeatureConfig",
      async () => ({ enabled: false }),
    );

    await assert.rejects(
      store.saveStreamMappings(removeLegacyUdp, {
        disableFeatureOnLegacyRepairConflict: true,
      }),
      /repair conflict/u,
    );

    assert.equal(disableMock.mock.callCount(), 0);
    assert.equal(store.config?.protocol_mapping_feature?.enabled, true);
  });

  it("keeps the local feature disabled when reload and retry fail", async (t) => {
    const store = useConfigStore();
    store.config = appConfig([legacyUdp], true);
    t.mock.method(ConfigAPI, "updateStreamMappings", async () => {
      throw repairConflict();
    });
    const disableMock = t.mock.method(
      SystemAPI,
      "updateProtocolMappingFeatureConfig",
      async () => ({ enabled: false }),
    );
    t.mock.method(ConfigAPI, "getConfig", async () => {
      throw new Error("reload failed");
    });
    t.mock.method(ConfigAPI, "getStreamMappings", async () => {
      throw new Error("retry source failed");
    });
    t.mock.method(console, "error", () => undefined);

    await assert.rejects(
      store.saveStreamMappings(removeLegacyUdp, {
        disableFeatureOnLegacyRepairConflict: true,
      }),
      /retry source failed/u,
    );

    assert.equal(disableMock.mock.callCount(), 1);
    assert.equal(store.config?.protocol_mapping_feature?.enabled, false);
    assert.deepEqual(store.config?.stream_mappings, [legacyUdp]);
  });

  it("keeps the local feature enabled when the disable request fails", async (t) => {
    const store = useConfigStore();
    store.config = appConfig([legacyUdp], true);
    t.mock.method(ConfigAPI, "updateStreamMappings", async () => {
      throw repairConflict();
    });
    t.mock.method(SystemAPI, "updateProtocolMappingFeatureConfig", async () => {
      throw new Error("disable failed");
    });
    const reloadMock = t.mock.method(ConfigAPI, "getConfig", async () =>
      revisioned(appConfig([legacyUdp], true)),
    );

    await assert.rejects(
      store.saveStreamMappings(removeLegacyUdp, {
        disableFeatureOnLegacyRepairConflict: true,
      }),
      /disable failed/u,
    );

    assert.equal(reloadMock.mock.callCount(), 0);
    assert.equal(store.config?.protocol_mapping_feature?.enabled, true);
    assert.deepEqual(store.config?.stream_mappings, [legacyUdp]);
  });
});
