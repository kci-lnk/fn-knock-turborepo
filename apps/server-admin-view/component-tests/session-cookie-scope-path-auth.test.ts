import { createPinia, setActivePinia } from "pinia";
import { describe, expect, it } from "vitest";
import { useConfigStore } from "../src/store/config";
import type { AppConfig } from "../src/types";
import { createDefaultMapping } from "../src/views/subdomain-proxy/model";
import { createDefaultLocation } from "../src/views/system-settings/gateway-locations/gatewayLocationModel";
import { useSessionCookieScope } from "../src/views/system-settings/session-settings/useSessionCookieScope";

describe("cookie scope for path authentication", () => {
  it("includes public Hosts with required-login paths and reacts to mode changes", () => {
    setActivePinia(createPinia());
    const store = useConfigStore();
    store.config = {
      run_type: 3,
      subdomain_mode: { root_domain: "example.com", cookie_domain: "" },
      host_mappings: [
        {
          ...createDefaultMapping(),
          host: "outside.test",
          use_auth: false,
          locations: [
            {
              ...createDefaultLocation(),
              path: "/private",
              auth_mode: "require_login",
            },
          ],
        },
        {
          ...createDefaultMapping(),
          host: "app.example.com",
          use_auth: false,
          locations: [
            {
              ...createDefaultLocation(),
              path: "/private",
              auth_mode: "require_login",
            },
          ],
        },
        { ...createDefaultMapping(), host: "public.test", use_auth: false },
        {
          ...createDefaultMapping(),
          host: "auth.test",
          service_role: "auth",
          use_auth: false,
        },
      ],
    } as AppConfig;
    const scope = useSessionCookieScope();
    expect(scope.incompatibleCookieScopeHosts.value).toEqual(["outside.test"]);
    store.config.host_mappings[0]!.locations[0]!.auth_mode = "inherit";
    expect(scope.incompatibleCookieScopeHosts.value).toEqual([]);
    store.config.host_mappings[0]!.use_auth = true;
    expect(scope.incompatibleCookieScopeHosts.value).toEqual(["outside.test"]);
  });
});
