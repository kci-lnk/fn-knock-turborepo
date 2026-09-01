import { defineComponent, effectScope, h, nextTick, ref } from "vue";
import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
  useMobileDialogInputFullscreen,
} from "@/components/ui/dialog";

import SubdomainMappingDialogFooter from "../src/views/subdomain-proxy/SubdomainMappingDialogFooter.vue";
import type { SubdomainMappingDialogProps } from "../src/views/subdomain-proxy/subdomain-mapping-dialog-contract";

type MutableVisualViewport = {
  height: number;
  offsetTop: number;
  addEventListener: ReturnType<typeof vi.fn>;
  removeEventListener: ReturnType<typeof vi.fn>;
};

const originalInnerHeightDescriptor = Object.getOwnPropertyDescriptor(
  window,
  "innerHeight",
);
const originalInnerWidthDescriptor = Object.getOwnPropertyDescriptor(
  window,
  "innerWidth",
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
  width = 390,
): MutableVisualViewport => {
  const viewport = {
    height,
    offsetTop,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  };
  Object.defineProperty(window, "innerHeight", {
    configurable: true,
    value: 900,
  });
  Object.defineProperty(window, "innerWidth", {
    configurable: true,
    value: width,
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
    useMobileDialogInputFullscreen({ isDialogOpen }),
  );
  if (!keyboard) throw new Error("keyboard layout harness failed");

  const container = document.createElement("div");
  const input = document.createElement("input");
  Object.defineProperty(container, "scrollTo", { value: vi.fn() });
  Object.defineProperty(input, "scrollIntoView", { value: vi.fn() });
  container.append(input);
  document.body.append(container);

  return { container, input, isDialogOpen, keyboard, scope };
};

const focusInput = (harness: ReturnType<typeof createKeyboardHarness>) => {
  harness.input.focus();
  harness.keyboard.handleFocusIn({
    currentTarget: harness.container,
    target: harness.input,
  } as unknown as FocusEvent);
};

const blurInput = (harness: ReturnType<typeof createKeyboardHarness>) => {
  harness.input.blur();
  harness.keyboard.handleFocusOut({
    currentTarget: harness.container,
    target: harness.input,
  } as unknown as FocusEvent);
};

const createFooterDialog = (
  view: "basic" | "path-browser" = "basic",
  overrides: Record<string, unknown> = {},
) =>
  ({
    iconEditor: { isIconBusy: false },
    isGatewayAdvancedLoading: false,
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
  if (originalInnerWidthDescriptor) {
    Object.defineProperty(
      window,
      "innerWidth",
      originalInnerWidthDescriptor,
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

describe("shared dialog mobile input fullscreen layout", () => {
  it("enables the behavior by default for every shared DialogContent", async () => {
    const viewport = installViewport();
    const isOpen = ref(true);
    const DialogHarness = defineComponent({
      setup: () => () =>
        h(
          Dialog,
          { open: isOpen.value, unmountOnHide: false },
          () =>
            h(DialogContent, null, {
              default: () => [
                h(DialogTitle, null, () => "Edit"),
                h(DialogDescription, null, () => "Edit this record"),
                h("input", { "data-testid": "dialog-input" }),
              ],
            }),
        ),
    });
    const wrapper = mount(DialogHarness, { attachTo: document.body });
    await nextTick();
    const content = document.querySelector<HTMLElement>(
      '[data-slot="dialog-content"]',
    );
    const input = document.querySelector<HTMLInputElement>(
      '[data-testid="dialog-input"]',
    );
    if (!content || !input) throw new Error("shared dialog harness failed");
    Object.defineProperty(input, "scrollIntoView", { value: vi.fn() });

    input.focus();
    await nextTick();

    expect(content.dataset.inputFullscreen).toBe("true");
    expect(content.style.getPropertyValue("--dialog-input-viewport-height")).toBe(
      "900px",
    );
    expect(content.classList).toContain(
      "max-sm:!h-[var(--dialog-input-viewport-height)]",
    );

    isOpen.value = false;
    await nextTick();
    expect(content.dataset.inputFullscreen).toBe("false");
    expect(viewport.removeEventListener).toHaveBeenCalledWith(
      "resize",
      expect.any(Function),
    );
    expect(viewport.removeEventListener).toHaveBeenCalledWith(
      "scroll",
      expect.any(Function),
    );
    wrapper.unmount();
  });

  it("fills the visual viewport on focus and tracks keyboard geometry", () => {
    const viewport = installViewport();
    const harness = createKeyboardHarness();
    focusInput(harness);

    expect(harness.keyboard.isInputFullscreen.value).toBe(true);
    expect(harness.keyboard.isSoftKeyboardVisible.value).toBe(false);
    expect(harness.keyboard.contentStyle.value).toMatchObject({
      "--dialog-input-viewport-height": "900px",
      "--dialog-input-viewport-top": "0px",
    });

    viewport.height = 510;
    viewport.offsetTop = 18;
    harness.keyboard.handleViewportChange();
    expect(harness.keyboard.isSoftKeyboardVisible.value).toBe(true);
    expect(harness.keyboard.contentStyle.value).toMatchObject({
      "--dialog-input-viewport-height": "510px",
      "--dialog-input-viewport-top": "18px",
    });

    blurInput(harness);
    vi.advanceTimersByTime(0);
    expect(harness.keyboard.isInputFullscreen.value).toBe(true);

    viewport.height = 900;
    viewport.offsetTop = 0;
    harness.keyboard.handleViewportChange();
    expect(harness.keyboard.isInputFullscreen.value).toBe(false);
    expect(harness.keyboard.isSoftKeyboardVisible.value).toBe(false);
    expect(viewport.removeEventListener).toHaveBeenCalledWith(
      "resize",
      expect.any(Function),
    );
    expect(viewport.removeEventListener).toHaveBeenCalledWith(
      "scroll",
      expect.any(Function),
    );

    harness.keyboard.reset();
    expect(harness.keyboard.contentStyle.value).toMatchObject({
      "--dialog-input-viewport-height": "100dvh",
      "--dialog-input-viewport-top": "0px",
    });
    harness.scope.stop();
  });

  it("does not change scrolling or layout when a desktop input receives focus", () => {
    const viewport = installViewport(900, 0, 1024);
    const harness = createKeyboardHarness();

    focusInput(harness);

    expect(harness.keyboard.isInputFullscreen.value).toBe(false);
    expect(harness.input.scrollIntoView).not.toHaveBeenCalled();
    expect(viewport.addEventListener).not.toHaveBeenCalled();
    harness.scope.stop();
  });

  it("ignores viewport shrinkage until a dialog input starts the keyboard session", () => {
    installViewport(510, 18);
    const harness = createKeyboardHarness();

    harness.keyboard.handleViewportChange();

    expect(harness.keyboard.isInputFullscreen.value).toBe(false);
    expect(harness.keyboard.isSoftKeyboardVisible.value).toBe(false);
    harness.scope.stop();
  });

  it("does not fullscreen controls that cannot open a software keyboard", () => {
    installViewport();
    const harness = createKeyboardHarness();
    harness.input.type = "checkbox";

    focusInput(harness);

    expect(harness.keyboard.isInputFullscreen.value).toBe(false);
    expect(harness.keyboard.isSoftKeyboardVisible.value).toBe(false);
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
    focusInput(harness);
    const scrollIntoView = vi.mocked(harness.input.scrollIntoView);
    const callsBeforeReset = scrollIntoView.mock.calls.length;

    harness.keyboard.reset();
    vi.runAllTimers();

    expect(scrollIntoView).toHaveBeenCalledTimes(callsBeforeReset);
    expect(harness.keyboard.isInputFullscreen.value).toBe(false);
    harness.scope.stop();
  });

  it("releases viewport listeners when its owning scope is disposed", () => {
    const viewport = installViewport();
    const harness = createKeyboardHarness();
    focusInput(harness);

    harness.scope.stop();

    expect(viewport.removeEventListener).toHaveBeenCalledWith(
      "resize",
      expect.any(Function),
    );
    expect(viewport.removeEventListener).toHaveBeenCalledWith(
      "scroll",
      expect.any(Function),
    );
    expect(harness.keyboard.isInputFullscreen.value).toBe(false);
  });

  it("defers collapse until after a newly focused footer action can click", () => {
    installViewport();
    const harness = createKeyboardHarness();
    const button = document.createElement("button");
    const handleClick = vi.fn();
    button.addEventListener("click", handleClick);
    document.body.append(button);
    focusInput(harness);

    blurInput(harness);
    button.click();

    expect(handleClick).toHaveBeenCalledOnce();
    expect(harness.keyboard.isInputFullscreen.value).toBe(true);
    vi.advanceTimersByTime(0);
    expect(harness.keyboard.isInputFullscreen.value).toBe(false);
    harness.scope.stop();
  });

  it("falls back to the dynamic viewport when VisualViewport is unavailable", () => {
    installViewport();
    Object.defineProperty(window, "visualViewport", {
      configurable: true,
      value: undefined,
    });
    const harness = createKeyboardHarness();
    focusInput(harness);

    expect(harness.keyboard.contentStyle.value).toMatchObject({
      "--dialog-input-viewport-height": "100dvh",
      "--dialog-input-viewport-top": "0px",
    });
    expect(harness.keyboard.isInputFullscreen.value).toBe(true);
    expect(harness.input.scrollIntoView).toHaveBeenCalled();
    harness.scope.stop();
  });
});

describe("subdomain mapping dialog footer", () => {
  it("keeps the safe area until the shared dialog reports a software keyboard", () => {
    const wrapper = mount(SubdomainMappingDialogFooter, {
      props: { dialog: createFooterDialog() },
      global: { plugins: [createTestI18n()] },
    });

    expect(wrapper.classes()).toContain(
      "max-sm:group-data-[soft-keyboard-visible=false]/dialog:pb-[calc(env(safe-area-inset-bottom)+1rem)]",
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
