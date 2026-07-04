import assert from "node:assert/strict";
import test from "node:test";
import type { ScanOptions } from "../plugins/scanner/types";
import {
  DISCOVERY_PORT_COUNT,
  DISCOVERY_LIMITED_PORT_RANGE,
  DISCOVERY_PORT_RANGE,
  LOCAL_SELF_DISCOVERY_SKIP_PORTS,
  buildDiscoveryHostGroups,
  buildDiscoveryPortModeLabel,
  buildDiscoveryScanOptions,
  calculateDiscoveryConcurrency,
  countDiscoveryPorts,
  countLimitedDiscoveryPorts,
  runServiceDiscoveryScan,
  type ServiceDiscoveryScanResult,
  type ServiceDiscoveryScanner,
} from "./service-discovery-scan";

const buildEmptyResult = (
  host: string,
  hosts: readonly string[],
  options?: ScanOptions,
): ServiceDiscoveryScanResult => {
  const skipSet = new Set(options?.skipPorts || []);
  const portsPerHost =
    options?.portRanges?.reduce((total, range) => {
      let count = 0;
      for (let port = range.start; port <= range.end; port += 1) {
        if (!skipSet.has(port)) {
          count += 1;
        }
      }
      return total + count;
    }, 0) ?? 0;

  return {
    host,
    totalPortsScanned: portsPerHost * hosts.length,
    foundServices: 0,
    scannedHosts: hosts.length,
    services: [],
  };
};

const createCapturingScanner = () => {
  const singleCalls: Array<{ host: string; options?: ScanOptions }> = [];
  const manyCalls: Array<{ hosts: string[]; options?: ScanOptions }> = [];
  const scanner: ServiceDiscoveryScanner = {
    async scanAndAnalyze(host, options) {
      singleCalls.push({ host, options });
      return buildEmptyResult(host, [host], options);
    },
    async scanAndAnalyzeMany(hosts, options) {
      manyCalls.push({ hosts, options });
      return buildEmptyResult(hosts[0] || "", hosts, options);
    },
  };

  return { manyCalls, scanner, singleCalls };
};

const assertFullDiscoveryRange = (options?: ScanOptions) => {
  assert.deepEqual(options?.portRanges, [DISCOVERY_PORT_RANGE]);
};

const assertLimitedDiscoveryRange = (options?: ScanOptions) => {
  assert.deepEqual(options?.portRanges, [DISCOVERY_LIMITED_PORT_RANGE]);
};

test("service discovery scan options cover 80-60000 and keep skipped ports", () => {
  const options = buildDiscoveryScanOptions({
    excludePorts: [7991, 7999],
    minimumHostConcurrency: 6,
    minimumMaxConcurrent: 64,
    scanHostCount: 254,
  });

  assert.equal(DISCOVERY_PORT_COUNT, 59921);
  assertFullDiscoveryRange(options);
  assert.deepEqual(options.skipPorts, [7991, 7999]);
  assert.ok((options.maxConcurrent || 0) > 0);
  assert.ok((options.hostConcurrency || 0) > 0);
});

test("service discovery concurrency scales from device profile without dropping below current floors when descriptors allow it", () => {
  const lowProfile = calculateDiscoveryConcurrency({
    deviceProfile: {
      cpuCount: 1,
      freeMemoryMb: 128,
      totalMemoryMb: 256,
    },
    fileDescriptorLimit: 8192,
    minimumHostConcurrency: 6,
    minimumMaxConcurrent: 64,
    scanHostCount: 1024,
  });
  const highProfile = calculateDiscoveryConcurrency({
    deviceProfile: {
      cpuCount: 16,
      freeMemoryMb: 16384,
      totalMemoryMb: 32768,
    },
    fileDescriptorLimit: 8192,
    minimumHostConcurrency: 6,
    minimumMaxConcurrent: 64,
    scanHostCount: 1024,
  });

  assert.ok(lowProfile.maxConcurrent >= 64);
  assert.ok(lowProfile.hostConcurrency >= 6);
  assert.ok(highProfile.maxConcurrent >= 64);
  assert.ok(highProfile.hostConcurrency > lowProfile.hostConcurrency);
  assert.ok(
    highProfile.maxConcurrent * highProfile.hostConcurrency <=
      highProfile.totalSocketBudget,
  );
});

test("service discovery concurrency caps sockets below descriptor pressure", () => {
  const concurrency = calculateDiscoveryConcurrency({
    deviceProfile: {
      cpuCount: 16,
      freeMemoryMb: 16384,
      totalMemoryMb: 32768,
    },
    fileDescriptorLimit: 256,
    minimumHostConcurrency: 6,
    minimumMaxConcurrent: 64,
    scanHostCount: 1024,
  });

  assert.ok(concurrency.maxConcurrent > 0);
  assert.ok(concurrency.hostConcurrency > 0);
  assert.ok(
    concurrency.maxConcurrent * concurrency.hostConcurrency <=
      concurrency.totalSocketBudget,
  );
  assert.ok(concurrency.totalSocketBudget < 384);
});

