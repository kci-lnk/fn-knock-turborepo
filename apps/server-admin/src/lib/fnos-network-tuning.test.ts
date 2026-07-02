import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  FnosNetworkTuningService,
  type CommandResult,
} from "./fnos-network-tuning";
import type { FnosNetworkTuningConfig } from "./redis";
import type { RuntimeProfile } from "./runtime-profile";

const managedPath = "/tmp/99-fn-knock-network.conf";

const createConfig = (
  patch: Partial<FnosNetworkTuningConfig> = {},
): FnosNetworkTuningConfig => ({
  bbr_enabled: false,
  mtu_probing_enabled: false,
  previous_tcp_congestion_control: null,
  previous_default_qdisc: null,
  previous_tcp_mtu_probing: null,
  updated_at: null,
  last_error: null,
  ...patch,
});

type HarnessOptions = {
  config?: FnosNetworkTuningConfig;
  availableCongestion?: string;
  failAssignments?: Set<string>;
  modprobeFails?: boolean;
};

const fpkRootProfile: RuntimeProfile = {
  deployment_target: "fpk",
  is_docker: false,
  is_linux: true,
  is_root_process: true,
};

const createHarness = (options: HarnessOptions = {}) => {
  let config = createConfig(options.config);
  const files = new Map<string, string>();
  const modules = new Set<string>();
  const sysctl = new Map<string, string>([
    ["net.ipv4.tcp_congestion_control", "cubic"],
    [
      "net.ipv4.tcp_available_congestion_control",
      options.availableCongestion ?? "reno cubic bbr",
    ],
    ["net.core.default_qdisc", "pfifo_fast"],
    ["net.ipv4.tcp_mtu_probing", "0"],
  ]);
  const commands: string[] = [];
  const events: string[] = [];

  const commandResult = (
    code: number,
    stdout = "",
    stderr = "",
  ): CommandResult => ({ code, stdout, stderr });

  const service = new FnosNetworkTuningService({
    managedConfigPath: managedPath,
    getRuntimeProfile: () => fpkRootProfile,
    configAccess: {
      getConfig: async () => ({ ...config }),
      updateConfig: async (patch) => {
        events.push("config-update");
        config = {
          ...config,
          ...patch,
          updated_at: patch.updated_at ?? "2026-07-03T00:00:00.000Z",
        };
        return { ...config };
      },
    },
    runCommand: async (command, args) => {
      commands.push([command, ...args].join(" "));
      events.push(`cmd:${[command, ...args].join(" ")}`);

      if (command === "modprobe" && args[0] === "tcp_bbr") {
        if (options.modprobeFails) {
          return commandResult(1, "", "modprobe failed");
        }
        modules.add("tcp_bbr");
        if (!sysctl.get("net.ipv4.tcp_available_congestion_control")) {
          sysctl.set("net.ipv4.tcp_available_congestion_control", "bbr");
        }
        return commandResult(0);
      }

      if (command !== "sysctl") {
        return commandResult(127, "", "not found");
      }

      if (args[0] === "-n") {
        const value = sysctl.get(args[1] ?? "");
        return value === undefined
          ? commandResult(1, "", "unknown key")
          : commandResult(0, `${value}\n`);
      }

      if (args[0] === "-w") {
        const [key, value] = String(args[1] ?? "").split("=");
        if (!key || value === undefined) {
          return commandResult(1, "", "bad assignment");
        }
        if (
          key === "net.ipv4.tcp_congestion_control" &&
          value === "bbr" &&
          !String(
            sysctl.get("net.ipv4.tcp_available_congestion_control") ?? "",
          )
            .split(/\s+/)
            .includes("bbr")
        ) {
          return commandResult(1, "", "bbr unavailable");
        }
        if (options.failAssignments?.has(`${key}=${value}`)) {
          return commandResult(1, "", "assignment rejected");
        }
        sysctl.set(key, value);
        return commandResult(0, `${key} = ${value}\n`);
      }

      return commandResult(1, "", "unsupported sysctl args");
    },
    readFile: async (path) => {
      if (path === "/proc/modules") {
        return [...modules].map((name) => `${name} 20480 0 - Live 0\n`).join("");
      }
      const content = files.get(path);
      if (content === undefined) {
        throw new Error("missing file");
      }
      return content;
    },
    writeFile: async (path, content) => {
      files.set(path, content);
    },
    rename: async (oldPath, newPath) => {
      const content = files.get(oldPath);
      if (content === undefined) {
        throw new Error("missing temp file");
      }
      files.set(newPath, content);
      files.delete(oldPath);
      if (newPath === managedPath) {
        events.push("managed-write");
      }
    },
    mkdir: async () => {},
    rm: async (path) => {
      files.delete(path);
      if (path === managedPath) {
        events.push("managed-remove");
      }
    },
  });

  return {
    commands,
    events,
    files,
    get config() {
      return config;
    },
    service,
    sysctl,
  };
};

