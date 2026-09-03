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
