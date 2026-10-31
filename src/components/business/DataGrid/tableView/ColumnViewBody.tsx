import React from "react";
import { formatCellValue, isComplexValue } from "./utils";
import type { TableRow } from "./types";
import type { ColumnSearchHighlight } from "./hooks/useColumnSearch";
import { columnSearchHighlightClass } from "./columnSearchHighlight";

interface ColumnViewBodyProps {
  columnSearch?: ColumnSearchHighlight;
  columns: string[];
  currentData: TableRow[];
  startIndex: number;
  showRowNumbers: boolean;
  showZebraStripes: boolean;
  showColumnComments: boolean;
  columnComments: Record<string, string>;
  normalizedSearchKeyword: string;
  matchedCellKeys: Set<string>;
  isEditableForUpdates: boolean;
  editingCell: { row: number; col: string } | null;
  selectedCell: { row: number; col: string } | null;
  editValue: string;
  editInputRef: React.RefObject<HTMLInputElement | null>;
  cellSelectionRange: {
    anchor: { row: number; colIndex: number };
    tip: { row: number; colIndex: number };
  } | null;
  getCellDisplayValue: (
    rowIndex: number,
    column: string,
    originalValue: unknown,
  ) => unknown;
  isCellModified: (rowIndex: number, column: string) => boolean;
  handleCellClick: (rowIndex: number, col: string) => void;
  handleCellDoubleClick: (
    rowIndex: number,
    col: string,
    currentValue: unknown,
  ) => void;
  handleCellMouseDownForRange: (
    e: React.MouseEvent,
    rowIndex: number,
    colIndex: number,
  ) => void;
  handleCellMouseMoveForRange: (rowIndex: number, colIndex: number) => void;
  handleEditKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  setEditValue: (value: string) => void;
  commitEdit: () => void;
  setComplexViewer: (
    viewer: { value: unknown; columnName: string } | null,
  ) => void;
}

function isCellInRange(
  rowIndex: number,
  colIndex: number,
  range: {
    anchor: { row: number; colIndex: number };
    tip: { row: number; colIndex: number };
  } | null,
): boolean {
  if (!range) return false;
  const minRow = Math.min(range.anchor.row, range.tip.row);
  const maxRow = Math.max(range.anchor.row, range.tip.row);
  const minCol = Math.min(range.anchor.colIndex, range.tip.colIndex);
  const maxCol = Math.max(range.anchor.colIndex, range.tip.colIndex);
  return (
    rowIndex >= minRow &&
    rowIndex <= maxRow &&
    colIndex >= minCol &&
    colIndex <= maxCol
  );
}

