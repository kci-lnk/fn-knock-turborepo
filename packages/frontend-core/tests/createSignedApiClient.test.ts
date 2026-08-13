/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import CryptoJS from "crypto-js";

import { buildRequestSignature } from "../src/api/createSignedApiClient";

describe("signed API requests", () => {
  it("binds the method, normalized URI and body digest", async () => {
    const originalNow = Date.now;
    Date.now = () => 1_700_000_000_000;
    try {
      const signed = await buildRequestSignature(
        "server-only-secret",
        "post",
        "https://auth.example.com/api/auth/wol/targets/device-1/wake?audit=1",
        '{"target":"device-1"}',
      );
      const bodyDigest = CryptoJS.SHA256('{"target":"device-1"}').toString(
        CryptoJS.enc.Hex,
      );
      const message = [
        "fn-knock-v1",
        "POST",
        "/api/auth/wol/targets/device-1/wake?audit=1",
        bodyDigest,
        signed.timestamp,
        signed.nonce,
      ].join("\n");
      assert.equal(
        signed.signature,
        CryptoJS.HmacSHA256(message, "server-only-secret").toString(
          CryptoJS.enc.Hex,
        ),
      );
      assert.match(signed.nonce, /^[0-9a-f]{32}$/);

      assert.notEqual(
        signed.signature,
        (
          await buildRequestSignature(
            "server-only-secret",
            "get",
            "https://auth.example.com/api/auth/wol/targets/device-1/wake?audit=1",
            '{"target":"device-1"}',
          )
        ).signature,
      );
      assert.notEqual(
        signed.signature,
        (
          await buildRequestSignature(
            "server-only-secret",
            "post",
            "https://auth.example.com/api/auth/wol/targets/device-2/wake?audit=1",
            '{"target":"device-1"}',
          )
        ).signature,
      );
      assert.notEqual(
        signed.signature,
        (
          await buildRequestSignature(
            "server-only-secret",
            "post",
            "https://auth.example.com/api/auth/wol/targets/device-1/wake?audit=1",
            '{"target":"device-2"}',
          )
        ).signature,
      );
    } finally {
      Date.now = originalNow;
    }
  });
});
