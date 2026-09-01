import { mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { createI18n } from "vue-i18n";
import { describe, expect, it } from "vitest";
import type { NotificationSchemaField } from "../src/types";
import SchemaFieldsEditor from "../src/views/event-center/notifications/SchemaFieldsEditor.vue";

const fields: NotificationSchemaField[] = [
  { key: "smtp_host", label: "SMTP host", type: "string" },
  {
    key: "smtp_password",
    label: "SMTP password",
    sensitive: true,
    type: "string",
  },
  { key: "smtp_port", label: "SMTP port", type: "number" },
  {
    key: "smtp_security",
    label: "SMTP security",
    options: [{ label: "TLS", value: "tls" }],
    type: "select",
  },
  { key: "allow_invalid_tls", label: "Allow invalid TLS", type: "boolean" },
  { key: "headers", label: "Headers", type: "json" },
];

describe("SchemaFieldsEditor", () => {
  it("associates every schema label with a unique form control id", () => {
    const TestHost = defineComponent({
      setup: () => () =>
        h("div", [
          h(SchemaFieldsEditor, { fields, modelValue: {} }),
          h(SchemaFieldsEditor, { fields, modelValue: {} }),
        ]),
    });
    const wrapper = mount(TestHost, {
      global: {
        plugins: [
          createI18n({
            legacy: false,
            locale: "en",
            messages: { en: {} },
            missingWarn: false,
          }),
        ],
      },
    });

    const labelTargets = Array.from(
      wrapper.element.querySelectorAll("label[for]"),
      (label) => label.getAttribute("for"),
    ).filter((value): value is string => Boolean(value));
    const renderedIds = new Set(
      Array.from(wrapper.element.querySelectorAll("[id]"), (element) =>
        element.getAttribute("id"),
      ).filter((value): value is string => Boolean(value)),
    );

    expect(labelTargets).toHaveLength(fields.length * 2);
    expect(new Set(labelTargets).size).toBe(fields.length * 2);
    expect(labelTargets.every((target) => renderedIds.has(target))).toBe(true);
  });
});