export const ColumnViewBody = React.memo(function ColumnViewBody({
  columnSearch,
  columns,
  currentData,
  startIndex,
  showRowNumbers,
  showZebraStripes,
  showColumnComments,
  columnComments,
  normalizedSearchKeyword,
  matchedCellKeys,
  isEditableForUpdates,
  editingCell,
  selectedCell,
  editValue,
  editInputRef,
  cellSelectionRange,
  getCellDisplayValue,
  isCellModified,
  handleCellClick,
  handleCellDoubleClick,
  handleCellMouseDownForRange,
  handleCellMouseMoveForRange,
  handleEditKeyDown,
  setEditValue,
  commitEdit,
  setComplexViewer,
}: ColumnViewBodyProps) {
  return (
    <table className="border-collapse w-auto">
      <colgroup>
        {showRowNumbers && <col style={{ width: 50 }} />}
        <col />
        {currentData.map((_, idx) => (
          <col key={idx} />
        ))}
      </colgroup>
      <thead className="bg-muted/90 sticky top-0 z-10">
        <tr>
          {showRowNumbers && (
            <th className="px-4 py-2 text-left text-xs font-semibold text-muted-foreground border-b border-r border-border">
              #
            </th>
          )}
          <th className="px-4 py-2 text-left text-xs font-semibold text-muted-foreground border-b border-r border-border">
            Column
          </th>
          {currentData.map((_, rowIndex) => (
            <th
              key={rowIndex}
              className="px-4 py-2 text-left text-xs font-semibold text-muted-foreground border-b border-r border-border"
            >
              Record {startIndex + rowIndex + 1}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {columns.map((column, colIndex) => (
          <tr
            key={column}
            className={[
              "hover:bg-muted/50 border-b border-border",
              showZebraStripes && colIndex % 2 === 1 ? "bg-muted/30" : "",
            ]
              .filter(Boolean)
              .join(" ")}
          >
            {showRowNumbers && (
              <td className="px-4 py-2 text-xs text-muted-foreground border-r border-border">
                {colIndex + 1}
              </td>
            )}
            <td
              data-field-index={colIndex}
              data-field-match={
                columnSearch?.matchedColumns.has(column) || undefined
              }
              data-field-active={
                columnSearch?.activeColumn === column || undefined
              }
              className={`px-4 py-2 text-xs font-semibold border-r border-border ${columnSearch?.matchedColumns.has(column) ? columnSearchHighlightClass(column, columnSearch) : "text-foreground bg-muted/30"}`}
            >
              <div>{column}</div>
              {showColumnComments && columnComments[column] && (
                <span className="block truncate text-[10px] text-muted-foreground/60 leading-tight font-normal">
                  {columnComments[column]}
                </span>
              )}
            </td>
            {currentData.map((row, rowIndex) => {
              const modified = isCellModified(rowIndex, column);
              const displayValue = getCellDisplayValue(
                rowIndex,
                column,
                row[column],
              );
              const editing =
                editingCell?.row === rowIndex && editingCell?.col === column;
              const selected =
                selectedCell?.row === rowIndex && selectedCell?.col === column;
              const inRange = isCellInRange(
                rowIndex,
                colIndex,
                cellSelectionRange,
              );
              const matched =
                normalizedSearchKeyword.length > 0 &&
                matchedCellKeys.has(`${rowIndex}::${column}`);

              return (
                <td
                  key={rowIndex}
                  className={[
                    "px-0 py-0 text-sm text-foreground font-mono border-r border-border relative group transition-all duration-150 ease-out",
                    selected && !editing
                      ? "bg-accent text-accent-foreground"
                      : "",
                    inRange && !selected && !editing ? "bg-accent" : "",
                    matched && !editing
                      ? "bg-amber-100/60 dark:bg-amber-900/20"
                      : "",
                    modified && !editing
                      ? "border-l-2 border-l-orange-400"
                      : "",
                    isEditableForUpdates ? "cursor-pointer" : "",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                  onMouseDown={(e) =>
                    handleCellMouseDownForRange(e, rowIndex, colIndex)
                  }
                  onMouseEnter={() =>
                    handleCellMouseMoveForRange(rowIndex, colIndex)
                  }
                  onClick={() => handleCellClick(rowIndex, column)}
                  onDoubleClick={() =>
                    handleCellDoubleClick(rowIndex, column, row[column])
                  }
                >
                  {editing ? (
                    <input
                      ref={editInputRef}
                      type="text"
                      autoCapitalize="none"
                      className="w-full h-full px-4 py-2 bg-background border-2 border-primary outline-none font-mono text-sm shadow-[0_0_0_3px_rgba(var(--primary)_0.15)] animate-in fade-in zoom-in-95 duration-150"
                      value={editValue}
                      onChange={(e) => setEditValue(e.target.value)}
                      onKeyDown={handleEditKeyDown}
                      onBlur={commitEdit}
                    />
                  ) : (
                    <div className="px-4 py-2 truncate">
                      {displayValue !== null && displayValue !== undefined ? (
                        <span
                          className={
                            modified
                              ? "text-orange-600 dark:text-orange-400"
                              : ""
                          }
                        >
                          {formatCellValue(displayValue)}
                        </span>
                      ) : (
                        <span className="text-muted-foreground italic">
                          NULL
                        </span>
                      )}
                      {isComplexValue(displayValue) && (
                        <button
                          className="absolute right-1 top-1/2 -translate-y-1/2 opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-foreground bg-background/80 rounded px-0.5 transition-opacity"
                          title="View structured data"
                          onMouseDown={(e) => e.stopPropagation()}
                          onClick={(e) => {
                            e.stopPropagation();
                            setComplexViewer({
                              value: displayValue,
                              columnName: column,
                            });
                          }}
                        >
                          <svg
                            width="12"
                            height="12"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            strokeWidth="2"
                            strokeLinecap="round"
                            strokeLinejoin="round"
                          >
                            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
                          </svg>
                        </button>
                      )}
                    </div>
                  )}
                </td>
              );
            })}
          </tr>
        ))}
      </tbody>
    </table>
  );
});
