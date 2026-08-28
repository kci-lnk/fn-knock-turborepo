import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

describe("DDNS primary config mobile actions", () => {
  it("groups expanded inline actions into clear mobile rows", () => {
    const source = readSource(
      "../src/views/ddns-management/DDNSPrimaryConfigCard.vue",
    );
    const actions = source.slice(
      source.indexOf('<template #actions="{ collapse }">'),
    );

    assert.match(actions, /inline-class="[^"]*grid-cols-2[^"]*sm:flex/u);
    assert.match(actions, /admin\.ddns\.collapse/u);
    assert.match(actions, /admin\.ddns\.actions/u);
    assert.match(actions, /common\.cancel/u);
    assert.match(actions, /common\.save/u);
    assert.match(actions, /class="col-span-2/u);
    assert.match(actions, /admin\.ddns\.saveAndUpdate/u);
    assert.doesNotMatch(actions, /<span class="hidden sm:inline">/u);
  });

  it("keeps the floating mobile actions labelled and gives the primary action its own row", () => {
    const source = readSource(
      "../src/views/ddns-management/DDNSPrimaryConfigCard.vue",
    );
    const floating = source.slice(source.indexOf("<template #floating>"));

    assert.match(source, /floating-class="w-full sm:w-fit"/u);
    assert.match(floating, /flex-1/u);
    assert.match(floating, /basis-full/u);
    assert.match(floating, /common\.cancel/u);
    assert.match(floating, /common\.save/u);
    assert.match(floating, /admin\.ddns\.saveAndUpdate/u);
    assert.doesNotMatch(floating, /<span class="hidden sm:inline">/u);
  });
});
