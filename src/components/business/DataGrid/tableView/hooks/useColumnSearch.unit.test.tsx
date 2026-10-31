import "../../../../../../test-setup";
import { afterEach, describe, expect, mock, spyOn, test } from "bun:test";
import { act, cleanup, renderHook } from "@testing-library/react";
import { useColumnSearch } from "./useColumnSearch";

afterEach(() => {
  cleanup();
  mock.restore();
});

const columns = ["id", "created_at", "updated_at", 'odd"field'];
const columnComments = { id: "User identifier" };

function props() {
  return {
    columns,
    columnComments,
    scrollContainerRef: { current: null as HTMLDivElement | null },
    viewMode: "table" as "table" | "column",
    isLoading: false,
  };
}

describe("useColumnSearch", () => {
  test("matches names and comments case-insensitively and wraps navigation", () => {
    const { result } = renderHook(() => useColumnSearch(props()));
    act(() => result.current.changeQuery("  _AT  "));
    expect(result.current.matches).toEqual(["created_at", "updated_at"]);
    expect(result.current.highlight.activeColumn).toBe("created_at");
    expect([...result.current.highlight.matchedColumns]).toEqual([
      "created_at",
      "updated_at",
    ]);
    act(() => result.current.navigate(-1));
    expect(result.current.highlight.activeColumn).toBe("updated_at");
    act(() => result.current.navigate(1));
    expect(result.current.highlight.activeColumn).toBe("created_at");
    act(() => result.current.changeQuery("IDENTIFIER"));
    expect(result.current.matches).toEqual(["id"]);
    act(() => result.current.changeQuery("missing"));
    act(() => result.current.navigate(1));
    expect(result.current.highlight.activeColumn).toBeNull();
    act(() => result.current.changeQuery(""));
    expect(result.current.highlight.matchedColumns.size).toBe(0);
  });

  test("retains the active field on refresh and reordering, and handles removal", () => {
    const initialProps = props();
    const { result, rerender } = renderHook(useColumnSearch, {
      initialProps,
    });
    act(() => result.current.changeQuery("_at"));
    act(() => result.current.navigate(1));
    rerender({ ...initialProps, columns: [...columns].reverse() });
    expect(result.current.highlight.activeColumn).toBe("updated_at");
    expect(result.current.activeIndex).toBe(0);
    rerender({ ...initialProps, columns: ["id", "created_at"] });
    expect(result.current.highlight.activeColumn).toBe("created_at");
    rerender({ ...initialProps, columns: [] });
    expect(result.current.activeIndex).toBe(-1);
    expect(result.current.highlight.activeColumn).toBeNull();
  });

  test("scrolls the matched field into view without resetting the other axis", () => {
    const frames = new Map<number, FrameRequestCallback>();
    let frameId = 0;
    spyOn(globalThis, "requestAnimationFrame").mockImplementation(
      (callback) => {
        frames.set(++frameId, callback);
        return frameId;
      },
    );
    spyOn(globalThis, "cancelAnimationFrame").mockImplementation((id) => {
      frames.delete(id);
    });
    const flushFrames = () => {
      act(() => {
        for (const [id, callback] of frames) {
          frames.delete(id);
          callback(0);
        }
      });
    };
    const container = document.createElement("div");
    const field = document.createElement("div");
    field.dataset.fieldIndex = "3";
    container.append(field);
    Object.defineProperty(container, "clientWidth", { value: 300 });
    Object.defineProperty(container, "clientHeight", { value: 200 });
    container.getBoundingClientRect = () =>
      ({ left: 100, top: 100 }) as DOMRect;
    field.getBoundingClientRect = () =>
      ({
        left: 700,
        right: 800,
        top: 600,
        bottom: 640,
        width: 100,
        height: 40,
      }) as DOMRect;
    const calls: ScrollToOptions[] = [];
    container.scrollTo = (options: ScrollToOptions | number) => {
      if (typeof options === "object") calls.push(options);
    };
    const initialProps = {
      ...props(),
      isLoading: true,
      scrollContainerRef: { current: container },
    };
    const { result, rerender } = renderHook(useColumnSearch, { initialProps });
    act(() => result.current.changeQuery('odd"field'));
    flushFrames();
    expect(calls).toEqual([]);
    rerender({ ...initialProps, isLoading: false });
    flushFrames();
    expect(calls).toEqual([{ left: 400 }]);
    rerender({ ...initialProps, viewMode: "column", isLoading: false });
    flushFrames();
    expect(calls[1]).toEqual({ top: 340 });
  });
});
