import { reactive, ref } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import { WOLAPI, type WOLTargetInput } from "../src/lib/api/wol";
import { useWolTargetSsh } from "../src/views/wol-management/useWolTargetSsh";

vi.mock("@admin-shared/utils/toast", () => ({
  toast: { error: vi.fn(), success: vi.fn() },
}));

const targetForm = () =>
  reactive<WOLTargetInput>({
    name: "Office PC",
    mac: "02:11:22:33:44:55",
    relayId: null,
    broadcastAddress: null,
    ipAddress: "192.0.2.10",
    enabled: true,
    ssh: {
      enabled: true,
      host: "192.0.2.10",
      port: 22,
      username: "operator",
      platform: "linux",
      authMethod: "password",
      hostKeyAlgorithm: "",
      hostKeyFingerprint: "",
      password: "secret",
      privateKey: "",
      privateKeyPassphrase: "",
      clearCredential: false,
    },
  });

const result = {
  authenticated: true,
  privilegeReady: true,
  latencyMs: 12,
  hostKeyAlgorithm: "ssh-ed25519",
  hostKeyFingerprint: "SHA256:tested",
};

describe("WoL SSH test draft isolation", () => {
  afterEach(() => vi.restoreAllMocks());

  it("applies a successful test only to the unchanged target draft", async () => {
    const form = targetForm();
    vi.spyOn(WOLAPI, "testSsh").mockResolvedValue(result);
    const controller = useWolTargetSsh({
      editingTargetId: ref("target-1"),
      targetForm: form,
      t: (key) => key,
    });

    await controller.testSsh();

    expect(form.ssh?.hostKeyAlgorithm).toBe("ssh-ed25519");
    expect(form.ssh?.hostKeyFingerprint).toBe("SHA256:tested");
  });

  it("discards a result when connection settings changed in flight", async () => {
    const form = targetForm();
    let resolveTest!: (value: typeof result) => void;
    vi.spyOn(WOLAPI, "testSsh").mockReturnValue(
      new Promise((resolve) => {
        resolveTest = resolve;
      }),
    );
    const controller = useWolTargetSsh({
      editingTargetId: ref("target-1"),
      targetForm: form,
      t: (key) => key,
    });

    const pending = controller.testSsh();
    form.ssh!.host = "192.0.2.20";
    resolveTest(result);
    await pending;

    expect(form.ssh?.hostKeyAlgorithm).toBe("");
    expect(form.ssh?.hostKeyFingerprint).toBe("");
  });
});
