import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { describe, expect, it } from "vitest";

import LdapLoginControls from "../src/components/LdapLoginControls.vue";

const providers = [
  {
    id: "employees",
    name: "Employees",
    protocol: "ldap" as const,
    type: "openldap" as const,
  },
  {
    id: "partners",
    name: "Partners",
    protocol: "ldap" as const,
    type: "custom" as const,
  },
];

function mountControls(disabled = false) {
  const i18n = createI18n({
    legacy: false,
    locale: "en",
    messages: {
      en: {
        auth: {
          totpLogin: "TOTP",
          ldapLogin: "LDAP",
          ldapProvider: "Provider",
          ldapUsername: "Username",
          ldapPassword: "Password",
        },
      },
    },
  });
  return mount(LdapLoginControls, {
    props: {
      credentialKind: "ldap",
      disabled,
      password: "",
      providerId: "employees",
      providers,
      username: "",
    },
    global: { plugins: [i18n] },
  });
}

describe("LdapLoginControls", () => {
  it("emits real form changes and credential-mode switches", async () => {
    const wrapper = mountControls();
    const select = wrapper.get("select");
    const [username, password] = wrapper.findAll("input");

    await select.setValue("partners");
    await username.setValue("alice");
    await password.setValue("secret");
    await wrapper.findAll("button")[0].trigger("click");

    expect(wrapper.emitted("update:providerId")?.at(-1)).toEqual(["partners"]);
    expect(wrapper.emitted("update:username")?.at(-1)).toEqual(["alice"]);
    expect(wrapper.emitted("update:password")?.at(-1)).toEqual(["secret"]);
    expect(wrapper.emitted("update:credentialKind")?.at(-1)).toEqual(["totp"]);
  });

  it("disables every interactive control while authentication is pending", () => {
    const wrapper = mountControls(true);
    for (const control of wrapper.findAll("button, select, input")) {
      expect(control.attributes()).toHaveProperty("disabled");
    }
  });
});
