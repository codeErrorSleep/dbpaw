import type { ColumnSearchHighlight } from "./hooks/useColumnSearch";

export function columnSearchHighlightClass(
  column: string,
  search?: ColumnSearchHighlight,
) {
  if (search?.activeColumn === column) {
    return "bg-amber-200 text-amber-950 dark:bg-amber-800 dark:text-amber-50 ring-2 ring-inset ring-amber-500";
  }
  return search?.matchedColumns.has(column)
    ? "bg-amber-100 text-amber-950 dark:bg-amber-900 dark:text-amber-100"
    : "";
}
