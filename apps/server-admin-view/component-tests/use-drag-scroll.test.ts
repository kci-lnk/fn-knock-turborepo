import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ref } from "vue";
import { useDragScroll } from "../src/composables/useDragScroll";

type Listener = (event: Event) => void;

interface FakeScrollElement {
  scrollWidth: number;
  clientWidth: number;
  scrollLeft: number;
  setPointerCapture: ReturnType<typeof vi.fn>;
  hasPointerCapture: ReturnType<typeof vi.fn>;
  releasePointerCapture: ReturnType<typeof vi.fn>;
  listeners: Record<string, Listener[]>;
}

const createElement = (overrides: Partial<FakeScrollElement> = {}) => {
  let scrollLeft = 100;
  const listeners: Record<string, Listener[]> = {};
  const el = {
    scrollWidth: 500,
    clientWidth: 200,
    get scrollLeft() {
      return scrollLeft;
    },
    set scrollLeft(value: number) {
      scrollLeft = value;
    },
    setPointerCapture: vi.fn(),
    hasPointerCapture: vi.fn(() => true),
    releasePointerCapture: vi.fn(),
    addEventListener: vi.fn((type: string, listener: Listener) => {
      (listeners[type] ??= []).push(listener);
    }),
    removeEventListener: vi.fn((type: string, listener: Listener) => {
      listeners[type] = (listeners[type] ?? []).filter(
        (item) => item !== listener,
      );
    }),
    listeners,
    ...overrides,
  };
  return el as unknown as HTMLElement & FakeScrollElement;
};

const downEvent = (clientX: number): PointerEvent =>
  ({
    button: 0,
    pointerType: "mouse",
    pointerId: 1,
    clientX,
    preventDefault: vi.fn(),
  }) as unknown as PointerEvent;

const moveEvent = (clientX: number): PointerEvent =>
  ({
    pointerId: 1,
    clientX,
    preventDefault: vi.fn(),
  }) as unknown as PointerEvent;

const upEvent = (): PointerEvent =>
  ({ pointerId: 1, type: "pointerup" }) as unknown as PointerEvent;

let windowListeners: Record<string, Listener[]>;