describe("FnosNetworkTuningService", () => {
  it("enables BBR and records previous sysctl values", async () => {
    const harness = createHarness();

    const status = await harness.service.update({ bbr_enabled: true });

    assert.equal(harness.sysctl.get("net.ipv4.tcp_congestion_control"), "bbr");
    assert.equal(harness.sysctl.get("net.core.default_qdisc"), "fq");
    assert.equal(harness.config.bbr_enabled, true);
    assert.equal(harness.config.previous_tcp_congestion_control, "cubic");
    assert.equal(harness.config.previous_default_qdisc, "pfifo_fast");
    assert.equal(status.bbr.active, true);
    assert.match(
      harness.files.get(managedPath) ?? "",
      /net\.ipv4\.tcp_congestion_control=bbr/,
    );
  });

  it("writes the managed sysctl file after saving app config", async () => {
    const harness = createHarness();

    await harness.service.update({ bbr_enabled: true });

    const runtimeIndex = harness.events.findIndex((event) =>
      event.includes("sysctl -w net.core.default_qdisc=fq"),
    );
    const configIndex = harness.events.indexOf("config-update");
    const writeIndex = harness.events.indexOf("managed-write");

    assert.notEqual(runtimeIndex, -1);
    assert.notEqual(configIndex, -1);
    assert.notEqual(writeIndex, -1);
    assert.ok(runtimeIndex < configIndex);
    assert.ok(configIndex < writeIndex);
  });

  it("serializes concurrent updates so features do not overwrite each other", async () => {
    const harness = createHarness();

    await Promise.all([
      harness.service.update({ bbr_enabled: true }),
      harness.service.update({ mtu_probing_enabled: true }),
    ]);

    assert.equal(harness.config.bbr_enabled, true);
    assert.equal(harness.config.mtu_probing_enabled, true);
    const content = harness.files.get(managedPath) ?? "";
    assert.match(content, /net\.ipv4\.tcp_congestion_control=bbr/);
    assert.match(content, /net\.ipv4\.tcp_mtu_probing=1/);
  });

  it("rejects BBR when the kernel does not expose tcp_bbr", async () => {
    const harness = createHarness({ availableCongestion: "reno cubic" });

    await assert.rejects(
      () => harness.service.update({ bbr_enabled: true }),
      /tcp_bbr/,
    );

    assert.equal(harness.config.bbr_enabled, false);
    assert.match(harness.config.last_error ?? "", /tcp_bbr/);
    assert.equal(harness.files.has(managedPath), false);
    assert.equal(harness.sysctl.get("net.ipv4.tcp_congestion_control"), "cubic");
  });

  it("enables and disables MTU probing with previous value restoration", async () => {
    const harness = createHarness();

    await harness.service.update({ mtu_probing_enabled: true });
    assert.equal(harness.sysctl.get("net.ipv4.tcp_mtu_probing"), "1");
    assert.equal(harness.config.previous_tcp_mtu_probing, "0");
    assert.match(
      harness.files.get(managedPath) ?? "",
      /net\.ipv4\.tcp_mtu_probing=1/,
    );

    await harness.service.update({ mtu_probing_enabled: false });
    assert.equal(harness.sysctl.get("net.ipv4.tcp_mtu_probing"), "0");
    assert.equal(harness.config.mtu_probing_enabled, false);
    assert.equal(harness.files.has(managedPath), false);
  });

  it("keeps managed config rendering idempotent", async () => {
    const harness = createHarness();

    await harness.service.update({ bbr_enabled: true });
    await harness.service.update({ bbr_enabled: true });

    const content = harness.files.get(managedPath) ?? "";
    assert.equal(
      content.match(/net\.ipv4\.tcp_congestion_control=bbr/g)?.length,
      1,
    );
    assert.equal(content.match(/net\.core\.default_qdisc=fq/g)?.length, 1);
  });

  it("disables BBR with safe fallbacks when no previous values exist", async () => {
    const harness = createHarness({
      config: createConfig({ bbr_enabled: true }),
    });
    harness.sysctl.set("net.ipv4.tcp_congestion_control", "bbr");
    harness.sysctl.set("net.core.default_qdisc", "fq");

    await harness.service.update({ bbr_enabled: false });

    assert.equal(harness.sysctl.get("net.ipv4.tcp_congestion_control"), "cubic");
    assert.equal(harness.sysctl.get("net.core.default_qdisc"), "pfifo_fast");
    assert.equal(harness.config.bbr_enabled, false);
    assert.equal(harness.files.has(managedPath), false);
  });

  it("falls back when previous BBR values are no longer accepted", async () => {
    const harness = createHarness({
      config: createConfig({
        bbr_enabled: true,
        previous_tcp_congestion_control: "reno",
        previous_default_qdisc: "cake",
      }),
      failAssignments: new Set([
        "net.ipv4.tcp_congestion_control=reno",
        "net.core.default_qdisc=cake",
      ]),
    });
    harness.sysctl.set("net.ipv4.tcp_congestion_control", "bbr");
    harness.sysctl.set("net.core.default_qdisc", "fq");

    await harness.service.update({ bbr_enabled: false });

    assert.equal(harness.sysctl.get("net.ipv4.tcp_congestion_control"), "cubic");
    assert.equal(harness.sysctl.get("net.core.default_qdisc"), "pfifo_fast");
    assert.equal(harness.config.bbr_enabled, false);
  });
});
