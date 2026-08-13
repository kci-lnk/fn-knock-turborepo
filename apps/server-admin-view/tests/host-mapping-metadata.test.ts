import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { createDefaultMapping } from "../src/views/subdomain-proxy/model";
import { hasPendingHostMappingMetadata } from "../src/store/hostMappingMetadata";

const completeMapping = () => ({
  ...createDefaultMapping(),
  host: "app.example.com",
  target: "http://127.0.0.1:8080",
  title: "Application",
  favicon: "data:image/png;base64,AA==",
});

describe("host mapping metadata refresh policy", () => {
  it("refreshes incomplete metadata but ignores empty targets", () => {
    const mapping = completeMapping();
    assert.equal(hasPendingHostMappingMetadata([{ ...mapping, title: "" }]), true);
    assert.equal(hasPendingHostMappingMetadata([{ ...mapping, target: "" }]), false);
  });

  it("refreshes authenticated metadata only when its request identity changes", () => {
    const previous = completeMapping();
    previous.basic_auth = {
      enabled: true,
      username: "reader",
      password: "secret",
    };
    const unchanged = { ...previous, basic_auth: { ...previous.basic_auth } };
    assert.equal(hasPendingHostMappingMetadata([unchanged], [previous]), false);
    assert.equal(
      hasPendingHostMappingMetadata(
        [{ ...unchanged, target: "http://127.0.0.1:9090" }],
        [previous],
      ),
      true,
    );
  });
});
