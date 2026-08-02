/// <reference types="node" />

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { describe, it } from "node:test";

import {
  areFirewallPortListsEqual,
  MAX_FIREWALL_ADDITIONAL_PORTS,
  resolveFirewallAdditionalPortsSuccessMessageKey,
  validateFirewallAdditionalPortDraft,
} from "../src/views/system-settings/firewallAdditionalPortsModel";
import { createFirewallAdditionalPortsController } from "../src/views/system-settings/useFirewallAdditionalPorts";
import type { FirewallAdditionalPortsDetails } from "../src/types";

const details = (
  overrides: Partial<FirewallAdditionalPortsDetails> = {},
): FirewallAdditionalPortsDetails => ({
  additionalPorts: [5666],
  automaticPorts: [7999],
  effectivePorts: [5666, 7999],
  runType: 0,
  appliedNow: true,
  ...overrides,
});

describe("firewall additional ports", () => {
  it("accepts empty drafts and returns sorted unique integer ports", () => {
    assert.deepEqual(validateFirewallAdditionalPortDraft([]), {
      valid: true,
      ports: [],
    });
    assert.deepEqual(validateFirewallAdditionalPortDraft([" 5666 ", "53"]), {
      valid: true,
      ports: [53, 5666],
    });
  });

  it("rejects blank, non-integer, out-of-range, duplicate, and oversized drafts", () => {
    assert.equal(validateFirewallAdditionalPortDraft([""]).valid, false);
    assert.deepEqual(validateFirewallAdditionalPortDraft(["1.5"]), {
      valid: false,
      code: "integer",
      index: 0,
    });
    assert.deepEqual(validateFirewallAdditionalPortDraft(["65536"]), {
      valid: false,
      code: "range",
      index: 0,
    });
    assert.deepEqual(validateFirewallAdditionalPortDraft(["53", "053"]), {
      valid: false,
      code: "duplicate",
      index: 1,
    });
    assert.deepEqual(
      validateFirewallAdditionalPortDraft(
        Array.from({ length: MAX_FIREWALL_ADDITIONAL_PORTS + 1 }, (_, index) =>
          String(index + 1),
        ),
      ),
      { valid: false, code: "tooMany" },
    );
  });

  it("compares saved port sets independent of ordering", () => {
    assert.equal(areFirewallPortListsEqual([5666, 53], [53, 5666]), true);
    assert.equal(areFirewallPortListsEqual([53], [53, 5666]), false);
  });

  it("loads authoritatively, preserves the dialog on failure, and supports retry", async () => {
    let reads = 0;
    let loadErrors = 0;
    const controller = createFirewallAdditionalPortsController(
      {
        canManageHostFirewall: () => true,
        hasUnsavedModeChanges: () => false,
      },
      {
        getDetails: async () => {
          reads += 1;
          if (reads === 1) throw new Error("load failed");
          return details();
        },
        updatePorts: async () => details(),
        onLoadError: () => {
          loadErrors += 1;
        },
        onSaveError: () => undefined,
        onUnsupported: () => undefined,
        onUpdated: () => undefined,
        onSaved: () => undefined,
      },
    );

    await controller.openDialog();
    assert.equal(controller.open.value, true);
    assert.equal(controller.loadFailed.value, true);
    assert.equal(controller.details.value, null);
    assert.equal(loadErrors, 1);

    await controller.load();
    assert.equal(controller.loadFailed.value, false);
    assert.deepEqual(controller.details.value, details());
  });

  it("keeps state on save failure, blocks closing while saving, and exposes returned ports", async () => {
    let updateAttempts = 0;
    let saveErrors = 0;
    let resolveUpdate: ((value: FirewallAdditionalPortsDetails) => void) | null =
      null;
    let updated: FirewallAdditionalPortsDetails | null = null;
    let saved: {
      result: FirewallAdditionalPortsDetails;
      showUnsavedModeNotice: boolean;
    } | null = null;
    const controller = createFirewallAdditionalPortsController(
      {
        canManageHostFirewall: () => true,
        hasUnsavedModeChanges: () => true,
      },
      {
        getDetails: async () => details(),
        updatePorts: async () => {
          updateAttempts += 1;
          if (updateAttempts === 1) throw new Error("save failed");
          return new Promise<FirewallAdditionalPortsDetails>((resolve) => {
            resolveUpdate = resolve;
          });
        },
        onLoadError: () => undefined,
        onSaveError: () => {
          saveErrors += 1;
        },
        onUnsupported: () => undefined,
        onUpdated: (result) => {
          updated = result;
        },
        onSaved: (result, showUnsavedModeNotice) => {
          saved = { result, showUnsavedModeNotice };
        },
      },
    );
    await controller.openDialog();

    await controller.save([53]);
    assert.equal(saveErrors, 1);
    assert.equal(controller.open.value, true);
    assert.deepEqual(controller.details.value, details());

    const save = controller.save([53, 5666]);
    await Promise.resolve();
    assert.equal(controller.saving.value, true);
    controller.updateOpen(false);
    assert.equal(controller.open.value, true);
    const result = details({
      additionalPorts: [53, 5666],
      effectivePorts: [53, 5666, 7999],
    });
    assert.ok(resolveUpdate);
    resolveUpdate(result);
    await save;

    assert.equal(controller.open.value, false);
    assert.deepEqual(controller.details.value, result);
    assert.deepEqual(updated, result);
    assert.deepEqual(saved, {
      result,
      showUnsavedModeNotice: true,
    });
  });

  it("selects accurate feedback for immediate, automatic-later, and manual-later apply", () => {
    assert.equal(
      resolveFirewallAdditionalPortsSuccessMessageKey(
        { appliedNow: true },
        false,
      ),
      "savedAndAppliedDescription",
    );
    assert.equal(
      resolveFirewallAdditionalPortsSuccessMessageKey(
        { appliedNow: false },
        true,
      ),
      "savedForLaterDescription",
    );
    assert.equal(
      resolveFirewallAdditionalPortsSuccessMessageKey(
        { appliedNow: false },
        false,
      ),
      "savedForLaterManualDescription",
    );
  });

  it("keeps the entry behind the existing host-firewall capability", async () => {
    let requests = 0;
    let unsupported = 0;
    const controller = createFirewallAdditionalPortsController(
      {
        canManageHostFirewall: () => false,
        hasUnsavedModeChanges: () => false,
      },
      {
        getDetails: async () => {
          requests += 1;
          return details();
        },
        updatePorts: async () => {
          requests += 1;
          return details();
        },
        onLoadError: () => undefined,
        onSaveError: () => undefined,
        onUnsupported: () => {
          unsupported += 1;
        },
        onUpdated: () => undefined,
        onSaved: () => undefined,
      },
    );
    await controller.openDialog();
    assert.equal(requests, 0);
    assert.equal(unsupported, 1);
    assert.equal(controller.open.value, false);

    const source = await readFile(
      new URL(
        "../src/views/system-settings/RunModeSettings.vue",
        import.meta.url,
      ),
      "utf8",
    );
    assert.match(source, /<DropdownMenu v-if="canManageHostFirewall">/u);
    assert.match(source, /additionalPorts\.menu/u);
    assert.match(source, /FirewallAdditionalPortsDialog/u);
  });
});
