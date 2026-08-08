import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

describe("Wake-on-LAN management", () => {
  it("gates navigation, route access, portal settings, and permissions on the feature switch", () => {
    const features = readSource(
      "../src/views/system-settings/useFeaturesSettings.ts",
    );
    const navigation = readSource("../src/views/layout/useLayoutNavigation.ts");
    const runtimeAccess = readSource("../src/router/runtime-access.ts");
    const router = readSource("../src/router/index.ts");
    const gatewayPortal = readSource(
      "../src/views/system-settings/GatewayPortalSettings.vue",
    );
    const permissions = readSource(
      "../src/views/auth-settings/useAuthSubdomainAccess.ts",
    );

    assert.match(features, /updateWOLFeature\(\{ enabled: nextValue \}\)/u);
    assert.match(navigation, /wol_feature\?\.enabled === true/u);
    assert.match(navigation, /icon: MonitorUp/u);
    assert.match(runtimeAccess, /query: \{ tab: "features" \}/u);
    assert.match(router, /if \(to\.path !== "\/wol"\) \{\s*return "\/whitelist"/u);
    assert.match(gatewayPortal, /v-if="configStore\.config\?\.wol_feature/u);
    assert.match(permissions, /__builtin_wol__/u);
    assert.match(permissions, /if \(wolFeatureEnabled\.value\)/u);
  });

  it("uses the existing server-admin-rs process as the Relay runtime", () => {
    const app = readSource("../../server-admin-rs/src/app.rs");
    const relay = readSource("../../server-admin-rs/src/wol/relay.rs");
    const bootstrap = readSource(
      "../src/views/wol-management/WOLBootstrapDialog.vue",
    );

    assert.match(app, /start_wol_tasks\(state\.clone\(\)\)/u);
    assert.match(relay, /state\.shutdown\.cancelled\(\)/u);
    assert.match(relay, /state\.wol_relay_reload\.notified\(\)/u);
    assert.match(relay, /wol_runtime_reload\.subscribe\(\)/u);
    assert.match(relay, /runtime_reload\.changed\(\)/u);
    assert.doesNotMatch(bootstrap, /fn-knock-wol-relay|psk_file|systemd/u);
    assert.equal(
      existsSync(new URL("../../wol-relay-rs/Cargo.toml", import.meta.url)),
      false,
    );
  });

  it("pairs with one code and hides Relay credentials from the basic UI", () => {
    const api = readSource("../src/lib/api/wol.ts");
    const page = readSource("../src/views/WOLManagement.vue");
    const localRelay = readSource(
      "../src/views/wol-management/WOLLocalRelaySettings.vue",
    );
    const bootstrap = readSource(
      "../src/views/wol-management/WOLBootstrapDialog.vue",
    );

    assert.match(api, /get\("\/wol\/local-relay"\)/u);
    assert.match(api, /put\("\/wol\/local-relay", payload\)/u);
    assert.match(api, /post\("\/wol\/local-relay\/pair"/u);
    assert.match(api, /pskConfigured: boolean/u);
    assert.doesNotMatch(
      api.slice(
        api.indexOf("export type WOLLocalRelayConfig"),
        api.indexOf("export type WOLLocalRelayInput"),
      ),
      /\bpsk:\s*string/u,
    );
    assert.match(localRelay, /pairingCode/u);
    assert.doesNotMatch(localRelay, /model\.psk|relayId|keyVersion/u);
    assert.match(bootstrap, /credential\.bootstrap\.pairingCode/u);
    assert.doesNotMatch(bootstrap, /credential\.bootstrap\.psk/u);
    assert.match(page, /psk: ""/u);
    assert.doesNotMatch(page + localRelay, /localStorage|sessionStorage/u);
    assert.doesNotMatch(page, /value="local-relay"/u);
  });

  it("keeps wake/probe feedback scoped and treats acknowledgement timeout as unknown", () => {
    const page = readSource("../src/views/WOLManagement.vue");
    assert.match(page, /wakingTargetIds = ref\(new Set<string>\(\)\)/u);
    assert.match(page, /probingRelayIds = ref\(new Set<string>\(\)\)/u);
    assert.match(page, /status === 504/u);
    assert.match(page, /toast\.warning\(t\("admin\.wol\.wakeUnknown"\)/u);
    assert.match(page, /!relay\.enabled/u);
  });

  it("prioritizes target names and notes over technical wake details", () => {
    const page = readSource("../src/views/WOLManagement.vue");
    const template = page.slice(page.indexOf("<template>"));
    const primaryIndex = template.indexOf('data-testid="wol-target-primary"');
    const technicalIndex = template.indexOf(
      'data-testid="wol-target-technical"',
    );

    assert.notEqual(primaryIndex, -1);
    assert.notEqual(technicalIndex, -1);
    assert(primaryIndex < technicalIndex);
    assert(template.indexOf("target.name", primaryIndex) < technicalIndex);
    assert(template.indexOf("target.note", primaryIndex) < technicalIndex);
    assert.match(
      template.slice(primaryIndex, technicalIndex),
      /text-lg[^"]*target\.name|text-lg[\s\S]*target\.name/u,
    );
    assert.match(
      template.slice(primaryIndex, technicalIndex),
      /text-sm[\s\S]*target\.note/u,
    );
    assert.match(
      template,
      /target\.status\.state === 'online'[\s\S]*bg-emerald-500/u,
    );
    assert.match(template, /<MonitorUp v-else/u);
    assert.match(
      template,
      /target\.status\.observedIp \|\| target\.ipAddress/u,
    );
    assert.match(template, /admin\.wol\.portal\.showShortcut/u);
  });

  it("streams custom-or-default LAN discovery results and stores device notes", () => {
    const api = readSource("../src/lib/api/wol.ts");
    const page = readSource("../src/views/WOLManagement.vue");
    const targetDialog = readSource(
      "../src/views/wol-management/WOLTargetDialog.vue",
    );
    const discoveryDialog = readSource(
      "../src/views/wol-management/WOLDiscoveryDialog.vue",
    );

    assert.match(api, /post\(\s*"\/wol\/discover\/jobs"/u);
    assert.match(api, /params: \{ cursor \}/u);
    assert.match(api, /type: "device"/u);
    assert.match(api, /targetCidrs/u);
    assert.match(page, /relayId: null/u);
    assert.match(page, /broadcastAddress: device\.broadcastAddress/u);
    assert.match(page, /name: device\.name/u);
    assert.match(page, /note: device\.note/u);
    assert.match(page, /DropdownMenuTrigger/u);
    assert.match(page, /wol-device-actions-menu-trigger/u);
    assert.match(page, /<DropdownMenuItem @select="openCreateTarget">/u);
    assert.match(targetDialog, /localDeliveryValue/u);
    assert.match(targetDialog, /v-model="model\.note"/u);
    assert.match(discoveryDialog, /selectedDevices/u);
    assert.match(discoveryDialog, /existing\.has\(device\.mac\)/u);
    assert.match(discoveryDialog, /customCidrs/u);
    assert.match(discoveryDialog, /v-if="showSettings"/u);
    assert.match(discoveryDialog, /<Settings2/u);
    assert.match(discoveryDialog, /progressPercent/u);
    assert.match(discoveryDialog, /notes\[device\.mac\]/u);
    assert.match(discoveryDialog, /names\[device\.mac\]/u);
    assert.match(discoveryDialog, /selectAllState/u);
    assert.match(discoveryDialog, /toggleAll/u);
    assert.doesNotMatch(discoveryDialog, /nextSelected\.add/u);
  });
});