test("loopback discovery keeps at least the previous 200 port concurrency", () => {
  const concurrency = calculateDiscoveryConcurrency({
    deviceProfile: {
      cpuCount: 4,
      freeMemoryMb: 4096,
      totalMemoryMb: 8192,
    },
    fileDescriptorLimit: 8192,
    minimumHostConcurrency: 1,
    minimumMaxConcurrent: 200,
    scanHostCount: 1,
  });

  assert.ok(concurrency.maxConcurrent >= 200);
});

test("current subnet discovery scans the full range for every selected host", async () => {
  const { manyCalls, scanner } = createCapturingScanner();

  await runServiceDiscoveryScan({
    excludePorts: [7999],
    fullRangeCidrs: ["192.168.1.0/24"],
    isDockerRuntime: true,
    scanCidrs: ["192.168.1.0/24"],
    scanHosts: ["192.168.1.10", "192.168.1.11"],
    scannerService: scanner,
  });

  assert.equal(
    buildDiscoveryPortModeLabel(true, ["192.168.1.0/24"], ["192.168.1.0/24"]),
    "80-60000",
  );
  assert.equal(manyCalls.length, 1);
  assert.deepEqual(manyCalls[0]?.hosts, ["192.168.1.10", "192.168.1.11"]);
  assertFullDiscoveryRange(manyCalls[0]?.options);
  assert.deepEqual(manyCalls[0]?.options?.skipPorts, [7999]);
  assert.ok((manyCalls[0]?.options?.maxConcurrent || 0) > 0);
  assert.ok((manyCalls[0]?.options?.hostConcurrency || 0) > 0);
});

test("other LAN subnet discovery scans only 80-9999", async () => {
  const { manyCalls, scanner } = createCapturingScanner();

  const result = await runServiceDiscoveryScan({
    excludePorts: [7999],
    fullRangeCidrs: ["192.168.31.0/24"],
    isDockerRuntime: false,
    scanCidrs: ["192.168.2.0/24"],
    scanHosts: ["192.168.2.10", "192.168.2.11"],
    scannerService: scanner,
  });

  assert.equal(
    buildDiscoveryPortModeLabel(false, ["192.168.2.0/24"], [
      "192.168.31.0/24",
    ]),
    "80-9999",
  );
  assert.equal(manyCalls.length, 1);
  assert.deepEqual(manyCalls[0]?.hosts, ["192.168.2.10", "192.168.2.11"]);
  assertLimitedDiscoveryRange(manyCalls[0]?.options);
  assert.equal(
    result.totalPortsScanned,
    countLimitedDiscoveryPorts([7999]) * 2,
  );
});

test("single host inside current subnet still scans the full range", async () => {
  const { manyCalls, scanner } = createCapturingScanner();

  const result = await runServiceDiscoveryScan({
    excludePorts: [7999],
    fullRangeCidrs: ["192.168.31.0/24"],
    isDockerRuntime: false,
    scanCidrs: ["192.168.31.10/32"],
    scanHosts: ["192.168.31.10"],
    scannerService: scanner,
  });

  assert.equal(manyCalls.length, 1);
  assert.deepEqual(manyCalls[0]?.hosts, ["192.168.31.10"]);
  assertFullDiscoveryRange(manyCalls[0]?.options);
  assert.equal(result.totalPortsScanned, countDiscoveryPorts([7999]));
});

test("local self hosts skip port 80 without hiding router port 80 in the same subnet", async () => {
  const { manyCalls, scanner } = createCapturingScanner();

  const groups = buildDiscoveryHostGroups(
    ["192.168.31.0/24"],
    ["192.168.31.0/24"],
    ["192.168.31.1", "192.168.31.20"],
    ["192.168.31.20"],
  );
  assert.deepEqual(
    groups.map((group) => ({
      hosts: group.hosts,
      skipPorts: group.skipPorts,
    })),
    [
      {
        hosts: ["192.168.31.1"],
        skipPorts: [],
      },
      {
        hosts: ["192.168.31.20"],
        skipPorts: [...LOCAL_SELF_DISCOVERY_SKIP_PORTS],
      },
    ],
  );

  const result = await runServiceDiscoveryScan({
    excludePorts: [7999],
    fullRangeCidrs: ["192.168.31.0/24"],
    isDockerRuntime: false,
    scanCidrs: ["192.168.31.0/24"],
    scanHosts: ["192.168.31.1", "192.168.31.20"],
    selfScanHosts: ["192.168.31.20"],
    scannerService: scanner,
  });

  assert.equal(manyCalls.length, 2);
  assert.deepEqual(manyCalls[0]?.hosts, ["192.168.31.1"]);
  assert.deepEqual(manyCalls[0]?.options?.skipPorts, [7999]);
  assert.deepEqual(manyCalls[1]?.hosts, ["192.168.31.20"]);
  assert.deepEqual(manyCalls[1]?.options?.skipPorts, [7999, 80]);
  assert.equal(
    result.totalPortsScanned,
    countDiscoveryPorts([7999]) + countDiscoveryPorts([7999, 80]),
  );
});

