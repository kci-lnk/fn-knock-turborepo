import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { isProtocolMappingVisible } from "../src/lib/protocol-mapping-visibility";
import type { AppConfig } from "../src/types";

const config = (overrides: Partial<AppConfig> = {}): AppConfig =>
  ({
    host_mapping_grouped_view: false,
    host_mapping_groups: [],
    host_mappings: [],
    protocol_mapping_feature: {
      availability: null,
      enabled: false,
    },
    run_type: 3,
    stream_mappings: [],
    ...overrides,
  }) as AppConfig;

describe("protocol mapping repair visibility", () => {
  it("keeps the repair page visible for a persisted startup issue without mappings", () => {
    assert.equal(
      isProtocolMappingVisible(
        config({
          protocol_mapping_feature: {
            availability: null,
            enabled: false,
            runtime_issue: {
              code: "runtime_sync_failed",
              listen_port: null,
              message: "legacy stream policy rejected",
              protocol: null,
              target: null,
            },
          },
        }),
      ),
      true,
    );
  });

  it("still hides an unused disabled feature and every non-subdomain mode", () => {
    assert.equal(isProtocolMappingVisible(config()), false);
    assert.equal(
      isProtocolMappingVisible(
        config({
          run_type: 1,
          stream_mappings: [
            {
              listen_port: 9000,
              protocol: "tcp",
              target: "127.0.0.1:9001",
              use_auth: true,
            },
          ],
        }),
      ),
      false,
    );
  });
});
