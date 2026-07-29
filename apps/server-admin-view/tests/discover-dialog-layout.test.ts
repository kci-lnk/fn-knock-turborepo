import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

const discoverDialogs = [
  "../src/views/subdomain-proxy/SubdomainDiscoverDialog.vue",
  "../src/views/reverse-proxy/ReverseProxyDiscoverDialog.vue",
] as const;

describe("service discovery dialog layout", () => {
  for (const path of discoverDialogs) {
    it(`keeps actions visible beside long service labels in ${path}`, () => {
      const source = readSource(path);

      assert.match(source, /class="min-w-\[42rem\] table-fixed"/u);
      assert.match(
        source,
        /class="block max-w-full truncate(?: text-sm)?"\s+:title=/u,
      );
    });
  }
});
