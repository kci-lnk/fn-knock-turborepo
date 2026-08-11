import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type {
  WOLDiscoveryPollEvent,
  WOLTarget,
} from "../src/lib/api/wol";
import {
  createWolLocalRelayInput,
  createWolTargetInput,
  reduceWolDiscoveryEvent,
  updatePendingIds,
  wolTargetToEditInput,
} from "../src/views/wol-management/wol-management-model";

describe("Wake-on-LAN management model", () => {
  it("creates secret-free forms with explicit delivery defaults", () => {
    assert.deepEqual(createWolTargetInput("Office PC"), {
      name: "Office PC",
      mac: "",
      relayId: null,
      broadcastAddress: null,
      ipAddress: null,
      enabled: true,
      integrations: undefined,
    });
    assert.equal(createWolLocalRelayInput().psk, "");
  });

  it("converges legacy dual-provider targets to one provider on edit", () => {
    const target = {
      id: "target-1",
      name: "Office PC",
      mac: "00:11:22:33:44:55",
      relayId: "relay-1",
      broadcastAddress: "192.0.2.255",
      ipAddress: "192.0.2.10",
      enabled: true,
      integrations: {
        blinker: {
          enabled: true,
          bindComponent: "switch",
          credentialConfigured: true,
          runtime: {},
        },
        bemfa: {
          enabled: true,
          topic: "office-pc",
          credentialConfigured: true,
          runtime: {},
        },
      },
    } as unknown as WOLTarget;

    const edit = wolTargetToEditInput(target);
    assert.equal(edit.integrations?.blinker.enabled, true);
    assert.equal(edit.integrations?.bemfa.enabled, false);
    assert.equal(edit.integrations?.blinker.deviceKey, "");
    assert.equal(edit.integrations?.bemfa.privateKey, "");
  });

  it("deduplicates streamed devices and keeps their IPs naturally sorted", () => {
    const meta = {
      type: "meta",
      data: {
        networks: [{ cidr: "192.0.2.0/24" }],
        progress: { scanned: 0, total: 10 },
      },
    } as unknown as WOLDiscoveryPollEvent;
    let state = reduceWolDiscoveryEvent(
      { progress: null, result: null },
      meta,
    );
    for (const [mac, ip] of [
      ["00:00:00:00:00:10", "192.0.2.10"],
      ["00:00:00:00:00:02", "192.0.2.2"],
      ["00:00:00:00:00:10", "192.0.2.3"],
    ]) {
      state = reduceWolDiscoveryEvent(state, {
        type: "device",
        data: { mac, ip, broadcastAddress: "192.0.2.255" },
      } as unknown as WOLDiscoveryPollEvent);
    }

    assert.deepEqual(
      state.result?.devices.map((device) => [device.mac, device.ip]),
      [
        ["00:00:00:00:00:02", "192.0.2.2"],
        ["00:00:00:00:00:10", "192.0.2.3"],
      ],
    );
    assert.strictEqual(
      reduceWolDiscoveryEvent(state, { type: "cancelled" }),
      state,
    );
  });

  it("updates pending IDs without mutating the prior set", () => {
    const current = new Set(["one"]);
    const added = updatePendingIds(current, "two", true);
    const removed = updatePendingIds(added, "one", false);

    assert.deepEqual([...current], ["one"]);
    assert.deepEqual([...added], ["one", "two"]);
    assert.deepEqual([...removed], ["two"]);
  });
});
