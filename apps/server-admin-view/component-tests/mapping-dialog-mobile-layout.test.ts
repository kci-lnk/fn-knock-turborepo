import { effectScope, ref } from "vue";
import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import SubdomainMappingDialogFooter from "../src/views/subdomain-proxy/SubdomainMappingDialogFooter.vue";
import type { SubdomainMappingDialogProps } from "../src/views/subdomain-proxy/subdomain-mapping-dialog-contract";
import { useMappingDialogKeyboardScroll } from "../src/views/subdomain-proxy/useMappingDialogKeyboardScroll";

type MutableVisualViewport = {
  height: number;
  offsetTop: number;
};

const originalInnerHeightDescriptor = Object.getOwnPropertyDescriptor(
  window,
  "innerHeight",
);
const originalVisualViewportDescriptor = Object.getOwnPropertyDescriptor(
  window,
  "visualViewport",
);

const createTestI18n = () =>
  createI18n({
    legacy: false,
    locale: "en",
    messages: {
      en: {
        admin: {
          subdomainProxy: {
            cancel: "Cancel",
            saveMapping: "Save mapping",
            staticServe: {
              browser: {
                cancel: "Cancel browser",
                useCurrentFolder: "Use folder",
                useSelectedFile: "Use file",
              },
            },
          },
        },
      },
    },
  });

const installViewport = (
  height = 900,
  offsetTop = 0,
): MutableVisualViewport => {
  const viewport = { height, offsetTop };
  Object.defineProperty(window, "innerHeight", {
    configurable: true,
    value: 900,
  });
  Object.defineProperty(window, "visualViewport", {
    configurable: true,
    value: viewport,
  });
  return viewport;
};

const createKeyboardHarness = () => {
  const scope = effectScope();
  const isDialogOpen = ref(true);
  const keyboard = scope.run(() =>
    useMappingDialogKeyboardScroll({ isDialogOpen }),
  );
  if (!keyboard) throw new Error("keyboard layout harness failed");

  const container = document.createElement("div");
  const input = document.createElement("input");
  Object.defineProperty(container, "scrollTo", { value: vi.fn() });
  Object.defineProperty(input, "scrollIntoView", { value: vi.fn() });
  container.append(input);
  document.body.append(container);
  keyboard.setMappingDialogScrollElement(container);

  return { container, input, isDialogOpen, keyboard, scope };
};

const createFooterDialog = (
  view: "basic" | "path-browser" = "basic",
  overrides: Record<string, unknown> = {},
) =>
  ({
    iconEditor: { isIconBusy: false },
    isGatewayAdvancedLoading: false,
    isMappingDialogKeyboardActive: true,
    isMappingDialogSoftKeyboardVisible: true,
    isMappingValid: true,
    isSavingMappings: false,
    pathBrowserEditor: {
      canConfirm: true,
      cancel: vi.fn(),
      confirmSelection: vi.fn(),
      isConfirming: false,
      targetType: "directory",
    },
    visibilityEditor: { mappingDialogView: view },
    ...overrides,
  }) as unknown as SubdomainMappingDialogProps;

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  document.body.replaceChildren();
  if (originalInnerHeightDescriptor) {
    Object.defineProperty(
      window,
      "innerHeight",
      originalInnerHeightDescriptor,
    );
  }
  if (originalVisualViewportDescriptor) {
    Object.defineProperty(
      window,
      "visualViewport",
      originalVisualViewportDescriptor,
    );
  } else {
    Reflect.deleteProperty(window, "visualViewport");
  }
});

