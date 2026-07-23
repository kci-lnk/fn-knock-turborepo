/// <reference types="node" />

import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  composeHostFromSubdomain,
  createDefaultModeForm,
  hasSubdomainRootDomainWildcard,
} from "../src/views/subdomain-proxy/model";
import { useSubdomainModeConfig } from "../src/views/subdomain-proxy/useSubdomainModeConfig";

describe("subdomain root domain validation", () => {
  it("rejects wildcard roots before composing a Host rule", () => {
    assert.equal(hasSubdomainRootDomainWildcard("*.example.com"), true);
    assert.equal(composeHostFromSubdomain("auth", "*.example.com"), "");
  });

  it("keeps exact root domains available for Host composition", () => {
    assert.equal(hasSubdomainRootDomainWildcard("example.com"), false);
    assert.equal(
      composeHostFromSubdomain("auth", "example.com"),
      "auth.example.com",
    );
  });

  it("blocks mode saves and new mappings while a wildcard root is present", () => {
    const persisted = {
      subdomain_mode: {
        ...createDefaultModeForm(),
        root_domain: "example.com",
      },
    };
    const controller = useSubdomainModeConfig({
      getConfig: () => persisted,
      saveSubdomainMode: async () => undefined,
      translate: (key) => key,
    });

    controller.modeForm.root_domain = "*.example.com";

    assert.equal(controller.isModeValid.value, false);
    assert.equal(controller.canManageNewMappings.value, false);
    assert.equal(controller.canUseRootDomainSuffix.value, false);
    assert.equal(
      controller.rootDomainValidationMessage.value,
      "admin.subdomainProxy.rootDomainWildcardForbidden",
    );
  });
});
