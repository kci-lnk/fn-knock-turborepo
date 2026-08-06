/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { serializeCredential } from "../src/passkey/utils";

const bytes = (...values: number[]) => new Uint8Array(values).buffer;

describe("passkey credential serialization", () => {
  it("serializes registration transports inside the WebAuthn response", () => {
    const credential = {
      id: "credential-id",
      rawId: bytes(1, 2, 3),
      type: "public-key",
      getClientExtensionResults: () => ({ credProps: { rk: true } }),
      response: {
        attestationObject: bytes(4, 5, 6),
        clientDataJSON: bytes(7, 8, 9),
        getTransports: () => ["internal", "hybrid"],
      },
    } as unknown as PublicKeyCredential;

    const serialized = serializeCredential(credential);

    assert.equal("transports" in serialized, false);
    assert.deepEqual(serialized.response.transports, ["internal", "hybrid"]);
  });
});
