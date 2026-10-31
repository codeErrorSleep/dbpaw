import { useEffect, useMemo, useState, type RefObject } from "react";

export interface ColumnSearchHighlight {
  matchedColumns: ReadonlySet<string>;
  activeColumn: string | null;
}

export function useColumnSearch({
  columns,
  columnComments,
  scrollContainerRef,
  viewMode,
  isLoading,
}: {
  columns: string[];
  columnComments: Record<string, string>;
  scrollContainerRef: RefObject<HTMLDivElement | null>;
  viewMode: "table" | "column";
  isLoading?: boolean;
}) {
  const [query, setQuery] = useState("");
  const [selectedColumn, setSelectedColumn] = useState<string | null>(null);
  const keyword = query.trim().toLowerCase();
  const matches = useMemo(
    () =>
      keyword
        ? columns.filter(
            (column) =>
              column.toLowerCase().includes(keyword) ||
              columnComments[column]?.toLowerCase().includes(keyword),
          )
        : [],
    [columns, columnComments, keyword],
  );
  const activeColumn =
    selectedColumn && matches.includes(selectedColumn)
      ? selectedColumn
      : (matches[0] ?? null);
  const activeIndex =
    activeColumn === null ? -1 : matches.indexOf(activeColumn);
  const highlight = useMemo<ColumnSearchHighlight>(
    () => ({ matchedColumns: new Set(matches), activeColumn }),
    [matches, activeColumn],
  );

  useEffect(() => {
    if (isLoading || activeColumn === null) return;
    const frame = requestAnimationFrame(() => {
      const container = scrollContainerRef.current;
      const target = container?.querySelector<HTMLElement>(
        `[data-field-index="${columns.indexOf(activeColumn)}"]`,
      );
      if (!container || !target) return;
      const bounds = container.getBoundingClientRect();
      const field = target.getBoundingClientRect();
      if (viewMode === "table") {
        const delta =
          field.left < bounds.left || field.width > container.clientWidth
            ? field.left - bounds.left
            : Math.max(0, field.right - bounds.left - container.clientWidth);
        if (delta) container.scrollTo({ left: container.scrollLeft + delta });
      } else {
        // The record header stays above the fields in the transposed view.
        const headerHeight =
          container.querySelector("thead")?.getBoundingClientRect().height ?? 0;
        const top = bounds.top + headerHeight;
        const delta =
          field.top < top ||
          field.height > container.clientHeight - headerHeight
            ? field.top - top
            : Math.max(0, field.bottom - bounds.top - container.clientHeight);
        if (delta) container.scrollTo({ top: container.scrollTop + delta });
      }
    });
    return () => cancelAnimationFrame(frame);
  }, [activeColumn, columns, isLoading, scrollContainerRef, viewMode]);

  const changeQuery = (value: string) => {
    setQuery(value);
    setSelectedColumn(null);
  };
  const navigate = (direction: 1 | -1) => {
    if (!matches.length) return;
    setSelectedColumn(
      matches[(activeIndex + direction + matches.length) % matches.length],
    );
  };

  return { query, changeQuery, matches, activeIndex, highlight, navigate };
}
