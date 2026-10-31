import { memo, type Key } from "react";
import { ChevronUp, ChevronDown, ArrowUpDown } from "lucide-react";
import { ContextMenu, ContextMenuTrigger } from "@/components/ui/context-menu";
import {
  createSingleAndDoubleClickHandler,
  formatCellValue,
  isComplexValue,
} from "./utils";
import { DraftRow } from "./DraftRows";
import { TableContextMenuContent } from "./TableContextMenuContent";
import type { InsertDraftRow } from "./hooks/useCellEditing";
import type { ColumnInfo } from "@/services/api";
import type { PendingChange } from "./hooks/useCellEditing";
import type { TableContext, TableRow } from "./types";
import type { ColumnSearchHighlight } from "./hooks/useColumnSearch";
import { columnSearchHighlightClass } from "./columnSearchHighlight";

interface DataRowProps {
  rowIndex: number;
  row: TableRow;
  columns: string[];
  showRowNumbers: boolean;
  showZebraStripes: boolean;
  startIndex: number;
  isRowSelected: boolean;
  isMultiRowSelection: boolean;
  editingCell: { row: number; col: string } | null;
  selectedCell: { row: number; col: string } | null;
  cellSelectionRange: {
    anchor: { row: number; colIndex: number };
    tip: { row: number; colIndex: number };
  } | null;
  normalizedSearchKeyword: string;
  matchedCellKeys: Set<string>;
  currentSearchMatch: { row: number; col: string } | null;
  isEditableForUpdates: boolean;
  editValue: string;
  editInputRef: React.RefObject<HTMLInputElement | null>;
  getColWidth: (column: string) => number;
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
  handleIndexMouseDown: (e: React.MouseEvent, rowIndex: number) => void;
  handleIndexMouseEnter: (rowIndex: number) => void;
  handleEditKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  setEditValue: (value: string) => void;
  commitEdit: () => void;
  setComplexViewer: (
    viewer: { value: unknown; columnName: string } | null,
  ) => void;
  setContextMenuRow: (row: number | null) => void;
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

const DataRow = memo(function DataRow({
  rowIndex,
  row,
  columns,
  showRowNumbers,
  showZebraStripes,
  startIndex,
  isRowSelected,
  isMultiRowSelection,
  editingCell,
  selectedCell,
  cellSelectionRange,
  normalizedSearchKeyword,
  matchedCellKeys,
  currentSearchMatch,
  isEditableForUpdates,
  editValue,
  editInputRef,
  getColWidth,
  getCellDisplayValue,
  isCellModified,
  handleCellClick,
  handleCellDoubleClick,
  handleCellMouseDownForRange,
  handleCellMouseMoveForRange,
  handleIndexMouseDown,
  handleIndexMouseEnter,
  handleEditKeyDown,
  setEditValue,
  commitEdit,
  setComplexViewer,
  setContextMenuRow,
}: DataRowProps) {
  const isEditing = (col: string) =>
    editingCell?.row === rowIndex && editingCell?.col === col;
  const isSelected = (col: string) =>
    selectedCell?.row === rowIndex && selectedCell?.col === col;

  return (
    <tr
      className={[
        "hover:bg-muted/50 border-b border-border group",
        showZebraStripes && rowIndex % 2 === 1 ? "bg-muted/30" : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      {showRowNumbers && (
        <td
          className={[
            "px-4 py-2 text-xs text-muted-foreground border-r border-border cursor-pointer select-none",
            isRowSelected ? "bg-accent text-accent-foreground" : "",
          ]
            .filter(Boolean)
            .join(" ")}
          onMouseDown={(e) => handleIndexMouseDown(e, rowIndex)}
          onMouseEnter={() => handleIndexMouseEnter(rowIndex)}
          onContextMenu={() => {
            if (isMultiRowSelection) {
              return;
            }
            setContextMenuRow(rowIndex);
          }}
        >
          {startIndex + rowIndex + 1}
        </td>
      )}
      {columns.map((column, colIndex) => {
        const modified = isCellModified(rowIndex, column);
        const displayValue = getCellDisplayValue(rowIndex, column, row[column]);
        const editing = isEditing(column);
        const selected = isSelected(column);
        const inRange = isCellInRange(rowIndex, colIndex, cellSelectionRange);
        const matched =
          normalizedSearchKeyword.length > 0 &&
          matchedCellKeys.has(`${rowIndex}::${column}`);
        const activeSearchMatch =
          !!currentSearchMatch &&
          currentSearchMatch.row === rowIndex &&
          currentSearchMatch.col === column;

        return (
          <td
            key={column}
            data-row-index={rowIndex}
            data-col-index={colIndex}
            className={[
              "px-0 py-0 text-sm text-foreground font-mono border-r border-border relative group transition-all duration-150 ease-out",
              selected && !editing ? "bg-accent text-accent-foreground" : "",
              inRange && !selected && !editing ? "bg-accent" : "",
              isRowSelected && !selected && !editing && !inRange
                ? "bg-accent/60"
                : "",
              matched && !editing ? "bg-amber-100/60 dark:bg-amber-900/20" : "",
              activeSearchMatch && !editing
                ? "border-b-2 border-b-amber-500/70"
                : "",
              modified && !editing ? "border-l-2 border-l-orange-400" : "",
              isEditableForUpdates ? "cursor-pointer" : "",
            ]
              .filter(Boolean)
              .join(" ")}
            style={{
              width: getColWidth(column),
              minWidth: 50,
            }}
            onMouseDown={(e) =>
              handleCellMouseDownForRange(e, rowIndex, colIndex)
            }
            onMouseEnter={() => handleCellMouseMoveForRange(rowIndex, colIndex)}
            onClick={() => handleCellClick(rowIndex, column)}
            onContextMenu={() => {
              if (isMultiRowSelection) {
                return;
              }
              handleCellClick(rowIndex, column);
              setContextMenuRow(rowIndex);
            }}
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
                      modified ? "text-orange-600 dark:text-orange-400" : ""
                    }
                  >
                    {formatCellValue(displayValue)}
                  </span>
                ) : (
                  <span className="text-muted-foreground italic">NULL</span>
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
  );
});

interface VirtualTableBodyProps {
  columnSearch?: ColumnSearchHighlight;
  columns: string[];
  currentData: TableRow[];
  virtualizer: {
    getTotalSize: () => number;
    getVirtualItems: () => Array<{
      index: number;
      start: number;
      end: number;
      key: Key;
    }>;
  };
  startIndex: number;
  showRowNumbers: boolean;
  showZebraStripes: boolean;
  showColumnComments: boolean;
  columnComments: Record<string, string>;
  getColWidth: (column: string) => number;
  tableWidthPx: number;
  INDEX_COL_WIDTH: number;
  thRefs: React.MutableRefObject<Record<string, HTMLTableCellElement | null>>;
  activeSortColumn?: string;
  activeSortDirection?: "asc" | "desc";
  selectedCell: { row: number; col: string } | null;
  selectedRows: Set<number>;
  editingCell: { row: number; col: string } | null;
  editValue: string;
  cellSelectionRange: {
    anchor: { row: number; colIndex: number };
    tip: { row: number; colIndex: number };
  } | null;
  normalizedSearchKeyword: string;
  matchedCellKeys: Set<string>;
  currentSearchMatch: { row: number; col: string } | null;
  isEditableForUpdates: boolean;
  editInputRef: React.RefObject<HTMLInputElement | null>;
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
  handleIndexMouseDown: (e: React.MouseEvent, rowIndex: number) => void;
  handleIndexMouseEnter: (rowIndex: number) => void;
  handleEditKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  setEditValue: (value: string) => void;
  commitEdit: () => void;
  setComplexViewer: (
    viewer: { value: unknown; columnName: string } | null,
  ) => void;
  setContextMenuRow: (row: number | null) => void;
  handleSortClick: (column: string) => void;
  handleHeaderCopy: (column: string) => void;
  handleMouseDown: (e: React.MouseEvent, column: string) => void;
  insertDraftRows: InsertDraftRow[];
  handleDraftValueChange: (
    tempId: string,
    column: string,
    value: string,
  ) => void;
  contextMenuRow: number | null;
  tableColumns: ColumnInfo[];
  tableContext?: TableContext;
  canUpdateDelete: boolean;
  onFilterChange?: (filter: string, orderBy: string) => void;
  orderByInput: string;
  getNormalizedCellRange: () => {
    minRow: number;
    maxRow: number;
    minCol: number;
    maxCol: number;
  } | null;
  handleCopy: (text: string, label?: string) => void;
  handleCopySelection: () => void;
  buildSelectionCSV: () => string;
  buildSelectionInsertSQL: () => string;
  buildSelectionUpdateSQL: () => string;
  buildRowsTSV: (rowIndexes: number[]) => string;
  buildRowsCSV: (rowIndexes: number[]) => string;
  buildRowsInsertSQL: (rowIndexes: number[]) => string;
  buildRowsUpdateSQL: (rowIndexes: number[]) => string;
  applyFilter: (operator: string) => void;
  setPendingChanges: React.Dispatch<
    React.SetStateAction<Map<string, PendingChange>>
  >;
  headerClickStateRef: React.MutableRefObject<
    Record<string, { timerId: ReturnType<typeof setTimeout> | null }>
  >;
  t: (key: string, params?: Record<string, any>) => string;
}

export function VirtualTableBody({
  columnSearch,
  columns,
  currentData,
  virtualizer,
  startIndex,
  showRowNumbers,
  showZebraStripes,
  showColumnComments,
  columnComments,
  getColWidth,
  tableWidthPx,
  INDEX_COL_WIDTH,
  thRefs,
  activeSortColumn,
  activeSortDirection,
  selectedCell,
  selectedRows,
  editingCell,
  editValue,
  cellSelectionRange,
  normalizedSearchKeyword,
  matchedCellKeys,
  currentSearchMatch,
  isEditableForUpdates,
  editInputRef,
  getCellDisplayValue,
  isCellModified,
  handleCellClick,
  handleCellDoubleClick,
  handleCellMouseDownForRange,
  handleCellMouseMoveForRange,
  handleIndexMouseDown,
  handleIndexMouseEnter,
  handleEditKeyDown,
  setEditValue,
  commitEdit,
  setComplexViewer,
  setContextMenuRow,
  handleSortClick,
  handleHeaderCopy,
  handleMouseDown,
  insertDraftRows,
  handleDraftValueChange,
  contextMenuRow,
  tableColumns,
  tableContext,
  canUpdateDelete,
  onFilterChange,
  orderByInput,
  getNormalizedCellRange,
  handleCopy,
  handleCopySelection,
  buildSelectionCSV,
  buildSelectionInsertSQL,
  buildSelectionUpdateSQL,
  buildRowsTSV,
  buildRowsCSV,
  buildRowsInsertSQL,
  buildRowsUpdateSQL,
  applyFilter,
  setPendingChanges,
  headerClickStateRef,
  t,
}: VirtualTableBodyProps) {
  const dataRowCount = currentData.length;
  const colSpan = columns.length + (showRowNumbers ? 1 : 0);

  const virtualItems = virtualizer.getVirtualItems();
  const topSpacerHeight = virtualItems.length > 0 ? virtualItems[0].start : 0;
  const bottomSpacerHeight =
    virtualItems.length > 0
      ? virtualizer.getTotalSize() - virtualItems[virtualItems.length - 1].end
      : virtualizer.getTotalSize();

  return (
    <ContextMenu
      onOpenChange={(open) => {
        if (!open) setContextMenuRow(null);
      }}
    >
      <ContextMenuTrigger asChild>
        <table
          className="border-collapse table-fixed"
          style={{
            width: tableWidthPx,
          }}
        >
          <colgroup>
            {showRowNumbers && (
              <col className="w-12" style={{ width: INDEX_COL_WIDTH }} />
            )}
            {columns.map((column) => (
              <col
                key={column}
                style={{
                  width: getColWidth(column),
                  minWidth: 50,
                }}
              />
            ))}
          </colgroup>
          <thead className="bg-muted/90 sticky top-0 z-10">
            <tr>
              {showRowNumbers && (
                <th className="px-4 py-2 text-left text-xs font-semibold text-muted-foreground border-b border-r border-border w-12">
                  #
                </th>
              )}
              {columns.map((column, columnIndex) => {
                const isSorted = activeSortColumn === column;
                const direction = isSorted ? activeSortDirection : undefined;
                const comment = columnComments[column]?.trim();
                const headerTooltip = comment || column;
                const headerActionLabel = t("tableView.header.actionHint", {
                  column,
                });
                const headerClickState =
                  headerClickStateRef.current[column] ??
                  (headerClickStateRef.current[column] = { timerId: null });
                const headerInteraction = createSingleAndDoubleClickHandler(
                  headerClickState,
                  () => handleHeaderCopy(column),
                  () => handleSortClick(column),
                );
                return (
                  <th
                    key={column}
                    data-field-index={columnIndex}
                    data-field-match={
                      columnSearch?.matchedColumns.has(column) || undefined
                    }
                    data-field-active={
                      columnSearch?.activeColumn === column || undefined
                    }
                    ref={(el) => {
                      thRefs.current[column] = el;
                    }}
                    className={`px-4 py-2 text-left text-xs font-semibold border-b border-r border-border relative group select-none ${columnSearch?.matchedColumns.has(column) ? columnSearchHighlightClass(column, columnSearch) : "text-muted-foreground"}`}
                    style={{
                      width: getColWidth(column),
                      minWidth: 50,
                    }}
                  >
                    <div className="flex items-center justify-between pr-2">
                      <button
                        type="button"
                        className="flex flex-col items-start cursor-pointer hover:text-foreground transition-colors min-w-0 flex-1 overflow-hidden"
                        title={`${headerTooltip}\n${headerActionLabel}`}
                        aria-label={headerActionLabel}
                        onClick={headerInteraction.handleClick}
                        onDoubleClick={headerInteraction.handleDoubleClick}
                      >
                        <div className="flex items-center gap-1 w-full">
                          <span className="truncate" title={headerTooltip}>
                            {column}
                          </span>
                          <span className="flex-shrink-0 w-3.5 h-3.5 flex items-center justify-center">
                            {isSorted ? (
                              direction === "asc" ? (
                                <ChevronUp className="w-3.5 h-3.5 text-primary" />
                              ) : (
                                <ChevronDown className="w-3.5 h-3.5 text-primary" />
                              )
                            ) : (
                              <ArrowUpDown className="w-3 h-3 text-muted-foreground/40 opacity-0 group-hover:opacity-100 transition-opacity" />
                            )}
                          </span>
                        </div>
                        {showColumnComments && comment && (
                          <span className="block truncate text-[10px] text-muted-foreground/60 leading-tight font-normal">
                            {comment}
                          </span>
                        )}
                      </button>
                      <div
                        className="absolute right-0 top-0 bottom-0 w-1 cursor-col-resize hover:bg-primary/50 group-hover:bg-muted-foreground/20 select-none touch-none"
                        onMouseDown={(e) => handleMouseDown(e, column)}
                      />
                    </div>
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {/* Top spacer */}
            {topSpacerHeight > 0 && (
              <tr key="top-spacer">
                <td
                  colSpan={colSpan}
                  style={{
                    height: topSpacerHeight,
                    padding: 0,
                    border: "none",
                  }}
                />
              </tr>
            )}

            {/* Virtual items (data rows + draft rows) */}
            {virtualItems.map((virtualRow) => {
              const virtualIndex = virtualRow.index;

              // Draft rows: indices >= dataRowCount
              if (virtualIndex >= dataRowCount) {
                const draftIndex = virtualIndex - dataRowCount;
                const draft = insertDraftRows[draftIndex];
                if (!draft) return null;

                return (
                  <DraftRow
                    key={`draft-${draft.tempId}`}
                    draft={draft}
                    columns={columns}
                    showRowNumbers={showRowNumbers}
                    getColWidth={getColWidth}
                    handleDraftValueChange={handleDraftValueChange}
                  />
                );
              }

              // Data rows
              const rowIndex = virtualIndex;
              const row = currentData[rowIndex];
              if (!row || typeof row !== "object") return null;
              const isRowSelected = selectedRows.has(rowIndex);

              return (
                <DataRow
                  key={rowIndex}
                  rowIndex={rowIndex}
                  row={row}
                  columns={columns}
                  showRowNumbers={showRowNumbers}
                  showZebraStripes={showZebraStripes}
                  startIndex={startIndex}
                  isRowSelected={isRowSelected}
                  isMultiRowSelection={isRowSelected && selectedRows.size > 1}
                  editingCell={editingCell}
                  selectedCell={selectedCell}
                  cellSelectionRange={cellSelectionRange}
                  normalizedSearchKeyword={normalizedSearchKeyword}
                  matchedCellKeys={matchedCellKeys}
                  currentSearchMatch={currentSearchMatch}
                  isEditableForUpdates={isEditableForUpdates}
                  editValue={editValue}
                  editInputRef={editInputRef}
                  getColWidth={getColWidth}
                  getCellDisplayValue={getCellDisplayValue}
                  isCellModified={isCellModified}
                  handleCellClick={handleCellClick}
                  handleCellDoubleClick={handleCellDoubleClick}
                  handleCellMouseDownForRange={handleCellMouseDownForRange}
                  handleCellMouseMoveForRange={handleCellMouseMoveForRange}
                  handleIndexMouseDown={handleIndexMouseDown}
                  handleIndexMouseEnter={handleIndexMouseEnter}
                  handleEditKeyDown={handleEditKeyDown}
                  setEditValue={setEditValue}
                  commitEdit={commitEdit}
                  setComplexViewer={setComplexViewer}
                  setContextMenuRow={setContextMenuRow}
                />
              );
            })}

            {/* Bottom spacer */}
            {bottomSpacerHeight > 0 && (
              <tr key="bottom-spacer">
                <td
                  colSpan={colSpan}
                  style={{
                    height: bottomSpacerHeight,
                    padding: 0,
                    border: "none",
                  }}
                />
              </tr>
            )}
          </tbody>
        </table>
      </ContextMenuTrigger>
      <TableContextMenuContent
        contextMenuRow={contextMenuRow}
        currentData={currentData}
        selectedRows={selectedRows}
        selectedCell={selectedCell}
        columns={columns}
        tableColumns={tableColumns}
        tableContext={tableContext}
        canUpdateDelete={canUpdateDelete}
        onFilterChange={onFilterChange}
        orderByInput={orderByInput}
        getNormalizedCellRange={getNormalizedCellRange}
        handleCopy={handleCopy}
        handleCopySelection={handleCopySelection}
        buildSelectionCSV={buildSelectionCSV}
        buildSelectionInsertSQL={buildSelectionInsertSQL}
        buildSelectionUpdateSQL={buildSelectionUpdateSQL}
        buildRowsTSV={buildRowsTSV}
        buildRowsCSV={buildRowsCSV}
        buildRowsInsertSQL={buildRowsInsertSQL}
        buildRowsUpdateSQL={buildRowsUpdateSQL}
        getCellDisplayValue={getCellDisplayValue}
        isCellModified={isCellModified}
        applyFilter={applyFilter}
        setPendingChanges={setPendingChanges}
      />
    </ContextMenu>
  );
}
