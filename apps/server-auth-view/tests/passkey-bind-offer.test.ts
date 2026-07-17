/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  passkeyBindingCopyKeys,
  shouldOfferPasskeyBinding,
} from "../src/lib/passkey-bind-offer";

describe("Passkey binding offer", () => {
  it("offers binding on another browser even when the account already has a Passkey", () => {
    assert.equal(
      shouldOfferPasskeyBinding({
        canBindPasskey: true,
        currentBrowserHasKnownPasskey: false,
        isPasskeySupported: true,
        loginMode: "totp",
      }),
      true,
    );
    assert.deepEqual(passkeyBindingCopyKeys(true), {
      button: "auth.home.addPasskey",
      hint: "auth.home.passkeyAvailableAddDevice",
    });
  });

  it("hides binding when this browser already has a known Passkey", () => {
    assert.equal(
      shouldOfferPasskeyBinding({
        canBindPasskey: true,
        currentBrowserHasKnownPasskey: true,
        isPasskeySupported: true,
        loginMode: "totp",
      }),
      false,
    );
  });

  it("does not offer binding when binding or Passkey is unavailable", () => {
    assert.equal(
      shouldOfferPasskeyBinding({
        canBindPasskey: false,
        currentBrowserHasKnownPasskey: false,
        isPasskeySupported: true,
        loginMode: "totp",
      }),
      false,
    );
    assert.equal(
      shouldOfferPasskeyBinding({
        canBindPasskey: true,
        currentBrowserHasKnownPasskey: false,
        isPasskeySupported: false,
        loginMode: "totp",
      }),
      false,
    );
    assert.equal(
      shouldOfferPasskeyBinding({
        canBindPasskey: true,
        currentBrowserHasKnownPasskey: false,
        isPasskeySupported: true,
        loginMode: "password",
      }),
      false,
    );
  });
});
