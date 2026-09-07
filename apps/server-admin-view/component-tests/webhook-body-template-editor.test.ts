import { mount } from "@vue/test-utils";
import { defineComponent, h, nextTick } from "vue";
import { createI18n } from "vue-i18n";
import { describe, expect, it } from "vitest";
import { Select } from "@/components/ui/select";
import WebhookBodyTemplateEditor from "../src/views/event-center/notifications/WebhookBodyTemplateEditor.vue";
import { DEFAULT_WEBHOOK_BODY_CONSTRAINTS } from "../src/views/event-center/notifications/webhook-body";

const CodeMirrorStub = defineComponent({
  name: "CodeMirrorEditor",
  props: {
    modelValue: { type: String, required: true },
    ariaLabel: { type: String, default: "" },
  },
  emits: ["update:modelValue"],
  setup(props, { emit, expose }) {
    expose({
      insertText(value: string) {
        emit("update:modelValue", `${props.modelValue}${value}`);
      },
    });
    return () =>
      h("textarea", {
        "aria-label": props.ariaLabel,
        value: props.modelValue,
        onInput: (event: Event) =>
          emit(
            "update:modelValue",
            (event.target as HTMLTextAreaElement).value,
          ),
      });
  },
});

const i18n = createI18n({
  legacy: false,
  locale: "en",
  missingWarn: false,
  messages: {
    en: {
      admin: {
        notifications: {
          body: {
            mode: "Body mode",
            standard: "Standard body",
            inherit: "Inherit provider",
            custom: "Custom body",
            format: "Body format",
            template: "Body template",
            formatJson: "Format JSON",
            variables: "Available variables",
            variablesHelp: "Variables help",
            sampleContext: "Sample context",
            sampleHelp: "Sample help",
            preview: "Render preview",
            previewing: "Rendering",
            testing: "Sending",
            testSend: "Send test",
            missingVariables: "Missing variables",
            errors: {
              invalidMode: "Invalid mode",
              invalidFormat: "Invalid format",
              templateRequired: "Template required",
              templateTooLarge: "Template too large",
              invalidJson: "Invalid JSON {detail}",
              unclosedVariable: "Unclosed variable",
              invalidVariable: "Invalid variable {detail}",
              tooManyVariables: "Too many variables",
              invalidContentType: "Invalid Content-Type",
              contentTypeTooLong: "Content-Type too long",
              sampleTooLarge: "Sample too large",
              invalidSample: "Invalid sample",
            },
          },
        },
      },
    },
  },
});

const mountEditor = (props: Record<string, unknown>) =>
  mount(WebhookBodyTemplateEditor, {
    props: {
      constraints: {
        ...DEFAULT_WEBHOOK_BODY_CONSTRAINTS,
        scope: "target",
      },
      modelValue: { mode: "inherit" },
      ...props,
    },
    global: {
      plugins: [i18n],
      stubs: { CodeMirrorEditor: CodeMirrorStub },
    },
  });

