import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h, nextTick, reactive, ref } from "vue";
import { createI18n } from "vue-i18n";
import { describe, expect, it, vi } from "vitest";
import type { HostLocation } from "../src/types";
import GatewayLocationMatchSection from "../src/views/system-settings/gateway-locations/GatewayLocationMatchSection.vue";
import GatewayLocationProxyFields from "../src/views/system-settings/gateway-locations/GatewayLocationProxyFields.vue";
import { createDefaultLocationForm } from "../src/views/system-settings/gateway-locations/gatewayLocationModel";
import { useGatewayLocationEditor } from "../src/views/system-settings/gateway-locations/useGatewayLocationEditor";

const createTestI18n = () =>
  createI18n({
    legacy: false,
    locale: "en",
    messages: {
      en: {
        admin: {
          gatewayLocationsSettings: {
            exactMatch: "Exact",
            exactPath: "Full exact path",
            exactPathDescription: "Matches only this complete request path.",
            matchMethod: "Match mode",
            pathForwarding: "Forwarded path",
            pathForwardingKeep: "Keep full path",
            pathForwardingStrip: "Strip matched path",
            pathPrefix: "Path prefix",
            pathPrefixDescription:
              "Matches this path prefix and all paths below it.",
            pathPreview: "Path preview",
            prefixMatch: "Prefix match",
            rewriteHtmlPath: "Rewrite HTML paths",
            rewriteHtmlPathHelp:
              "The gateway rewrites upstream HTML links and asset paths.",
            rewriteHtmlPathHelpAria: "Learn about rewriting HTML paths",
            target: "Target",
          },
        },
      },
    },
  });

const SelectStub = defineComponent({
  props: { modelValue: { type: String, required: true } },
  emits: ["update:modelValue"],
  setup(props, { emit }) {
    return () =>
      h(
        "select",
        {
          value: props.modelValue,
          onChange: (event: Event) =>
            emit(
              "update:modelValue",
              (event.target as HTMLSelectElement).value,
            ),
        },
        [
          h("option", { value: "exact" }, "Exact"),
          h("option", { value: "prefix" }, "Prefix match"),
          h("option", { value: "strip" }, "Strip matched path"),
          h("option", { value: "keep" }, "Keep full path"),
        ],
      );
  },
});

const selectStubs = {
  Select: SelectStub,
  SelectContent: true,
  SelectItem: true,
  SelectTrigger: true,
  SelectValue: true,
};