describe("subdomain mapping dialog mobile keyboard layout", () => {
  it("fills the visual viewport on focus and tracks keyboard geometry", () => {
    const viewport = installViewport();
    const harness = createKeyboardHarness();
    harness.input.focus();

    harness.keyboard.handleMappingDialogFocusIn({
      target: harness.input,
    } as unknown as FocusEvent);

    expect(harness.keyboard.isMappingDialogKeyboardActive.value).toBe(true);
    expect(harness.keyboard.isMappingDialogSoftKeyboardVisible.value).toBe(
      false,
    );
    expect(harness.keyboard.mappingDialogContentStyle.value).toMatchObject({
      "--mapping-dialog-viewport-height": "900px",
      "--mapping-dialog-viewport-top": "0px",
    });

    viewport.height = 510;
    viewport.offsetTop = 18;
    harness.keyboard.handleMappingDialogViewportResize();
    expect(harness.keyboard.isMappingDialogSoftKeyboardVisible.value).toBe(
      true,
    );
    expect(harness.keyboard.mappingDialogContentStyle.value).toMatchObject({
      "--mapping-dialog-viewport-height": "510px",
      "--mapping-dialog-viewport-top": "18px",
    });

    harness.input.blur();
    harness.keyboard.handleMappingDialogFocusOut({
      target: harness.input,
    } as unknown as FocusEvent);
    vi.advanceTimersByTime(0);
    expect(harness.keyboard.isMappingDialogKeyboardActive.value).toBe(true);

    viewport.height = 900;
    viewport.offsetTop = 0;
    harness.keyboard.handleMappingDialogViewportResize();
    expect(harness.keyboard.isMappingDialogKeyboardActive.value).toBe(false);
    expect(harness.keyboard.isMappingDialogSoftKeyboardVisible.value).toBe(
      false,
    );

    harness.keyboard.resetMappingDialogKeyboardScroll();
    expect(harness.keyboard.mappingDialogContentStyle.value).toMatchObject({
      "--mapping-dialog-viewport-height": "100dvh",
      "--mapping-dialog-viewport-top": "0px",
    });
    harness.scope.stop();
  });

  it("ignores viewport shrinkage until a mapping input starts the keyboard session", () => {
    installViewport(510, 18);
    const harness = createKeyboardHarness();

    harness.keyboard.handleMappingDialogViewportResize();

    expect(harness.keyboard.isMappingDialogKeyboardActive.value).toBe(false);
    expect(harness.keyboard.isMappingDialogSoftKeyboardVisible.value).toBe(
      false,
    );
    harness.scope.stop();
  });

  it("cancels delayed input scrolling when the dialog resets", () => {
    installViewport();
    const harness = createKeyboardHarness();
    const containerRect = {
      bottom: 700,
      height: 700,
      left: 0,
      right: 390,
      top: 0,
      width: 390,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    } as DOMRect;
    const inputRect = {
      ...containerRect,
      bottom: 136,
      height: 36,
      top: 100,
      y: 100,
    } as DOMRect;
    vi.spyOn(harness.container, "getBoundingClientRect").mockReturnValue(
      containerRect,
    );
    vi.spyOn(harness.input, "getBoundingClientRect").mockReturnValue(inputRect);
    harness.input.focus();
    harness.keyboard.handleMappingDialogFocusIn({
      target: harness.input,
    } as unknown as FocusEvent);
    const scrollIntoView = vi.mocked(harness.input.scrollIntoView);
    const callsBeforeReset = scrollIntoView.mock.calls.length;

    harness.keyboard.resetMappingDialogKeyboardScroll();
    vi.runAllTimers();

    expect(scrollIntoView).toHaveBeenCalledTimes(callsBeforeReset);
    expect(harness.keyboard.isMappingDialogKeyboardActive.value).toBe(false);
    harness.scope.stop();
  });

  it("defers collapse until after a newly focused footer action can click", () => {
    installViewport();
    const harness = createKeyboardHarness();
    const button = document.createElement("button");
    const handleClick = vi.fn();
    button.addEventListener("click", handleClick);
    document.body.append(button);
    harness.input.focus();
    harness.keyboard.handleMappingDialogFocusIn({
      target: harness.input,
    } as unknown as FocusEvent);

    harness.input.blur();
    harness.keyboard.handleMappingDialogFocusOut({
      target: harness.input,
    } as unknown as FocusEvent);
    button.click();

    expect(handleClick).toHaveBeenCalledOnce();
    expect(harness.keyboard.isMappingDialogKeyboardActive.value).toBe(true);
    vi.advanceTimersByTime(0);
    expect(harness.keyboard.isMappingDialogKeyboardActive.value).toBe(false);
    harness.scope.stop();
  });

  it("falls back to the dynamic viewport when VisualViewport is unavailable", () => {
    Object.defineProperty(window, "visualViewport", {
      configurable: true,
      value: undefined,
    });
    const harness = createKeyboardHarness();
    harness.input.focus();
    harness.keyboard.handleMappingDialogFocusIn({
      target: harness.input,
    } as unknown as FocusEvent);

    expect(harness.keyboard.mappingDialogContentStyle.value).toMatchObject({
      "--mapping-dialog-viewport-height": "100dvh",
      "--mapping-dialog-viewport-top": "0px",
    });
    harness.scope.stop();
  });
});

