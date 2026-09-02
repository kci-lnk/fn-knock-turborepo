import { DOMWrapper, mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { createI18n } from "vue-i18n";
import { afterEach, describe, expect, it } from "vitest";
import type { NotificationProviderDefinition } from "../src/types";
import ProviderEditorDialog from "../src/views/event-center/notifications/ProviderEditorDialog.vue";
import WebhookHeadersEditor from "../src/views/event-center/notifications/WebhookHeadersEditor.vue";
import {
  buildSchemaPayload,
  createEditableSchemaRecord,
} from "../src/views/event-center/notifications/form-utils";

const i18n = createI18n({
  legacy: false,
  locale: "en",
  missingWarn: false,
  messages: {
    en: {
      common: { cancel: "Cancel", save: "Save" },
      admin: {
        notifications: {
          schema: {
            enabled: "Enabled",
            disabled: "Disabled",
            sensitiveConfigured: "Configured",
          },
          headers: {
            empty: "No custom request headers configured.",
            name: "Header name",
            value: "Header value",
            namePlaceholder: "Authorization",
            valuePlaceholder: "Bearer token",
            add: "Add header",
            remove: "Remove header",
            migrationTitle: "Legacy rule headers are still in use",
            migrationDescription:
              "Re-enter existing rule headers here before saving.",
            errors: {
              tooMany: "Too many headers ({max})",
              nameRequired: "Header name required",
              nameTooLong: "Header {name} name too long ({max})",
              invalidName: "Invalid header name {name}",
              reservedName: "Reserved header {name}",
              duplicateName: "Duplicate header {name}",
              valueTooLong: "Header {name} value too long ({max})",
              invalidValue: "Invalid header value {name}",
              totalTooLarge: "Headers too large ({max})",
            },
          },
          providers: {
            createDialogTitle: "Create provider",
            editDialogTitle: "Edit provider",
            dialogDescription: "Configure provider",
            name: "Name",
            providerType: "Provider type",
            selectProviderType: "Select provider type",
            createNameHelp: "Default: {name}",
            editNameHelp: "Edit the provider",
            enabledStatus: "Enabled",
            connectionConfig: "Connection config",
            testProvider: "Test provider",
          },
        },
      },
    },
  },
});

const customHeaderField = {
  key: "custom_headers",
  label: "Custom headers",
  type: "headers" as const,
  sensitive: true,
  constraints: {
    max_items: 32,
    max_name_bytes: 128,
    max_value_bytes: 8192,
    max_total_bytes: 16384,
    reserved_names: ["host", "content-type", "x-fn-knock-signature"],
  },
};

afterEach(() => {
  document.body.replaceChildren();
});

describe("WebhookHeadersEditor", () => {
  it("adds, edits, removes, and preserves ordered plaintext rows", async () => {
    const wrapper = mount(WebhookHeadersEditor, {
      props: { modelValue: [], constraints: customHeaderField.constraints },
      global: { plugins: [i18n] },
    });

    expect(wrapper.text()).toContain("No custom request headers configured.");
    await wrapper.get("button").trigger("click");
    expect(wrapper.emitted("update:modelValue")?.at(-1)).toEqual([
      [{ name: "", value: "" }],
    ]);

    await wrapper.setProps({
      modelValue: [
        { name: "Authorization", value: "Bearer one" },
        { name: "X-API-Key", value: "two" },
      ],
    });
    const inputs = wrapper.findAll("input");
    expect(inputs.map((input) => input.element.type)).toEqual([
      "text",
      "text",
      "text",
      "text",
    ]);
    expect(inputs.map((input) => input.element.value)).toEqual([
      "Authorization",
      "Bearer one",
      "X-API-Key",
      "two",
    ]);

    await inputs[3]!.setValue("updated");
    expect(wrapper.emitted("update:modelValue")?.at(-1)).toEqual([
      [
        { name: "Authorization", value: "Bearer one" },
        { name: "X-API-Key", value: "updated" },
      ],
    ]);

    await wrapper
      .findAll('button[aria-label="Remove header"]')[0]!
      .trigger("click");
    expect(wrapper.emitted("update:modelValue")?.at(-1)).toEqual([
      [{ name: "X-API-Key", value: "two" }],
    ]);
  });

  it("shows inline validation for empty, duplicate, reserved, and injected headers", () => {
    const wrapper = mount(WebhookHeadersEditor, {
      props: {
        constraints: customHeaderField.constraints,
        modelValue: [
          { name: "", value: "" },
          { name: "Host", value: "example.com" },
          { name: "host", value: "duplicate" },
          { name: "X-Token", value: "line\r\nbreak" },
          { name: "X-Tab", value: "\ttrim-bypass" },
          { name: "X-Control", value: "control\u0085" },
          { name: "\tX-Name", value: "value" },
        ],
      },
      global: { plugins: [i18n] },
    });

    expect(wrapper.text()).toContain("Header name required");
    expect(wrapper.text()).toContain("Reserved header Host");
    expect(wrapper.text()).toContain("Duplicate header host");
    expect(wrapper.text()).toContain("Invalid header value X-Token");
    expect(wrapper.text()).toContain("Invalid header value X-Tab");
    expect(wrapper.text()).toContain("Invalid header value X-Control");
    expect(wrapper.text()).toContain("Invalid header name X-Name");
    expect(wrapper.findAll('[role="alert"]')).not.toHaveLength(0);
    const inputs = wrapper.findAll("input");
    expect(inputs[0]!.attributes("aria-invalid")).toBe("true");
    expect(inputs[1]!.attributes("aria-invalid")).toBe("false");
    expect(inputs[6]!.attributes("aria-invalid")).toBe("false");
    expect(inputs[7]!.attributes("aria-invalid")).toBe("true");
    expect(inputs[7]!.attributes("aria-describedby")).toBeTruthy();
  });

  it("round-trips detail values and builds the normalized provider payload", () => {
    const editable = createEditableSchemaRecord([customHeaderField], {
      custom_headers: [
        { name: " Authorization ", value: " Bearer token " },
        { name: "X-Empty", value: "" },
      ],
    });
    expect(editable.custom_headers).toEqual([
      { name: " Authorization ", value: " Bearer token " },
      { name: "X-Empty", value: "" },
    ]);
    expect(
      buildSchemaPayload({
        fields: [customHeaderField],
        value: editable,
        editing: true,
        configuredSensitiveFields: ["custom_headers"],
      }),
    ).toEqual({
      custom_headers: [
        { name: "Authorization", value: "Bearer token" },
        { name: "X-Empty", value: "" },
      ],
    });
  });
});

describe("ProviderEditorDialog webhook headers", () => {
  it("shows the migration notice and disables both save and test while invalid", async () => {
    const definition = {
      type: "webhook",
      label: "Webhook",
      description: "Webhook",
      connection_schema: [customHeaderField],
      target_schema: [],
      sensitive_fields: ["custom_headers"],
      capabilities: {
        supports_text: true,
        supports_markdown: true,
        supports_rich_blocks: false,
        supports_actions: true,
        supports_mentions: true,
        supports_attachments: false,
        supports_provider_dedupe_key: true,
      },
    } satisfies NotificationProviderDefinition;
    mount(ProviderEditorDialog, {
      props: {
        catalog: [definition],
        connectionConfigInvalid: true,
        configuredSensitiveFields: [],
        form: {
          name: "Webhook 1",
          type: "webhook",
          enabled: true,
          connection_config: {
            custom_headers: [{ name: "", value: "" }],
          },
        },
        generatedProviderName: "Webhook 1",
        mode: "edit",
        open: true,
        saving: false,
        selectedDefinition: definition,
        showLegacyWebhookHeaderMigration: true,
        showWxPusherAlert: false,
        testingDraft: false,
      },
      global: { plugins: [i18n] },
    });
    await nextTick();

    expect(document.body.textContent).toContain(
      "Legacy rule headers are still in use",
    );
    for (const label of ["Test provider", "Save"]) {
      const button = [...document.body.querySelectorAll("button")].find(
        (candidate) => candidate.textContent?.trim() === label,
      );
      expect(button, `missing ${label} button`).toBeDefined();
      expect(new DOMWrapper(button!).attributes("disabled")).toBeDefined();
    }
  });
});
