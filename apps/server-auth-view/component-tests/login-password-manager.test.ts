import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { defineComponent, ref } from "vue";
import { createI18n } from "vue-i18n";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import Login from "../src/views/Login.vue";

const api = vi.hoisted(() => ({
  bootstrap: vi.fn(),
  location: vi.fn(),
  post: vi.fn(),
}));

vi.mock("@/lib/api", () => ({
  AuthAPI: {
    getBootstrap: api.bootstrap,
    getClientLocation: api.location,
  },
  apiClient: { post: api.post },
  buildAuthApiPath: (path: string) => `/api/auth${path}`,
  fetchNoStore: vi.fn(),
}));

vi.mock("@/composables/useAuthSystemConfig", () => ({
  useAuthSystemConfig: () => ({ applyAuthSystemConfig: async () => undefined }),
}));

vi.mock("@/composables/useAuthBrowserCapabilities", () => ({
  useAuthBrowserCapabilities: () => ({
    canUseNativePow: ref(false),
    isPasskeySupported: ref(false),
    refreshBrowserCapabilities: vi.fn(),
  }),
}));

const captchaWidget = defineComponent({
  emits: ["verified", "reset"],
  setup(_props, { emit, expose }) {
    expose({ reset: () => emit("reset") });
  },
  template:
    '<button type="button" data-testid="verify-captcha" @click="$emit(\'verified\', \'test-captcha\')">Verify captcha</button>',
});

const wrappers: VueWrapper[] = [];

beforeEach(() => {
  vi.spyOn(Date, "now").mockReturnValue(1_000_000);
  api.post.mockReset().mockResolvedValue({
    data: { success: true, data: { run_type: 0 } },
  });
  api.bootstrap.mockReset();
  api.location.mockReset().mockResolvedValue({
    ip: "127.0.0.1",
    location: "",
    status: "skipped",
  });
});

afterEach(() => {
  for (const wrapper of wrappers.splice(0)) wrapper.unmount();
  vi.restoreAllMocks();
});

async function mountLogin(mode: "password" | "totp" = "password") {
  api.bootstrap.mockResolvedValue({
    auth: { authenticated: false, login_mode: mode },
    client: { ip: "127.0.0.1" },
    captcha: {
      provider: "turnstile",
      available: true,
      turnstile: { site_key: "test-site-key" },
    },
    passkey: { available: false },
    ldap: {
      providers: [
        {
          id: "employees",
          name: "Employees",
          type: "openldap",
          protocol: "ldap",
        },
      ],
    },
  });
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: { template: "<div>Authenticated</div>" } },
      { path: "/login", component: Login },
    ],
  });
  await router.push("/login");
  const wrapper = mount(Login, {
    attachTo: document.body,
    global: {
      plugins: [
        router,
        createI18n({
          legacy: false,
          locale: "en",
          missingWarn: false,
          fallbackWarn: false,
          messages: { en: {} },
        }),
      ],
      stubs: { AuthFooter: true, TurnstileWidget: captchaWidget },
    },
  });
  wrappers.push(wrapper);
  await flushPromises();
  return { wrapper, router };
}

async function verifyCaptcha(wrapper: VueWrapper) {
  await wrapper.get('[data-testid="verify-captcha"]').trigger("click");
  await flushPromises();
}

async function fillCredentials(
  wrapper: VueWrapper,
  username: string,
  password: string,
) {
  // Extensions set the DOM value and dispatch events instead of using Vue APIs.
  for (const [name, value] of Object.entries({ username, password })) {
    const field = wrapper.get<HTMLInputElement>(`input[name="${name}"]`);
    field.element.value = value;
    await field.trigger("input");
    await field.trigger("change");
  }
  await flushPromises();
}

function expectCredentialFields(wrapper: VueWrapper, prefix: string) {
  expect(wrapper.get("form").attributes("autocomplete")).toBe("on");
  for (const name of ["username", "password"]) {
    const field = wrapper.get(`input[name="${name}"]`);
    expect(field.attributes("id")).toBe(`${prefix}-${name}`);
    expect(wrapper.find(`label[for="${prefix}-${name}"]`).exists()).toBe(true);
    expect(field.attributes("type")).toBe(name === "username" ? "text" : "password");
    expect(field.attributes("autocomplete")).toBe(
      name === "username" ? "username" : "current-password",
    );
    for (const attribute of [
      "data-bwignore",
      "data-1p-ignore",
      "data-lpignore",
      "data-form-type",
    ]) {
      expect(field.element.hasAttribute(attribute)).toBe(false);
    }
  }
}