describe("gateway location rule dialog sections", () => {
  it("places match mode first and updates path semantics without clearing input", async () => {
    const form = reactive(createDefaultLocationForm());
    form.path = "/api/status";
    const wrapper = mount(GatewayLocationMatchSection, {
      props: { form },
      global: {
        plugins: [createTestI18n()],
        stubs: selectStubs,
      },
    });

    const select = wrapper.get("select");
    const pathInput = wrapper.get<HTMLInputElement>("#location-path");
    expect(
      select.element.compareDocumentPosition(pathInput.element) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(wrapper.text()).toContain("Full exact path");
    expect(wrapper.text()).toContain(
      "Matches only this complete request path.",
    );

    await select.setValue("prefix");

    expect(form.match).toBe("prefix");
    expect(form.path).toBe("/api/status");
    expect(pathInput.element.value).toBe("/api/status");
    expect(wrapper.text()).toContain("Path prefix");
    expect(wrapper.text()).toContain(
      "Matches this path prefix and all paths below it.",
    );
    wrapper.unmount();
  });

  it("exposes the HTML rewrite explanation on mouse hover", async () => {
    const form = reactive(createDefaultLocationForm());
    form.path = "/api";
    form.target = "http://upstream:8080";
    const wrapper = mount(GatewayLocationProxyFields, {
      attachTo: document.body,
      props: { form, isWebSocketTarget: false },
      global: {
        plugins: [createTestI18n()],
        stubs: {
          ...selectStubs,
          ProxyTargetInputField: true,
        },
      },
    });

    const helpButton = wrapper.get(
      'button[aria-label="Learn about rewriting HTML paths"]',
    );
    expect(document.body.textContent).not.toContain(
      "The gateway rewrites upstream HTML links and asset paths.",
    );

    helpButton.element.dispatchEvent(
      new PointerEvent("pointermove", {
        bubbles: true,
        pointerType: "mouse",
      }),
    );
    await new Promise((resolve) => setTimeout(resolve, 1));
    await nextTick();
    await flushPromises();

    expect(document.body.textContent).toContain(
      "The gateway rewrites upstream HTML links and asset paths.",
    );

    wrapper.unmount();
  });

  it("exposes the HTML rewrite explanation on keyboard focus", async () => {
    const form = reactive(createDefaultLocationForm());
    const wrapper = mount(GatewayLocationProxyFields, {
      attachTo: document.body,
      props: { form, isWebSocketTarget: false },
      global: {
        plugins: [createTestI18n()],
        stubs: {
          ...selectStubs,
          ProxyTargetInputField: true,
        },
      },
    });
    const helpButton = wrapper.get(
      'button[aria-label="Learn about rewriting HTML paths"]',
    );

    await helpButton.trigger("focus");
    await nextTick();
    await flushPromises();

    expect(document.body.textContent).toContain(
      "The gateway rewrites upstream HTML links and asset paths.",
    );
    wrapper.unmount();
  });

  it("hides HTML rewriting and expands path forwarding for WebSocket targets", () => {
    const form = reactive(createDefaultLocationForm());
    const wrapper = mount(GatewayLocationProxyFields, {
      props: { form, isWebSocketTarget: true },
      global: {
        plugins: [createTestI18n()],
        stubs: {
          ...selectStubs,
          ProxyTargetInputField: true,
        },
      },
    });

    expect(wrapper.text()).not.toContain("Rewrite HTML paths");
    expect(wrapper.get(".grid.gap-4").classes()).toContain("sm:grid-cols-1");
    wrapper.unmount();
  });

  it("keeps action-specific values while switching and serializes only the active action", async () => {
    const draftLocations = ref<HostLocation[]>([]);
    const persistLocations = vi.fn(async () => true);
    let editor!: ReturnType<typeof useGatewayLocationEditor>;
    const harness = defineComponent({
      setup() {
        editor = useGatewayLocationEditor({
          draftLocations,
          persistLocations,
        });
        return () => h("div");
      },
    });
    const wrapper = mount(harness, {
      global: { plugins: [createTestI18n()] },
    });

    editor.openCreateDialog();
    editor.form.path = "/maintenance";
    editor.form.target = "http://upstream:8080";
    editor.setAction("response");
    editor.form.response.status = 503;
    editor.form.response.body = "Temporarily unavailable";
    editor.form.headers.push({ name: "Retry-After", value: "60" });
    expect(editor.form.strip_path).toBe(false);
    expect(editor.form.rewrite_html).toBe(false);

    editor.setAction("proxy");
    expect(editor.form.target).toBe("http://upstream:8080");
    expect(editor.form.response.body).toBe("Temporarily unavailable");
    expect(editor.form.strip_path).toBe(true);
    expect(editor.form.rewrite_html).toBe(true);

    editor.setAction("response");
    await editor.saveDialogLocation();

    expect(persistLocations).toHaveBeenCalledOnce();
    expect(persistLocations.mock.calls[0]?.[0]).toEqual([
      expect.objectContaining({
        action: "response",
        path: "/maintenance",
        rewrite_html: false,
        strip_path: false,
        target: "",
        response: expect.objectContaining({
          body: "Temporarily unavailable",
          headers: { "Retry-After": "60" },
          status: 503,
        }),
      }),
    ]);
    wrapper.unmount();
  });
});
