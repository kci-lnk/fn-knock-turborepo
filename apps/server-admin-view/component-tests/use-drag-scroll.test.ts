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
  ({ pointerId: 1, clientX }) as unknown as PointerEvent;

const upEvent = (): PointerEvent =>
  ({ pointerId: 1 }) as unknown as PointerEvent;

let windowListeners: Record<string, Listener[]>;

beforeEach(() => {
  windowListeners = {};
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
    expect(isDragging.value).toBe(true);
    expect(down.preventDefault).toHaveBeenCalledTimes(1);
    expect(el.setPointerCapture).toHaveBeenCalledWith(1);
    expect(windowListeners["pointermove"]).toHaveLength(1);

    // Below the 4px threshold: no scrolling yet.
    emitWindow("pointermove", moveEvent(102));
    expect(el.scrollLeft).toBe(100);

    // Dragging 50px to the right scrolls content 50px to the left.
    emitWindow("pointermove", moveEvent(150));
    expect(el.scrollLeft).toBe(50);

    emitWindow("pointerup", upEvent());
    expect(isDragging.value).toBe(false);
    expect(el.releasePointerCapture).toHaveBeenCalledWith(1);
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
