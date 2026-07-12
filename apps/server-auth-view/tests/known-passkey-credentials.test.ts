/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  normalizePasskeyCredentialIds,
  useKnownPasskeyCredentials,
  type CredentialDigestStorage,
} from "../src/composables/useKnownPasskeyCredentials";

class MemoryStorage implements CredentialDigestStorage {
  readonly values = new Map<string, string>();

  getItem(key: string) {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string) {
    this.values.set(key, value);
  }

  removeItem(key: string) {
    this.values.delete(key);
  }
}

describe("known passkey credential storage", () => {
  it("normalizes, trims, and deduplicates credential ids", () => {
    assert.deepEqual(
      normalizePasskeyCredentialIds([" first ", "", "first", 1, "second"]),
      ["first", "second"],
    );
    assert.deepEqual(normalizePasskeyCredentialIds(null), []);
  });

  it("remembers digests without persisting raw credential ids", async () => {
    const storage = new MemoryStorage();
    const knownCredentials = useKnownPasskeyCredentials({
      storage,
      storageKey: "known",
      hashCredentialId: async (credentialId) => `digest:${credentialId.trim()}`,
    });

    assert.equal(
      await knownCredentials.rememberKnownPasskeyCredentialId(" credential-1 "),
      true,
    );
    assert.equal(
      await knownCredentials.rememberKnownPasskeyCredentialId("credential-1"),
      true,
    );
    assert.deepEqual(knownCredentials.readKnownPasskeyCredentialDigests(), [
      "digest:credential-1",
    ]);
    assert.equal(storage.getItem("known")?.includes("credential-1"), true);
    assert.equal(storage.getItem("known")?.includes('"credential-1"'), false);
  });

  it("matches any server credential id against the remembered digests", async () => {
    const storage = new MemoryStorage();
    const knownCredentials = useKnownPasskeyCredentials({
      storage,
      storageKey: "known",
      hashCredentialId: async (credentialId) => `digest:${credentialId}`,
    });

    knownCredentials.persistKnownPasskeyCredentialDigests(["digest:second"]);

    assert.equal(
      await knownCredentials.hasKnownPasskeyCredential(["first", "second"]),
      true,
    );
    assert.equal(
      await knownCredentials.hasKnownPasskeyCredential(["first", "third"]),
      false,
    );
  });

  it("fails closed when storage is unavailable or malformed", async () => {
    const unavailable = useKnownPasskeyCredentials({
      storage: null,
      hashCredentialId: async (credentialId) => `digest:${credentialId}`,
    });
    assert.equal(
      await unavailable.rememberKnownPasskeyCredentialId("credential"),
      false,
    );
    assert.equal(
      await unavailable.hasKnownPasskeyCredential(["credential"]),
      false,
    );

    const storage = new MemoryStorage();
    storage.setItem("known", "not-json");
    const malformed = useKnownPasskeyCredentials({
      storage,
      storageKey: "known",
    });
    assert.deepEqual(malformed.readKnownPasskeyCredentialDigests(), []);
  });
});
