import React, { useRef, useState } from "react";
import { describe, expect, test } from "bun:test";
import { act, render } from "@testing-library/react";
import { Window } from "happy-dom";
import { VirtualTableBody } from "./VirtualTableBody";

if (!globalThis.window) {
  const window = new Window();
  globalThis.window = window as unknown as Window & typeof globalThis;
  globalThis.document = window.document as unknown as Document;
  globalThis.HTMLElement = window.HTMLElement as unknown as typeof HTMLElement;
  globalThis.Node = window.Node as unknown as typeof Node;
  globalThis.MouseEvent = window.MouseEvent as unknown as typeof MouseEvent;
}

function makeRows(count: number) {
  return Array.from({ length: count }, (_, index) => ({
    id: index,
    name: `row-${index}`,
  }));
}

function noop() {}

function buildVirtualTableBodyProps(getVisibleIndex: () => number) {
  const rows = makeRows(30);
  const columns = ["id", "name"];

  return {
    columns,
    currentData: rows,
    virtualizer: {
      getTotalSize: () => rows.length * 36,
      getVirtualItems: () => {
        const index = getVisibleIndex();
        return [
          {
            index,
            start: index * 36,
            end: (index + 1) * 36,
            key: index,
          },
        ];
      },
    },
    startIndex: 0,
    showRowNumbers: true,
    showZebraStripes: false,
    showColumnComments: false,
    columnComments: {},
    getColWidth: () => 100,
    tableWidthPx: 248,
    INDEX_COL_WIDTH: 48,
    thRefs: { current: {} },
    selectedCell: null,
    selectedRows: new Set(),
    editingCell: null,
    editValue: "",
    cellSelectionRange: null,
    normalizedSearchKeyword: "",
    matchedCellKeys: new Set(),
    currentSearchMatch: null,
    isEditableForUpdates: false,
    editInputRef: { current: null },
    getCellDisplayValue: (_: number, __: string, originalValue: unknown) =>
      originalValue,
    isCellModified: () => false,
    handleCellClick: noop,
    handleCellDoubleClick: noop,
    handleCellMouseDownForRange: noop,
    handleCellMouseMoveForRange: noop,
    handleIndexMouseDown: noop,
    handleIndexMouseEnter: noop,
    handleEditKeyDown: noop,
    setEditValue: noop,
    commitEdit: noop,
    setComplexViewer: noop,
    setContextMenuRow: noop,
    handleSortClick: noop,
    handleHeaderCopy: noop,
    handleMouseDown: noop,
    insertDraftRows: [],
    handleDraftValueChange: noop,
    contextMenuRow: null,
    tableColumns: [],
    canUpdateDelete: false,
    orderByInput: "",
    getNormalizedCellRange: () => null,
    handleCopy: noop,
    handleCopySelection: noop,
    buildSelectionCSV: () => "",
    buildSelectionInsertSQL: () => "",
    buildSelectionUpdateSQL: () => "",
    buildRowsTSV: () => "",
    buildRowsCSV: () => "",
    buildRowsInsertSQL: () => "",
    buildRowsUpdateSQL: () => "",
    applyFilter: noop,
    setPendingChanges: noop,
    headerClickStateRef: { current: {} },
    t: (key: string) => key,
  } as React.ComponentProps<typeof VirtualTableBody>;
}

let triggerVisibleRangeChange: (() => void) | null = null;

function Harness() {
  const visibleIndexRef = useRef(0);
  const tablePropsRef = useRef<React.ComponentProps<typeof VirtualTableBody> | null>(
    null,
  );
  const [tick, setTick] = useState(0);
  if (!tablePropsRef.current) {
    tablePropsRef.current = buildVirtualTableBodyProps(
      () => visibleIndexRef.current,
    );
  }
  triggerVisibleRangeChange = () => {
    visibleIndexRef.current = 20;
    setTick((value) => value + 1);
  };

  return React.createElement(
    "div",
    null,
    React.createElement("span", null, tick),
    React.createElement(VirtualTableBody, tablePropsRef.current),
  );
}

describe("VirtualTableBody", () => {
  test("highlights fields even when a table has no data rows", () => {
    const props = buildVirtualTableBodyProps(() => 0);
    const { container } = render(
      React.createElement(VirtualTableBody, {
        ...props,
        currentData: [],
        virtualizer: {
          getTotalSize: () => 0,
          getVirtualItems: () => [],
        },
        columnSearch: {
          matchedColumns: new Set(["name"]),
          activeColumn: "name",
        },
      }),
    );
    const headers = container.getElementsByTagName("th");
    expect(headers[1].hasAttribute("data-field-match")).toBe(false);
    expect(headers[2].getAttribute("data-field-match")).toBe("true");
    expect(headers[2].getAttribute("data-field-active")).toBe("true");
    expect(headers[2].className).toContain("ring-amber-500");
    expect(headers[2].textContent).toContain("name");
  });

  test("updates rendered rows when the virtualizer visible range changes", () => {
    const { container } = render(React.createElement(Harness));

    expect(container.textContent).toContain("row-0");

    act(() => {
      triggerVisibleRangeChange?.();
    });

    expect(container.textContent).toContain("row-20");
  });
});