describe("WebhookBodyTemplateEditor", () => {
  it("switches to custom mode with an editable default template", async () => {
    const wrapper = mountEditor({});
    await wrapper.findComponent(Select).vm.$emit("update:modelValue", "custom");
    const update = wrapper.emitted("update:modelValue")?.at(-1)?.[0] as Record<
      string,
      unknown
    >;
    expect(update.mode).toBe("custom");
    expect(update.format).toBe("json");
    expect(String(update.template)).toContain("{{message}}");
  });

  it("edits templates, inserts variables, and emits preview and test actions", async () => {
    const wrapper = mountEditor({
      modelValue: {
        mode: "custom",
        format: "text",
        content_type: "text/plain; charset=utf-8",
        template: "prefix ",
      },
      sampleContext: '{"event":{"payload":{"ip":"192.0.2.1"}}}',
    });
    const template = wrapper.get('textarea[aria-label="Body template"]');
    await template.setValue("changed {{message.title}}");
    expect(wrapper.emitted("update:modelValue")?.at(-1)?.[0]).toMatchObject({
      template: "changed {{message.title}}",
    });

    const variableButton = wrapper
      .findAll("button")
      .find((button) => button.text().includes("event.payload"));
    expect(variableButton).toBeDefined();
    await variableButton!.trigger("click");
    expect(wrapper.emitted("update:modelValue")?.at(-1)?.[0]).toMatchObject({
      template: "prefix {{event.payload}}",
    });

    const preview = wrapper
      .findAll("button")
      .find((button) => button.text().includes("Render preview"));
    const test = wrapper
      .findAll("button")
      .find((button) => button.text().includes("Send test"));
    await preview!.trigger("click");
    await test!.trigger("click");
    expect(wrapper.emitted("preview")).toHaveLength(1);
    expect(wrapper.emitted("test")).toHaveLength(1);
  });

  it("disables preview and test on inline errors and renders missing paths", async () => {
    const wrapper = mountEditor({
      modelValue: {
        mode: "custom",
        format: "json",
        content_type: "application/json",
        template: "{",
      },
      preview: {
        format: "json",
        content_type: "application/json",
        body: '{"missing":null}',
        byte_length: 16,
        missing_variables: ["event.payload.ip"],
      },
    });
    await nextTick();
    expect(wrapper.text()).toContain("Invalid JSON");
    expect(wrapper.text()).toContain("event.payload.ip");
    for (const label of ["Render preview", "Send test"]) {
      const button = wrapper
        .findAll("button")
        .find((candidate) => candidate.text().includes(label));
      expect(button?.attributes("disabled")).toBeDefined();
    }
  });

  it("contains long preview output without horizontal overflow", () => {
    const wrapper = mountEditor({
      preview: {
        format: "json",
        content_type: "application/json",
        body: JSON.stringify({ value: "x".repeat(2048) }),
        byte_length: 2060,
        missing_variables: ["event.payload.deeplyNestedUnbrokenPath"],
      },
    });

    expect(wrapper.classes()).toEqual(
      expect.arrayContaining(["min-w-0", "max-w-full"]),
    );
    expect(wrapper.get("pre").classes()).toEqual(
      expect.arrayContaining([
        "w-full",
        "min-w-0",
        "max-w-full",
        "overflow-x-hidden",
        "whitespace-pre-wrap",
        "[overflow-wrap:anywhere]",
      ]),
    );
  });

  it("disables preview and test when the sample context exceeds its byte limit", async () => {
    const wrapper = mountEditor({
      constraints: {
        ...DEFAULT_WEBHOOK_BODY_CONSTRAINTS,
        scope: "target",
        max_sample_bytes: 4,
      },
      sampleContext: '{"value":1}',
    });
    await nextTick();
    expect(wrapper.text()).toContain("Sample too large");
    for (const label of ["Render preview", "Send test"]) {
      const button = wrapper
        .findAll("button")
        .find((candidate) => candidate.text().includes(label));
      expect(button?.attributes("disabled")).toBeDefined();
    }
  });
});

it("groups detail variables by event and inserts a stable detail path", async () => {
  const wrapper = mountEditor({
    modelValue: { mode: "custom", format: "text", template: "" },
  });
  const section = wrapper.get('[data-testid="fact-variables"]');
  expect(section.text()).toContain("admin.notifications.body.commonFacts");
  expect(section.text()).toContain(
    "admin.eventCenter.eventTypes.FN_EVENT_AUTH_LOGOUT",
  );
  const button = section
    .findAll("button")
    .find((item) =>
      item.text().includes("message.fact_values.credential_name"),
    );
  expect(button).toBeDefined();
  await button!.trigger("click");
  expect(wrapper.emitted("update:modelValue")?.at(-1)?.[0]).toMatchObject({
    template: "{{message.fact_values.credential_name}}",
  });
});

it("uses the actual admin locale scope for every detail label", async () => {
  const { default: messages } =
    await import("../../../packages/i18n/src/messages/scopes/admin/zh-CN");
  const wrapper = mount(WebhookBodyTemplateEditor, {
    props: { modelValue: { mode: "custom", format: "text", template: "" } },
    global: {
      plugins: [
        createI18n({
          legacy: false,
          locale: "zh-CN",
          messages: { "zh-CN": messages },
        }),
      ],
      stubs: { CodeMirrorEditor: CodeMirrorStub },
    },
  });
  const section = wrapper.get('[data-testid="fact-variables"]');
  expect(section.text()).toContain("凭证名称");
  expect(section.text()).toContain("关联 TOTP");
  expect(section.text()).not.toContain("admin.notifications.");
  expect(section.text()).not.toContain("server.notifications.");
});

it("keeps the sample empty until explicitly inserted and allows clearing it", async () => {
  const wrapper = mountEditor({});
  const sample = wrapper.get('textarea[aria-label="Sample context"]');
  expect((sample.element as HTMLTextAreaElement).value).toBe("");
  const button = wrapper
    .findAll("button")
    .find((item) =>
      item.text().includes("admin.notifications.body.loadSample"),
    );
  await button!.trigger("click");
  const emitted = wrapper.emitted("update:sampleContext")!.at(-1)![0] as string;
  expect(JSON.parse(emitted).message.fact_values.credential_name).toBe("macOS");
  await wrapper.setProps({ sampleContext: emitted });
  await sample.setValue("");
  expect(wrapper.emitted("update:sampleContext")!.at(-1)![0]).toBe("");
  await wrapper.setProps({ sampleContext: "" });
  expect((sample.element as HTMLTextAreaElement).value).toBe("");
});
