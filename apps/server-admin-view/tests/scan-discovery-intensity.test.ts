import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

const componentSource = readSource(
  "../src/components/ScanDiscoveryIntensityDialog.vue",
);
const zhCnAdminSource = readSource(
  "../../../packages/i18n/src/messages/admin/zh-CN.ts",
);
const scanApiSource = readSource("../src/lib/api/scan.ts");
const reverseProxySource = readSource("../src/views/ReverseProxy.vue");
const subdomainProxySource = readSource("../src/views/SubdomainProxy.vue");
const subdomainCardSource = readSource(
  "../src/views/subdomain-proxy/SubdomainMappingsCard.vue",
);

describe("scan discovery intensity", () => {
  it("keeps the configuration UI in one business-named SFC", () => {
    assert.match(componentSource, /ScanAPI\.getDiscoverSettings\(\)/u);
    assert.match(componentSource, /ScanAPI\.saveDiscoverSettings/u);
    assert.doesNotMatch(
      componentSource,
      /EffortCard|Ultracode|useSliderState|useWebglFire/u,
    );
    assert.doesNotMatch(
      componentSource,
      /DialogHeader|DialogDescription|DialogFooter|<Switch|<Button/u,
    );
    assert.match(componentSource, /:show-close-button="false"/u);
    assert.doesNotMatch(
      componentSource,
      /lowLoad|highSpeed|scan-pressure-scale/u,
    );
    assert.match(zhCnAdminSource, /title: "扫描强度配置"/u);
    assert.match(componentSource, /width: min\(376px/u);
    assert.match(componentSource, /height: 30px/u);
    assert.match(componentSource, /width: 29px/u);
    assert.match(componentSource, /class="scan-pressure-handle"/u);
    assert.match(componentSource, /:style="sliderHandleStyle"/u);
    assert.match(componentSource, /--scan-handle-offset/u);
    assert.match(componentSource, /right: "0px"/u);
    assert.match(componentSource, /scan-pressure-terminal-shield/u);
    assert.match(
      componentSource,
      /\.scan-pressure-card\.is-energized \.scan-pressure-terminal-shield/su,
    );
    assert.match(
      componentSource,
      /::-webkit-slider-thumb\s*\{[^}]*background: transparent;[^}]*box-shadow: none;/su,
    );
    assert.match(
      componentSource,
      /\.scan-pressure-card\.is-energized \.scan-pressure-handle/su,
    );
    assert.match(
      componentSource,
      /\.scan-pressure-track\s*\{[^}]*overflow: visible/su,
    );
    assert.match(
      componentSource,
      /\.scan-pressure-visual\s*\{[^}]*overflow: hidden/su,
    );
    assert.match(componentSource, /max="100"/u);
    assert.match(componentSource, /<TooltipProvider>/u);
    assert.match(componentSource, /<TooltipContent align="end"/u);
    assert.match(componentSource, /currentEffectiveConcurrency/u);
    assert.match(componentSource, /payload\.capability\.safeConcurrency/u);
    assert.match(componentSource, /admin\.scanIntensity\.safeConcurrency/u);
    assert.match(componentSource, /@click\.stop="toggleConcurrencyPopup"/u);
    assert.match(componentSource, /position < 33/u);
    assert.match(componentSource, /position < 66/u);
    assert.match(componentSource, /position < 100/u);
    assert.doesNotMatch(componentSource, /max="3"/u);
  });

  it("retains the four-pass WebGL feedback pipeline and lifecycle guards", () => {
    assert.match(componentSource, /getContext\("webgl2"/u);
    assert.match(componentSource, /PROBE_FIELD_SOURCE/u);
    assert.match(componentSource, /PROBE_BLUR_SOURCE/u);
    assert.match(componentSource, /PROBE_COMPOSITE_SOURCE/u);
    assert.match(componentSource, /feedbackFront/u);
    assert.match(componentSource, /feedbackBack/u);
    assert.match(componentSource, /ResizeObserver/u);
    assert.match(componentSource, /webglcontextlost/u);
    assert.match(componentSource, /webglcontextrestored/u);
    assert.match(componentSource, /prefers-reduced-motion/u);
    assert.match(componentSource, /releaseRenderTargets/u);
    assert.match(componentSource, /releaseMatrixPrograms/u);
    assert.match(componentSource, /visualTier !== 3/u);
    assert.match(componentSource, /startPortWave/u);
    assert.match(componentSource, /stopPortWave/u);
  });

  it("uses four concurrency-only levels and the shared settings endpoint", () => {
    assert.match(scanApiSource, /"low" \| "medium" \| "high" \| "extreme"/u);
    assert.match(scanApiSource, /get\("\/scan\/discover-settings"\)/u);
    assert.match(scanApiSource, /post\("\/scan\/discover-settings"/u);
    assert.match(componentSource, /low: 32/u);
    assert.match(componentSource, /medium: 115/u);
    assert.match(componentSource, /high: 256/u);
    assert.match(componentSource, /extreme: 512/u);
    assert.match(componentSource, /@change="flushManualSave"/u);
    assert.match(componentSource, /persistSettings\("auto"\)/u);
    assert.match(zhCnAdminSource, /80–60000/u);
  });

  it("wires both discovery dropdowns to the shared dialog", () => {
    assert.match(reverseProxySource, /<ScanDiscoveryIntensityDialog/u);
    assert.match(reverseProxySource, /admin\.scanIntensity\.title/u);
    assert.match(subdomainProxySource, /<ScanDiscoveryIntensityDialog/u);
    assert.match(subdomainProxySource, /@open-discover-settings/u);
    assert.match(subdomainCardSource, /emit\('open-discover-settings'\)/u);
  });
});
