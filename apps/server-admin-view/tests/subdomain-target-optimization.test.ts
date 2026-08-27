/// <reference types="node" />

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { afterEach, describe, it } from "node:test";
import { computed, nextTick, ref } from "vue";
import {
  ScanAPI,
  type ScanDiscoveryHostCandidate,
  type ScanDiscoveryTargetsResponse,
} from "../src/lib/api/scan";
import type { HostMapping } from "../src/types";
import { createDefaultMapping } from "../src/views/subdomain-proxy/model";
import {
  buildTargetOptimizationDestinations,
  buildTargetOptimizationPreviews,
  parseOptimizableTargetHostname,
  resolveDefaultTargetOptimizationDestination,
  rewriteTargetHostname,
} from "../src/views/subdomain-proxy/subdomain-target-optimization";
import { useSubdomainTargetOptimization } from "../src/views/subdomain-proxy/useSubdomainTargetOptimization";
import { useHostTargetCandidateCatalog } from "../src/views/subdomain-proxy/useHostTargetCandidateCatalog";

const candidate = (
  address: string,
  source: ScanDiscoveryHostCandidate["source"],
): ScanDiscoveryHostCandidate => ({
  address,
  cidr: `${address}/32`,
  includedInAutomaticScan: true,
  recommended: source === "loopback",
  source,
});

const nativeCandidates = [
  candidate("127.0.0.1", "loopback"),
  candidate("192.168.50.8", "interface"),
  candidate("10.20.0.8", "interface"),
];

const mapping = (host: string, target: string): HostMapping => ({
  ...createDefaultMapping(),
  host,
  target,
});

const response = (
  hostCandidates: ScanDiscoveryHostCandidate[],
): ScanDiscoveryTargetsResponse =>
  ({
    automaticTargets: [],
    customTargets: [],
    effectiveCidrs: [],
    hostCandidates,
    limits: { maxCidrs: 8, maxHosts: 2048 },
    selectedCidrs: [],
    selectedTargets: [],
    selectionMode: "automatic",
  }) as ScanDiscoveryTargetsResponse;

const originalGetDiscoverTargets = ScanAPI.getDiscoverTargets;
const originalConsoleWarn = console.warn;
afterEach(() => {
  ScanAPI.getDiscoverTargets = originalGetDiscoverTargets;
  console.warn = originalConsoleWarn;
});

