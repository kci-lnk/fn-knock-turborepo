/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { downloadBlob } from "../../../packages/admin-shared/src/utils/downloadBlob";

describe("Blob downloads", () => {
  it("keeps the object URL alive until after the click stack completes", () => {
    const events: string[] = [];
    let deferredRevoke: (() => void) | undefined;
    const originalDocument = Object.getOwnPropertyDescriptor(
      globalThis,
      "document",
    );
    const originalCreateObjectURL = URL.createObjectURL;
    const originalRevokeObjectURL = URL.revokeObjectURL;
    const originalSetTimeout = globalThis.setTimeout;

    Object.defineProperty(globalThis, "document", {
      configurable: true,
      value: {
        body: {
          appendChild: () => events.push("append"),
        },
        createElement: () => ({
          href: "",
          download: "",
          click: () => events.push("click"),
          remove: () => events.push("remove"),
        }),
      },
    });
    URL.createObjectURL = () => {
      events.push("create");
      return "blob:test";
    };
    URL.revokeObjectURL = (url) => events.push(`revoke:${url}`);
    globalThis.setTimeout = ((callback: () => void, delay: number) => {
      events.push(`schedule:${delay}`);
      deferredRevoke = callback;
      return 1;
    }) as typeof setTimeout;

    try {
      downloadBlob(new Blob(["ZIP"]), "certificate.zip");
      assert.deepEqual(events, [
        "create",
        "append",
        "click",
        "remove",
        "schedule:1000",
      ]);

      deferredRevoke?.();
      assert.equal(events.at(-1), "revoke:blob:test");
    } finally {
      if (originalDocument) {
        Object.defineProperty(globalThis, "document", originalDocument);
      } else {
        Reflect.deleteProperty(globalThis, "document");
      }
      URL.createObjectURL = originalCreateObjectURL;
      URL.revokeObjectURL = originalRevokeObjectURL;
      globalThis.setTimeout = originalSetTimeout;
    }
  });
});
