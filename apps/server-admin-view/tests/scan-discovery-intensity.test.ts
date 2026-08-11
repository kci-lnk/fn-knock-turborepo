import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

const componentSource = readSource(
  "../src/components/ScanDiscoveryIntensityDialog.vue",
);
const matrixComposableSource = readSource(
  "../src/composables/useScanIntensityMatrix.ts",
);
const settingsComposableSource = readSource(
  "../src/composables/useScanDiscoveryIntensitySettings.ts",
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
    assert.match(componentSource, /useScanDiscoveryIntensitySettings/u);
    assert.doesNotMatch(componentSource, /ScanAPI\./u);
    assert.match(settingsComposableSource, /ScanAPI\.getDiscoverSettings\(\)/u);
    assert.match(settingsComposableSource, /ScanAPI\.saveDiscoverSettings/u);
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
    assert.match(settingsComposableSource, /currentEffectiveConcurrency/u);
    assert.match(
      settingsComposableSource,
      /payload\.capability\.safeConcurrency/u,
    );
    assert.match(componentSource, /admin\.scanIntensity\.safeConcurrency/u);
    assert.match(componentSource, /@click\.stop="toggleConcurrencyPopup"/u);
    assert.match(settingsComposableSource, /position < 33/u);
    assert.match(settingsComposableSource, /position < 66/u);
    assert.match(settingsComposableSource, /position < 100/u);
    assert.doesNotMatch(componentSource, /max="3"/u);
  });

  it("encapsulates the four-pass WebGL pipeline behind a business composable", () => {
    assert.match(componentSource, /useScanIntensityMatrix/u);
    assert.match(componentSource, /setCanvas: setMatrixCanvas/u);
    assert.match(matrixComposableSource, /getContext\("webgl2"/u);
    assert.match(matrixComposableSource, /PROBE_FIELD_SOURCE/u);
    assert.match(matrixComposableSource, /PROBE_BLUR_SOURCE/u);
    assert.match(matrixComposableSource, /PROBE_COMPOSITE_SOURCE/u);
    assert.match(matrixComposableSource, /feedbackFront/u);
    assert.match(matrixComposableSource, /feedbackBack/u);
    assert.match(matrixComposableSource, /ResizeObserver/u);
    assert.match(matrixComposableSource, /webglcontextlost/u);
    assert.match(matrixComposableSource, /webglcontextrestored/u);
    assert.match(matrixComposableSource, /prefers-reduced-motion/u);
    assert.match(matrixComposableSource, /releaseRenderTargets/u);
    assert.match(matrixComposableSource, /releaseMatrixPrograms/u);
    assert.match(matrixComposableSource, /visualTier !== 3/u);
    assert.match(matrixComposableSource, /startPortWave/u);
    assert.match(matrixComposableSource, /stopPortWave/u);
  });

  it("uses four concurrency-only levels and the shared settings endpoint", () => {
    assert.match(scanApiSource, /ScanSchemas\["ScanDiscoverySettingsData"\]/u);
    assert.match(
      scanApiSource,
      /ScanSchemas\["ScanDiscoverySettingsUpdateData"\]/u,
    );
    assert.match(scanApiSource, /get\("\/scan\/discover-settings"\)/u);
    assert.match(scanApiSource, /post\("\/scan\/discover-settings"/u);
    assert.match(settingsComposableSource, /low: 32/u);
    assert.match(settingsComposableSource, /medium: 115/u);
    assert.match(settingsComposableSource, /high: 256/u);
    assert.match(settingsComposableSource, /extreme: 512/u);
    assert.match(componentSource, /@change="flushManualSave"/u);
    assert.match(settingsComposableSource, /persistSettings\("auto"\)/u);
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