describe("subdomain target optimization model", () => {
  it("rewrites only the IPv4 hostname while preserving the original target text", () => {
    const targets = [
      "http://127.0.0.1:80",
      "https://user:pass@127.0.0.1:443/path?q=1#part",
      "ws://127.0.0.1:9000/socket",
      "wss://127.0.0.1:9443/socket?token=yes",
      "  http://127.0.0.1:8080/path  ",
    ];
    for (const target of targets) {
      assert.equal(
        rewriteTargetHostname(
          target,
          new Set(["127.0.0.1"]),
          "192.168.50.8",
        ),
        target.replace("127.0.0.1", "192.168.50.8"),
      );
    }
  });

  it("strictly rejects localhost, other loopback addresses, hostnames, and invalid targets", () => {
    for (const target of [
      "http://localhost:8080",
      "http://device.local:8080",
      "ftp://127.0.0.1:21",
      "127.0.0.1:8080",
      "not a target",
    ]) {
      assert.equal(parseOptimizableTargetHostname(target), null);
      assert.equal(
        rewriteTargetHostname(
          target,
          new Set(["127.0.0.1"]),
          "192.168.50.8",
        ),
        null,
      );
    }
    assert.equal(
      rewriteTargetHostname(
        "http://127.0.0.2:8080",
        new Set(["127.0.0.1"]),
        "192.168.50.8",
      ),
      null,
    );
  });

  it("converts only detected local interfaces back to exact loopback", () => {
    const mappings = [
      mapping("local.example.test", "http://192.168.50.8:8080/path"),
      mapping("other.example.test", "http://192.168.50.99:8080"),
      mapping("loop.example.test", "http://127.0.0.1:8080"),
    ];
    assert.deepEqual(
      buildTargetOptimizationPreviews({
        candidates: nativeCandidates,
        destinationAddress: "127.0.0.1",
        isAuthServiceTarget: () => false,
        isDockerDeployment: false,
        mappings,
      }),
      [
        {
          direction: "lan_to_loopback",
          host: "local.example.test",
          target: "http://192.168.50.8:8080/path",
          nextTarget: "http://127.0.0.1:8080/path",
        },
      ],
    );
  });

  it("keeps loopback out of Docker destinations and ignores auth mappings", () => {
    const dockerCandidates = [
      candidate("127.0.0.1", "loopback"),
      candidate("192.168.50.8", "proxy"),
    ];
    assert.deepEqual(
      buildTargetOptimizationDestinations(dockerCandidates, true).map(
        (item) => item.address,
      ),
      ["192.168.50.8"],
    );
    assert.equal(
      buildTargetOptimizationPreviews({
        candidates: dockerCandidates,
        destinationAddress: "127.0.0.1",
        isAuthServiceTarget: () => false,
        isDockerDeployment: true,
        mappings: [mapping("app.example.test", "http://192.168.50.8:80")],
      }).length,
      0,
    );
    assert.equal(
      buildTargetOptimizationPreviews({
        candidates: dockerCandidates,
        destinationAddress: "192.168.50.8",
        isAuthServiceTarget: (target) => target.endsWith(":7997"),
        isDockerDeployment: true,
        mappings: [
          mapping("app.example.test", "http://127.0.0.1:8080"),
          mapping("auth.example.test", "http://127.0.0.1:7997"),
        ],
      }).length,
      1,
    );
  });

  it("prefers LAN for loopback mappings and otherwise selects loopback", () => {
    assert.equal(
      resolveDefaultTargetOptimizationDestination({
        candidates: nativeCandidates,
        isAuthServiceTarget: () => false,
        isDockerDeployment: false,
        mappings: [mapping("loop.example.test", "http://127.0.0.1:8080")],
      }),
      "192.168.50.8",
    );
    assert.equal(
      resolveDefaultTargetOptimizationDestination({
        candidates: nativeCandidates,
        isAuthServiceTarget: () => false,
        isDockerDeployment: false,
        mappings: [mapping("lan.example.test", "http://10.20.0.8:8080")],
      }),
      "127.0.0.1",
    );
  });
});

