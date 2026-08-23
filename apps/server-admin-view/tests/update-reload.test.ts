import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  buildCacheBustedApplicationUrl,
  claimChunkReload,
  isDynamicImportFailure,
  isUpdatedApplicationReady,
  waitForUpdatedApplication,
} from "../src/lib/update-reload";

describe("FPK update reload", () => {
  it("recognizes the restarted backend by its target version", async () => {
    let now = 0;
    let attempts = 0;
    const status = await waitForUpdatedApplication({
      targetVersion: "v2.3.5",
      previousVersion: "2.3.4",
      timeoutMs: 10_000,
      intervalMs: 1_000,
      now: () => now,
      sleep: async (delayMs) => {
        now += delayMs;
      },
      loadStatus: async () => {
        attempts += 1;
        if (attempts === 1) throw new Error("CGI backend is restarting");
        return { localVersion: attempts === 2 ? "2.3.4" : "2.3.5" };
      },
    });

    assert.equal(status?.localVersion, "2.3.5");
    assert.equal(attempts, 3);
  });

  it("does not accept the old backend and stops at the timeout", async () => {
    let now = 0;
    const status = await waitForUpdatedApplication({
      targetVersion: "2.3.5",
      previousVersion: "2.3.4",
      timeoutMs: 2_000,
      intervalMs: 1_000,
      now: () => now,
      sleep: async (delayMs) => {
        now += delayMs;
      },
      loadStatus: async () => ({ localVersion: "2.3.4" }),
    });

    assert.equal(status, null);
    assert.equal(now, 2_000);
  });

  it("falls back to detecting a version change when the target is absent", () => {
    assert.equal(
      isUpdatedApplicationReady({ localVersion: "2.3.5" }, null, "2.3.4"),
      true,
    );
    assert.equal(
      isUpdatedApplicationReady({ localVersion: "2.3.4" }, null, "2.3.4"),
      false,
    );
  });

  it("cache-busts the stable CGI document while preserving its hash route", () => {
    const url = new URL(
      buildCacheBustedApplicationUrl(
        "https://nas.example/cgi/ThirdParty/fn-knock/index.cgi/?source=desktop#/about",
        1234,
      ),
    );

    assert.equal(url.pathname, "/cgi/ThirdParty/fn-knock/index.cgi/");
    assert.equal(url.searchParams.get("source"), "desktop");
    assert.equal(url.searchParams.get("_fn_knock_reload"), "1234");
    assert.equal(url.searchParams.get("_fn_knock_reload_reason"), "update");
    assert.equal(url.hash, "#/about");
  });

  it("recovers dynamic import failures once without creating a reload loop", () => {
    const values = new Map<string, string>();
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => {
        values.set(key, value);
      },
    };

    assert.equal(
      isDynamicImportFailure(
        new Error("Failed to fetch dynamically imported module: old.js"),
      ),
      true,
    );
    const chunkError = new Error("Loading an application route failed");
    chunkError.name = "ChunkLoadError";
    assert.equal(isDynamicImportFailure(chunkError), true);
    assert.equal(
      claimChunkReload("https://nas.example/app/#/about", storage, 10_000),
      true,
    );
    assert.equal(
      claimChunkReload("https://nas.example/app/#/about", storage, 10_100),
      false,
    );
    assert.equal(
      claimChunkReload(
        "https://nas.example/app/?_fn_knock_reload=10000&_fn_knock_reload_reason=chunk#/about",
        null,
        10_100,
      ),
      false,
    );
    assert.equal(
      claimChunkReload(
        "https://nas.example/app/?_fn_knock_reload=10100&_fn_knock_reload_reason=chunk#/about",
        null,
        10_000,
      ),
      false,
    );
  });
});
