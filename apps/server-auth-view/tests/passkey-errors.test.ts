/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  getPasskeyErrorDetails,
  resolvePasskeyRegistrationError,
  shouldRetryPasskeyRegistrationWithStandardProfile,
} from "../src/lib/passkey-errors";

const messages = {
  alreadyRegistered: "already registered",
  cancelled: "cancelled",
  failed: "failed",
  unavailable: "unavailable",
};

describe("passkey registration errors", () => {
  it("maps Android's generic transient error to an actionable message", () => {
    assert.equal(
      resolvePasskeyRegistrationError(
        {
          name: "UnknownError",
          message: "The operation failed for an unknown transient reason",
        },
        messages,
      ),
      "unavailable",
    );
  });

  it("distinguishes explicit cancellation, catch-all rejection, and an existing credential", () => {
    assert.equal(
      resolvePasskeyRegistrationError({ name: "AbortError" }, messages),
      "cancelled",
    );
    assert.equal(
      resolvePasskeyRegistrationError({ name: "NotAllowedError" }, messages),
      "unavailable",
    );
    assert.equal(
      resolvePasskeyRegistrationError({ name: "InvalidStateError" }, messages),
      "already registered",
    );
  });

  it("does not expose unknown platform error messages", () => {
    assert.equal(
      resolvePasskeyRegistrationError(
        { name: "VendorError", message: "internal provider details" },
        messages,
      ),
      "failed",
    );
    assert.deepEqual(getPasskeyErrorDetails("plain error"), {
      name: "",
      message: "plain error",
    });
  });

  it("retries only Android provider failures with the standard profile", () => {
    assert.equal(
      shouldRetryPasskeyRegistrationWithStandardProfile(
        { name: "UnknownError" },
        true,
      ),
      true,
    );
    assert.equal(
      shouldRetryPasskeyRegistrationWithStandardProfile(
        { name: "NotSupportedError" },
        true,
      ),
      true,
    );
    assert.equal(
      shouldRetryPasskeyRegistrationWithStandardProfile(
        { name: "NotAllowedError" },
        true,
      ),
      false,
    );
    assert.equal(
      shouldRetryPasskeyRegistrationWithStandardProfile(
        { name: "UnknownError" },
        false,
      ),
      false,
    );
  });
});