beforeEach(() => {
  windowListeners = {};
  vi.stubGlobal(
    "matchMedia",
    vi.fn(() => ({ matches: true })),
  );
  vi.spyOn(window, "addEventListener").mockImplementation(
    (type: string, listener: EventListenerOrEventListenerObject) => {
      (windowListeners[type] ??= []).push(listener as Listener);
    },
  );
  vi.spyOn(window, "removeEventListener").mockImplementation(
    (type: string, listener: EventListenerOrEventListenerObject) => {
      windowListeners[type] = (windowListeners[type] ?? []).filter(
        (item) => item !== (listener as Listener),
      );
    },
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const emitWindow = (type: string, event: Event) => {
  for (const listener of windowListeners[type] ?? []) listener(event);
};

const dispatchClick = (el: FakeScrollElement): MouseEvent => {
  const event = {
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
  } as unknown as MouseEvent;
  for (const listener of el.listeners["click"] ?? []) listener(event);
  return event;
};

describe("useDragScroll", () => {
  it("scrolls horizontally while dragging beyond the threshold", () => {
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { isDragging, onPointerDown } = useDragScroll(elRef);

    const down = downEvent(100);
    onPointerDown(down);
    expect(isDragging.value).toBe(false);
    expect(down.preventDefault).not.toHaveBeenCalled();
    expect(el.setPointerCapture).not.toHaveBeenCalled();
    expect(windowListeners["pointermove"]).toHaveLength(1);

    // Below the 6px threshold: no scrolling yet and the gesture remains a click.
    emitWindow("pointermove", moveEvent(102));
    expect(el.scrollLeft).toBe(100);
    expect(isDragging.value).toBe(false);

    // Dragging 50px to the right scrolls content 50px to the left.
    const move = moveEvent(150);
    emitWindow("pointermove", move);
    expect(el.scrollLeft).toBe(50);
    expect(isDragging.value).toBe(true);
    expect(move.preventDefault).toHaveBeenCalledTimes(1);

    emitWindow("pointerup", upEvent());
    expect(isDragging.value).toBe(false);
    expect(el.releasePointerCapture).not.toHaveBeenCalled();
    expect(windowListeners["pointermove"]).toHaveLength(0);
  });

  it("ends the drag when the pointer is released outside the element", () => {
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { isDragging, onPointerDown } = useDragScroll(elRef);

    onPointerDown(downEvent(100));
    // The pointerup lands on window (outside the area) instead of the element.
    emitWindow("pointerup", upEvent());

    expect(isDragging.value).toBe(false);
    expect(windowListeners["pointermove"]).toHaveLength(0);
    expect(windowListeners["pointerup"]).toHaveLength(0);
    expect(windowListeners["blur"]).toHaveLength(0);
  });

  it("ends the drag when the window loses focus mid-drag", () => {
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { isDragging, onPointerDown } = useDragScroll(elRef);

    onPointerDown(downEvent(100));
    emitWindow("pointermove", moveEvent(120));
    expect(isDragging.value).toBe(true);

    emitWindow("blur", new Event("blur"));
    expect(isDragging.value).toBe(false);
    expect(windowListeners["pointermove"]).toHaveLength(0);
  });

  it("suppresses the click that follows a real drag", () => {
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { onPointerDown } = useDragScroll(elRef);

    onPointerDown(downEvent(100));
    emitWindow("pointermove", moveEvent(130));
    emitWindow("pointerup", upEvent());

    const click = dispatchClick(el);
    expect(click.preventDefault).toHaveBeenCalledTimes(1);
    expect(click.stopPropagation).toHaveBeenCalledTimes(1);
  });

  it("keeps a clean click (no drag) clickable", () => {
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { onPointerDown } = useDragScroll(elRef);

    onPointerDown(downEvent(100));
    emitWindow("pointerup", upEvent());

    const click = dispatchClick(el);
    expect(click.preventDefault).not.toHaveBeenCalled();
    expect(click.stopPropagation).not.toHaveBeenCalled();
  });

  it("keeps a click with small pointer jitter clickable", () => {
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { isDragging, onPointerDown } = useDragScroll(elRef);

    const down = downEvent(100);
    onPointerDown(down);
    emitWindow("pointermove", moveEvent(105));
    emitWindow("pointerup", upEvent());

    const click = dispatchClick(el);
    expect(isDragging.value).toBe(false);
    expect(down.preventDefault).not.toHaveBeenCalled();
    expect(click.preventDefault).not.toHaveBeenCalled();
    expect(click.stopPropagation).not.toHaveBeenCalled();
  });

  it("does not suppress a later click when a drag produces no click", () => {
    vi.useFakeTimers();
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { onPointerDown } = useDragScroll(elRef);

    onPointerDown(downEvent(100));
    emitWindow("pointermove", moveEvent(130));
    emitWindow("pointerup", upEvent());
    vi.runAllTimers();

    const click = dispatchClick(el);
    expect(click.preventDefault).not.toHaveBeenCalled();
    expect(click.stopPropagation).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("ignores non-mouse pointers so touch keeps native panning", () => {
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { isDragging, onPointerDown } = useDragScroll(elRef);

    onPointerDown({
      button: 0,
      pointerType: "touch",
      pointerId: 2,
      clientX: 100,
    } as unknown as PointerEvent);
    expect(isDragging.value).toBe(false);
    expect(el.setPointerCapture).not.toHaveBeenCalled();
    expect(windowListeners["pointermove"]).toBeUndefined();
  });

  it("ignores mouse-like events on coarse non-hover touch devices", () => {
    vi.mocked(window.matchMedia).mockReturnValue({
      matches: false,
    } as MediaQueryList);
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { onPointerDown } = useDragScroll(elRef);

    onPointerDown(downEvent(100));

    expect(windowListeners["pointermove"]).toBeUndefined();
  });

  it("skips mouse drag on a touch-capable desktop PC", () => {
    vi.spyOn(window.navigator, "maxTouchPoints", "get").mockReturnValue(5);
    vi.spyOn(window.navigator, "platform", "get").mockReturnValue("Win32");
    vi.spyOn(window.navigator, "userAgent", "get").mockReturnValue(
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    );
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { onPointerDown } = useDragScroll(elRef);

    onPointerDown(downEvent(100));

    expect(windowListeners["pointermove"]).toBeUndefined();
  });

  it("preserves the first click in an unrecognized touch WebView reporting mouse input", () => {
    vi.spyOn(window.navigator, "maxTouchPoints", "get").mockReturnValue(5);
    vi.spyOn(window.navigator, "platform", "get").mockReturnValue("Linux aarch64");
    vi.spyOn(window.navigator, "userAgent", "get").mockReturnValue(
      "Unrecognized WebView",
    );
    vi.mocked(window.matchMedia).mockReturnValue({
      matches: true,
    } as MediaQueryList);
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { isDragging, onPointerDown } = useDragScroll(elRef);
    const down = downEvent(100);
    Object.assign(down, { width: 1, height: 1 });

    onPointerDown(down);
    expect(windowListeners["pointermove"]).toBeUndefined();
    const move = moveEvent(108);
    emitWindow("pointermove", move);
    emitWindow("pointerup", upEvent());
    const click = dispatchClick(el);

    expect(isDragging.value).toBe(false);
    expect(el.scrollLeft).toBe(100);
    expect(down.preventDefault).not.toHaveBeenCalled();
    expect(move.preventDefault).not.toHaveBeenCalled();
    expect(click.preventDefault).not.toHaveBeenCalled();
    expect(click.stopPropagation).not.toHaveBeenCalled();
  });

  it("ignores misclassified mouse events in an Android WebView", () => {
    vi.spyOn(window.navigator, "maxTouchPoints", "get").mockReturnValue(5);
    vi.spyOn(window.navigator, "platform", "get").mockReturnValue("Linux armv8l");
    vi.spyOn(window.navigator, "userAgent", "get").mockReturnValue(
      "Mozilla/5.0 (Linux; Android 12; HUAWEI) AppleWebKit/537.36 Mobile",
    );
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { onPointerDown } = useDragScroll(elRef);

    onPointerDown(downEvent(100));

    expect(windowListeners["pointermove"]).toBeUndefined();
  });

  it("ignores misclassified mouse events from iPadOS desktop mode", () => {
    vi.spyOn(window.navigator, "maxTouchPoints", "get").mockReturnValue(5);
    vi.spyOn(window.navigator, "platform", "get").mockReturnValue("MacIntel");
    vi.spyOn(window.navigator, "userAgent", "get").mockReturnValue(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15) AppleWebKit/605.1.15",
    );
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { onPointerDown } = useDragScroll(elRef);

    onPointerDown(downEvent(100));

    expect(windowListeners["pointermove"]).toBeUndefined();
  });

  it("ignores WebView mouse events that originated from touch", () => {
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { onPointerDown } = useDragScroll(elRef);
    const event = downEvent(100) as PointerEvent & {
      sourceCapabilities?: { firesTouchEvents?: boolean };
    };
    event.sourceCapabilities = { firesTouchEvents: true };

    onPointerDown(event);

    expect(windowListeners["pointermove"]).toBeUndefined();
  });

  it("ignores misclassified mouse events with touch contact geometry", () => {
    const el = createElement();
    const elRef = ref<HTMLElement | null>(el);
    const { onPointerDown } = useDragScroll(elRef);
    const event = downEvent(100);
    Object.assign(event, { width: 12, height: 10 });

    onPointerDown(event);

    expect(windowListeners["pointermove"]).toBeUndefined();
  });

  it("does nothing when the content fits within the viewport", () => {
    const el = createElement({ scrollWidth: 200, clientWidth: 200 });
    const elRef = ref<HTMLElement | null>(el);
    const { isDragging, onPointerDown } = useDragScroll(elRef);

    onPointerDown(downEvent(100));
    expect(isDragging.value).toBe(false);
    expect(el.setPointerCapture).not.toHaveBeenCalled();
    expect(windowListeners["pointermove"]).toBeUndefined();
  });
});