describe("auth login password-manager autofill", () => {
  it.each(["password", "ldap"] as const)(
    "submits %s credentials when autofill and submit occur in the same event turn",
    async (method) => {
      const { wrapper } = await mountLogin(method === "password" ? "password" : "totp");
      await verifyCaptcha(wrapper);
      if (method === "ldap") {
        await wrapper.findAll("button").find(
          (button) => button.text() === "auth.ldapLogin",
        )!.trigger("click");
      }
      await fillCredentials(wrapper, "previous-alice", "previous-password");

      for (const [name, value] of Object.entries({
        username: "instant-alice",
        password: "instant-password",
      })) {
        const field = wrapper.get<HTMLInputElement>(`input[name="${name}"]`);
        field.element.value = value;
        field.element.dispatchEvent(new Event("input", { bubbles: true }));
        field.element.dispatchEvent(new Event("change", { bubbles: true }));
      }
      wrapper.get("form").element.dispatchEvent(
        new Event("submit", { bubbles: true, cancelable: true }),
      );
      await flushPromises();

      expect(api.post).toHaveBeenCalledExactlyOnceWith(
        "/login",
        expect.objectContaining({
          method,
          username: "instant-alice",
          password: "instant-password",
        }),
      );
    },
  );

  it("shows password fields only after captcha and submits autofilled credentials without losing them on visibility changes", async () => {
    const { wrapper, router } = await mountLogin();
    expect(wrapper.find('input[name="username"]').exists()).toBe(false);
    expect(wrapper.find('input[name="password"]').exists()).toBe(false);
    await wrapper.get("form").trigger("submit");
    expect(api.post).not.toHaveBeenCalled();

    await verifyCaptcha(wrapper);
    expectCredentialFields(wrapper, "login");
    await fillCredentials(wrapper, " alice ", "secret-password");

    const password = wrapper.get<HTMLInputElement>("#login-password");
    await wrapper.get('button[aria-label="auth.showPassword"]').trigger("click");
    expect(password.attributes("type")).toBe("text");
    expect(password.element.value).toBe("secret-password");
    await wrapper.get('button[aria-label="auth.hidePassword"]').trigger("click");
    expect(password.attributes("type")).toBe("password");
    expect(password.element.value).toBe("secret-password");

    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(api.post).toHaveBeenCalledExactlyOnceWith(
      "/login",
      expect.objectContaining({
        method: "password",
        username: "alice",
        password: "secret-password",
        captcha: { provider: "turnstile", token: "test-captcha" },
      }),
    );
    expect(router.currentRoute.value.path).toBe("/");
  });

  it("allows filling LDAP credentials while preserving TOTP ignore attributes", async () => {
    const { wrapper } = await mountLogin("totp");
    expect(wrapper.find("#ldap-username").exists()).toBe(false);
    await verifyCaptcha(wrapper);
    const otp = wrapper.get('input[autocomplete="one-time-code"]');
    expect(otp.attributes("data-bwignore")).toBe("true");
    expect(otp.attributes("data-form-type")).toBe("other");

    const ldapButton = wrapper.findAll("button").find(
      (button) => button.text() === "auth.ldapLogin",
    );
    expect(ldapButton).toBeDefined();
    await ldapButton!.trigger("click");
    expectCredentialFields(wrapper, "ldap");
    await fillCredentials(wrapper, "directory-alice", "directory-password");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(api.post).toHaveBeenCalledExactlyOnceWith(
      "/login",
      expect.objectContaining({
        method: "ldap",
        provider_id: "employees",
        username: "directory-alice",
        password: "directory-password",
      }),
    );
  });

  it("allows autofill again after a failed login and fresh captcha verification", async () => {
    api.post.mockResolvedValueOnce({
      data: { success: false, message: "Invalid credentials" },
    });
    const { wrapper } = await mountLogin();
    await verifyCaptcha(wrapper);
    await fillCredentials(wrapper, "alice", "incorrect-password");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(wrapper.find("#login-password").exists()).toBe(false);

    vi.mocked(Date.now).mockReturnValue(1_000_500);
    await verifyCaptcha(wrapper);
    expectCredentialFields(wrapper, "login");
    expect(wrapper.get<HTMLInputElement>("#login-password").element.value).toBe("");
    await fillCredentials(wrapper, "alice", "correct-password");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(api.post).toHaveBeenCalledTimes(2);
    expect(api.post).toHaveBeenLastCalledWith(
      "/login",
      expect.objectContaining({ username: "alice", password: "correct-password" }),
    );
  });
});