describe("subdomain mapping dialog footer", () => {
  it("keeps the safe area until the software keyboard is actually visible", async () => {
    const wrapper = mount(SubdomainMappingDialogFooter, {
      props: {
        dialog: createFooterDialog("basic", {
          isMappingDialogKeyboardActive: true,
          isMappingDialogSoftKeyboardVisible: false,
        }),
      },
      global: { plugins: [createTestI18n()] },
    });

    expect(wrapper.classes()).toContain(
      "max-sm:pb-[calc(env(safe-area-inset-bottom)+1rem)]",
    );
    await wrapper.setProps({
      dialog: createFooterDialog("basic", {
        isMappingDialogKeyboardActive: true,
        isMappingDialogSoftKeyboardVisible: true,
      }),
    });
    expect(wrapper.classes()).not.toContain(
      "max-sm:pb-[calc(env(safe-area-inset-bottom)+1rem)]",
    );
  });

  it("keeps cancel and save as equal mobile columns without changing actions", async () => {
    const wrapper = mount(SubdomainMappingDialogFooter, {
      props: { dialog: createFooterDialog() },
      global: { plugins: [createTestI18n()] },
    });

    expect(wrapper.classes()).toEqual(
      expect.arrayContaining(["grid", "grid-cols-2", "sm:flex"]),
    );
    const buttons = wrapper.findAll("button");
    expect(buttons.map((button) => button.text())).toEqual([
      "Cancel",
      "Save mapping",
    ]);
    expect(
      buttons.every(
        (button) =>
          button.classes().includes("w-full") &&
          button.classes().includes("sm:w-auto"),
      ),
    ).toBe(true);

    await buttons[0]?.trigger("click");
    await buttons[1]?.trigger("click");
    expect(wrapper.emitted("close")).toHaveLength(1);
    expect(wrapper.emitted("save")).toHaveLength(1);
  });

  it("uses the same horizontal order and disabled state in the path browser", async () => {
    const cancel = vi.fn();
    const confirmSelection = vi.fn();
    const dialog = createFooterDialog("path-browser", {
      pathBrowserEditor: {
        canConfirm: false,
        cancel,
        confirmSelection,
        isConfirming: false,
        targetType: "directory",
      },
    });
    const wrapper = mount(SubdomainMappingDialogFooter, {
      props: { dialog },
      global: { plugins: [createTestI18n()] },
    });
    const buttons = wrapper.findAll("button");

    expect(buttons.map((button) => button.text())).toEqual([
      "Cancel browser",
      "Use folder",
    ]);
    expect(buttons[1]?.attributes()).toHaveProperty("disabled");
    await buttons[0]?.trigger("click");
    await buttons[1]?.trigger("click");
    expect(cancel).toHaveBeenCalledOnce();
    expect(confirmSelection).not.toHaveBeenCalled();
  });
});