describe("subdomain target optimization state", () => {
  it("reports candidate failures with safe native and Docker fallbacks", async () => {
    console.warn = () => undefined;
    ScanAPI.getDiscoverTargets = async () => {
      throw new Error("load failed");
    };
    const nativeCatalog = useHostTargetCandidateCatalog({
      isDockerDeployment: computed(() => false),
      open: computed(() => true),
    });
    await nextTick();
    await nextTick();
    assert.equal(nativeCatalog.loadFailed.value, true);
    assert.deepEqual(
      nativeCatalog.effectiveCandidates.value.map((item) => item.address),
      ["127.0.0.1"],
    );

    const dockerCatalog = useHostTargetCandidateCatalog({
      isDockerDeployment: computed(() => true),
      open: computed(() => true),
    });
    await nextTick();
    await nextTick();
    assert.equal(dockerCatalog.loadFailed.value, true);
    assert.deepEqual(dockerCatalog.effectiveCandidates.value, []);
  });

  it("defaults to all matching mappings, supports reselection, and saves only selected rows", async () => {
    ScanAPI.getDiscoverTargets = async () => response(nativeCandidates);
    const mappings = ref([
      mapping("one.example.test", "http://127.0.0.1:8080"),
      mapping("two.example.test", "https://127.0.0.1:8443/path"),
      mapping("lan.example.test", "http://192.168.50.8:3000"),
    ]);
    const saved = ref<HostMapping[] | null>(null);
    const model = useSubdomainTargetOptimization({
      allMappings: computed(() => mappings.value),
      isAuthServiceTarget: () => false,
      isDockerDeployment: computed(() => false),
      isSavingMappings: ref(false),
      runSaveMappings: async (action) => action(),
      saveHostMappings: async (next) => {
        saved.value = next;
      },
      translate: (key) => key,
    });

    model.openDialog();
    await nextTick();
    await nextTick();
    assert.equal(model.destinationAddress.value, "192.168.50.8");
    assert.equal(model.selectedCount.value, 2);

    model.setMappingSelected("two.example.test", false);
    assert.equal(model.selectedCount.value, 1);
    await model.saveOptimizedTargets();

    assert.equal(model.isOpen.value, false);
    assert.equal(
      saved.value?.find((item) => item.host === "one.example.test")?.target,
      "http://192.168.50.8:8080",
    );
    assert.equal(
      saved.value?.find((item) => item.host === "two.example.test")?.target,
      "https://127.0.0.1:8443/path",
    );
  });

  it("reselects all eligible rows when the destination changes and stays open on save failure", async () => {
    ScanAPI.getDiscoverTargets = async () => response(nativeCandidates);
    const model = useSubdomainTargetOptimization({
      allMappings: computed(() => [
        mapping("loop.example.test", "http://127.0.0.1:8080"),
        mapping("lan.example.test", "http://192.168.50.8:3000"),
      ]),
      isAuthServiceTarget: () => false,
      isDockerDeployment: computed(() => false),
      isSavingMappings: ref(false),
      runSaveMappings: async (action) => {
        try {
          return await action();
        } catch {
          return undefined;
        }
      },
      saveHostMappings: async () => {
        throw new Error("save failed");
      },
      translate: (key) => key,
    });

    model.openDialog();
    await nextTick();
    await nextTick();
    model.setAllSelected(false);
    model.setDestinationAddress("127.0.0.1");
    assert.deepEqual(
      model.previews.value.map((item) => item.host),
      ["lan.example.test"],
    );
    assert.equal(model.selectedCount.value, 1);

    await model.saveOptimizedTargets();
    assert.equal(model.isOpen.value, true);
    assert.equal(model.selectedCount.value, 1);
  });

  it("recovers to a valid safe destination when deployment mode changes", async () => {
    const docker = ref(false);
    ScanAPI.getDiscoverTargets = async () =>
      response(
        docker.value
          ? [candidate("10.20.0.8", "configured")]
          : nativeCandidates,
      );
    const model = useSubdomainTargetOptimization({
      allMappings: computed(() => [
        mapping("loop.example.test", "http://127.0.0.1:8080"),
        mapping("lan.example.test", "http://192.168.50.8:3000"),
      ]),
      isAuthServiceTarget: () => false,
      isDockerDeployment: computed(() => docker.value),
      isSavingMappings: ref(false),
      runSaveMappings: async (action) => action(),
      saveHostMappings: async () => undefined,
      translate: (key) => key,
    });

    model.openDialog();
    await nextTick();
    await nextTick();
    model.setDestinationAddress("127.0.0.1");
    docker.value = true;
    await nextTick();
    await nextTick();
    await nextTick();

    assert.equal(model.destinationAddress.value, "10.20.0.8");
    assert.deepEqual(
      model.destinations.value.map((item) => item.address),
      ["10.20.0.8"],
    );
    assert.equal(model.selectedCount.value, 1);
  });
});

describe("subdomain target optimization interface", () => {
  it("wires the top utility menu to a preview-and-confirm multi-select dialog", () => {
    const readSource = (path: string) =>
      readFileSync(new URL(path, import.meta.url), "utf8");
    const header = readSource(
      "../src/views/subdomain-proxy/SubdomainMappingsCardHeader.vue",
    );
    const maintenanceItems = readSource(
      "../src/views/subdomain-proxy/SubdomainMappingsMaintenanceMenuItems.vue",
    );
    const dialog = readSource(
      "../src/views/subdomain-proxy/SubdomainTargetOptimizationDialog.vue",
    );
    const overview = readSource(
      "../src/views/subdomain-proxy/SubdomainProxyOverview.vue",
    );

    assert.match(header, /emit\('open-target-optimization'\)/u);
    assert.match(header, /SubdomainMappingsMaintenanceMenuItems/u);
    assert.match(maintenanceItems, /!hasMappings \|\| saving \|\| clearing/u);
    assert.match(overview, /@open-target-optimization="openTargetOptimization"/u);
    assert.match(dialog, /<Select/u);
    assert.match(dialog, /<Checkbox/u);
    assert.match(dialog, /preview\.target/u);
    assert.match(dialog, /preview\.nextTarget/u);
    assert.match(dialog, /saveOptimizedTargets/u);
  });
});
