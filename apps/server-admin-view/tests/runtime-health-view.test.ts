import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

describe("runtime health view", () => {
  it("keeps virtual components compact and process details conditional", () => {
    const viewSource = readSource("../src/views/event-center/RuntimeTab.vue");
    const cardSource = readSource(
      "../src/views/event-center/RuntimeComponentCard.vue",
    );
    const controllerSource = readSource(
      "../src/views/event-center/useRuntimeHealth.ts",
    );

    assert.match(viewSource, /useRuntimeHealth/u);
    assert.match(controllerSource, /process_state !== "not_applicable"/u);
    assert.match(viewSource, /xl:grid-cols-3/u);
    assert.match(viewSource, /xl:col-span-2/u);
    assert.match(viewSource, /sm:grid-cols-2 xl:auto-rows-fr xl:grid-cols-1/u);
    assert.match(cardSource, /v-if="variant === 'process'"/u);
    assert.match(cardSource, /component\.rss_bytes != null/u);
  });

  it("offers only the Rust and Go process operational logs", () => {
    const viewSource = readSource("../src/views/event-center/RuntimeTab.vue");
    const cardSource = readSource(
      "../src/views/event-center/RuntimeComponentCard.vue",
    );
    const apiSource = readSource("../src/lib/api/runtime-health.ts");
    const controllerSource = readSource(
      "../src/views/event-center/useRuntimeHealth.ts",
    );

    assert.match(
      controllerSource,
      /component\.id === "management" \|\| component\.id === "gateway_process"/u,
    );
    assert.match(cardSource, /admin\.eventCenter\.runtime\.viewLogs/u);
    assert.match(apiSource, /runtime-health\/logs/u);
    assert.match(apiSource, /apiClient\.delete/u);
    assert.match(viewSource, /ConfirmDangerPopover/u);
    assert.match(viewSource, /admin\.eventCenter\.runtime\.clearLogs/u);
    assert.doesNotMatch(viewSource, /RuntimeHealthAPI|EventCenterAPI/u);
  });

  it("opens Go memory controls from the gateway process card", () => {
    const viewSource = readSource("../src/views/event-center/RuntimeTab.vue");
    const cardSource = readSource(
      "../src/views/event-center/RuntimeComponentCard.vue",
    );
    const dialogSource = readSource(
      "../src/views/event-center/GatewayMemoryDialog.vue",
    );
    const apiSource = readSource("../src/lib/api/runtime-health.ts");

    assert.match(viewSource, /component\.id === 'gateway_process'/u);
    assert.match(viewSource, /GatewayMemoryDialog/u);
    assert.match(cardSource, /MemoryStick/u);
    assert.match(cardSource, /manageMemory/u);
    assert.match(dialogSource, /updateGatewayMemoryConfig/u);
    assert.match(dialogSource, /reclaimGatewayMemory/u);
    assert.match(dialogSource, /!validMemoryLimit\.value/u);
    assert.match(dialogSource, /MIN_GC_PERCENT = 25/u);
    assert.match(dialogSource, /MAX_GC_PERCENT = 500/u);
    assert.match(dialogSource, /MIN_MEMORY_LIMIT_MIB = 64/u);
    assert.match(dialogSource, /MAX_MEMORY_LIMIT_MIB = 4096/u);
    assert.match(dialogSource, /memory_limit_mib:/u);
    assert.match(dialogSource, /saving\.value \|\| reclaiming\.value/u);
    assert.match(apiSource, /runtime-health\/gateway-memory\/reclaim/u);
  });
});