test("hosts inside the current subnet keep the full range when selected through a wider CIDR", async () => {
  const { manyCalls, scanner } = createCapturingScanner();

  const groups = buildDiscoveryHostGroups(
    ["192.168.30.0/23"],
    ["192.168.31.0/24"],
    ["192.168.30.10", "192.168.31.1"],
  );

  assert.deepEqual(
    groups.map((group) => ({
      hosts: group.hosts,
      mode: group.mode,
      portRange: group.portRange,
    })),
    [
      {
        hosts: ["192.168.31.1"],
        mode: "full",
        portRange: DISCOVERY_PORT_RANGE,
      },
      {
        hosts: ["192.168.30.10"],
        mode: "limited",
        portRange: DISCOVERY_LIMITED_PORT_RANGE,
      },
    ],
  );

  const result = await runServiceDiscoveryScan({
    excludePorts: [7999],
    fullRangeCidrs: ["192.168.31.0/24"],
    isDockerRuntime: false,
    scanCidrs: ["192.168.30.0/23"],
    scanHosts: ["192.168.30.10", "192.168.31.1"],
    scannerService: scanner,
  });

  assert.equal(manyCalls.length, 2);
  assert.deepEqual(manyCalls[0]?.hosts, ["192.168.31.1"]);
  assertFullDiscoveryRange(manyCalls[0]?.options);
  assert.deepEqual(manyCalls[1]?.hosts, ["192.168.30.10"]);
  assertLimitedDiscoveryRange(manyCalls[1]?.options);
  assert.equal(
    result.totalPortsScanned,
    countDiscoveryPorts([7999]) + countLimitedDiscoveryPorts([7999]),
  );
});

test("loopback-only discovery scans one host worth of ports", async () => {
  const { scanner, singleCalls } = createCapturingScanner();

  const result = await runServiceDiscoveryScan({
    excludePorts: [7999],
    isDockerRuntime: false,
    scanCidrs: ["127.0.0.1/32"],
    scanHosts: ["127.0.0.1"],
    scannerService: scanner,
  });

  assert.equal(singleCalls.length, 1);
  assert.equal(singleCalls[0]?.host, "127.0.0.1");
  assert.equal(result.scannedHosts, 1);
  assert.equal(result.totalPortsScanned, countDiscoveryPorts([7999, 80]));
  assertFullDiscoveryRange(singleCalls[0]?.options);
  assert.deepEqual(singleCalls[0]?.options?.skipPorts, [7999, 80]);
});

test("mixed loopback discovery scans loopback and other hosts with the full range", async () => {
  const { manyCalls, scanner, singleCalls } = createCapturingScanner();

  await runServiceDiscoveryScan({
    excludePorts: [7999],
    fullRangeCidrs: ["192.168.1.0/24"],
    isDockerRuntime: false,
    scanCidrs: ["127.0.0.1/32", "192.168.1.0/24"],
    scanHosts: ["127.0.0.1", "192.168.1.10"],
    scannerService: scanner,
  });

  assert.equal(singleCalls.length, 1);
  assert.equal(singleCalls[0]?.host, "127.0.0.1");
  assertFullDiscoveryRange(singleCalls[0]?.options);
  assert.deepEqual(singleCalls[0]?.options?.skipPorts, [7999, 80]);
  assert.ok((singleCalls[0]?.options?.maxConcurrent || 0) > 0);

  assert.equal(manyCalls.length, 1);
  assert.deepEqual(manyCalls[0]?.hosts, ["192.168.1.10"]);
  assertFullDiscoveryRange(manyCalls[0]?.options);
  assert.ok((manyCalls[0]?.options?.maxConcurrent || 0) > 0);
});

test("mixed loopback discovery does not overlap loopback and network scans", async () => {
  const events: string[] = [];
  let activeScans = 0;
  let maxActiveScans = 0;
  const runGroup = async (
    name: string,
    host: string,
    hosts: readonly string[],
    options?: ScanOptions,
  ) => {
    events.push(`start:${name}`);
    activeScans += 1;
    maxActiveScans = Math.max(maxActiveScans, activeScans);
    await new Promise((resolve) => setTimeout(resolve, 0));
    activeScans -= 1;
    events.push(`end:${name}`);
    return buildEmptyResult(host, hosts, options);
  };
  const scanner: ServiceDiscoveryScanner = {
    scanAndAnalyze: (host, options) =>
      runGroup("loopback", host, [host], options),
    scanAndAnalyzeMany: (hosts, options) =>
      runGroup("network", hosts[0] || "", hosts, options),
  };

  await runServiceDiscoveryScan({
    excludePorts: [7999],
    isDockerRuntime: false,
    scanCidrs: ["127.0.0.1/32", "192.168.1.0/24"],
    scanHosts: ["127.0.0.1", "192.168.1.10"],
    scannerService: scanner,
  });

  assert.equal(maxActiveScans, 1);
  assert.deepEqual(events, [
    "start:loopback",
    "end:loopback",
    "start:network",
    "end:network",
  ]);
});
