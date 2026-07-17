/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  getPasskeyErrorDetails,
  resolvePasskeyRegistrationError,
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

  it("distinguishes cancellation and an existing credential", () => {
    assert.equal(
      resolvePasskeyRegistrationError({ name: "NotAllowedError" }, messages),
      "cancelled",
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
});
