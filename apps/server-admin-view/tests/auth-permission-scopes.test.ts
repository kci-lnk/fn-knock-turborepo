import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { ref } from "vue";
import {
  createAuthStreamAccessKey,
  normalizeAuthSubdomainAccess,
  useAuthSubdomainAccess,
} from "../src/views/auth-settings/useAuthSubdomainAccess";
import type {
  AuthAccount,
  HostMapping,
  StreamMapping,
  TOTPCredential,
} from "../src/types";

describe("authentication permission scopes", () => {
  it("normalizes and deduplicates protocol mappings by protocol and port", () => {
    assert.deepEqual(
      normalizeAuthSubdomainAccess({
        mode: "custom",
        hosts: ["HTTPS://App.Example.com/path", "app.example.com."],
        streams: [
          { protocol: " TCP ", listen_port: 2222 },
          { protocol: "tcp", listen_port: 2222 },
          { protocol: "udp", listen_port: 53 },
          { protocol: "icmp", listen_port: 7 },
          { protocol: "tcp", listen_port: 0 },
        ],
      }),
      {
        mode: "custom",
        hosts: ["app.example.com"],
        streams: [
          { protocol: "udp", listen_port: 53 },
          { protocol: "tcp", listen_port: 2222 },
        ],
      },
    );
    assert.deepEqual(normalizeAuthSubdomainAccess(null), {
      mode: "all",
      hosts: [],
      streams: [],
    });
  });

  it("offers only protected host and protocol mappings as custom scopes", () => {
    const credentials = ref<TOTPCredential[]>([]);
    const hostMappings = ref([
      {
        host: "protected.example.com",
        use_auth: true,
        service_role: "app",
        title: "Protected",
        title_override: "",
      },
      {
        host: "public.example.com",
        use_auth: false,
        service_role: "app",
        title: "Public",
        title_override: "",
      },
    ] as HostMapping[]);
    const streamMappings = ref<StreamMapping[]>([
      {
        protocol: "tcp",
        listen_port: 2222,
        target: "127.0.0.1:22",
        use_auth: true,
      },
      {
        protocol: "udp",
        listen_port: 5353,
        target: "127.0.0.1:53",
        use_auth: false,
      },
    ]);

    const wolFeatureEnabled = ref(true);
    const access = useAuthSubdomainAccess({
      credentials,
      hostMappings,
      streamMappings,
      wolFeatureEnabled,
      replaceAuthAccount: (_account: AuthAccount) => undefined,
      translate: (key) => key,
    });
    const keys = access.subdomainAccessOptions.value.map(
      (option) => option.key,
    );

    assert(keys.includes("host:__builtin_select__"));
    assert(keys.includes("host:__builtin_wol__"));
    assert(keys.includes("host:protected.example.com"));
    assert(
      keys.includes(
        createAuthStreamAccessKey({
          protocol: "tcp",
          listen_port: 2222,
        }),
      ),
    );
    assert(!keys.includes("host:public.example.com"));
    assert(!keys.includes("stream:udp:5353"));

    wolFeatureEnabled.value = false;
    access.openSubdomainAccessDialog({
      id: "wol-user",
      comment: "",
      created_at: "",
      access_scopes: [],
      subdomain_access: {
        mode: "custom",
        hosts: ["__builtin_wol__"],
        streams: [],
      },
    });
    const retainedWol = access.subdomainAccessOptions.value.find(
      (option) => option.key === "host:__builtin_wol__",
    );
    assert.equal(retainedWol?.stale, true);
    assert.equal(retainedWol?.builtin, undefined);
  });
});
