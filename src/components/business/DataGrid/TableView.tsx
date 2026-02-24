import { useState, useRef, useEffect, useCallback, useMemo } from "react";
import {
  Download,
  Filter,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  ChevronDown,
  ArrowUpDown,
  Copy,
  Table as TableIcon,
  Files,
  FileCode,
  Save,
  Undo2,
  Loader2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
  ContextMenuSeparator,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
} from "@/components/ui/context-menu";
import { api } from "@/services/api";
import { isEditableTarget, isModKey } from "@/lib/keyboard";

interface PendingChange {
  rowIndex: number;
  column: string;
  originalValue: any;
  newValue: string;
}

interface TableViewProps {
  data?: any[];
  columns?: string[];
  hideHeader?: boolean;
  total?: number;
  page?: number;
  pageSize?: number;
  executionTimeMs?: number;
  onPageChange?: (page: number) => void;
  sortColumn?: string;
  sortDirection?: "asc" | "desc";
  onSortChange?: (column: string, direction: "asc" | "desc") => void;
  filter?: string;
  orderBy?: string;
  onFilterChange?: (filter: string, orderBy: string) => void;
  onOpenDDL?: (ctx: {
    connectionId: number;
    database: string;
    schema: string;
    table: string;
  }) => void;
  onDataRefresh?: () => void;
  tableContext?: {
    connectionId: number;
    database: string;
    schema: string;
    table: string;
    driver: string;
  };
}

export function TableView({
  data = [],
  columns = [],
  hideHeader = false,
  total = 0,
  page = 1,
  pageSize = 50,
  executionTimeMs = 0,
  onPageChange,
  sortColumn: controlledSortColumn,
  sortDirection: controlledSortDirection,
  onSortChange,
  filter: controlledFilter,
  orderBy: controlledOrderBy,
  onFilterChange,
  onOpenDDL,
  onDataRefresh,
  tableContext,
}: TableViewProps) {
  const [whereInput, setWhereInput] = useState(controlledFilter || "");
  const [orderByInput, setOrderByInput] = useState(controlledOrderBy || "");
  const [columnWidths, setColumnWidths] = useState<Record<string, number>>({});

  useEffect(() => {
    setWhereInput(controlledFilter || "");
  }, [controlledFilter]);

  useEffect(() => {
    setOrderByInput(controlledOrderBy || "");
  }, [controlledOrderBy]);

  // --- Cell selection & editing state ---
  const [selectedCell, setSelectedCell] = useState<{ row: number; col: string } | null>(null);
  const [editingCell, setEditingCell] = useState<{ row: number; col: string } | null>(null);
  const [editValue, setEditValue] = useState<string>("");
  const [pendingChanges, setPendingChanges] = useState<Map<string, PendingChange>>(new Map());
  const [primaryKeys, setPrimaryKeys] = useState<string[]>([]);
  const [isSaving, setIsSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const editInputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Sort state: controlled (via props) or uncontrolled (internal state for client-side sorting)
  const [internalSortColumn, setInternalSortColumn] = useState<string | undefined>();
  const [internalSortDirection, setInternalSortDirection] = useState<"asc" | "desc" | undefined>();

  const isControlledSort = !!onSortChange;
  const activeSortColumn = isControlledSort ? controlledSortColumn : internalSortColumn;
  const activeSortDirection = isControlledSort ? controlledSortDirection : internalSortDirection;

  const handleSortClick = (column: string) => {
    if (isControlledSort) {
      // Controlled mode: delegate to parent
      if (activeSortColumn === column) {
        // Toggle direction
        onSortChange(column, activeSortDirection === "asc" ? "desc" : "asc");
      } else {
        // New column, start with asc
        onSortChange(column, "asc");
      }
    } else {
      // Uncontrolled mode: manage internally for client-side sorting
      if (internalSortColumn === column) {
        setInternalSortDirection((prev) => (prev === "asc" ? "desc" : "asc"));
      } else {
        setInternalSortColumn(column);
        setInternalSortDirection("asc");
      }
    }
  };

  // Refs for table header cells to measure actual width
  const thRefs = useRef<Record<string, HTMLTableCellElement | null>>({});

  const handleShowDDL = () => {
    if (!tableContext) return;
    onOpenDDL?.(tableContext);
  };

  // --- Fetch primary keys when tableContext is available ---
  useEffect(() => {
    if (!tableContext) {
      setPrimaryKeys([]);
      return;
    }
    api.metadata
      .getTableMetadata(
        tableContext.connectionId,
        tableContext.database,
        tableContext.schema,
        tableContext.table,
      )
      .then((meta) => {
        const pks = meta.columns
          .filter((c) => c.primaryKey)
          .map((c) => c.name);
        setPrimaryKeys(pks);
      })
      .catch((e) => {
        console.error("Failed to fetch primary keys:", e);
        setPrimaryKeys([]);
      });
  }, [tableContext?.connectionId, tableContext?.database, tableContext?.schema, tableContext?.table]);

  // Clear pending changes when data/page changes
  useEffect(() => {
    setPendingChanges(new Map());
    setEditingCell(null);
    setSelectedCell(null);
    setSaveError(null);
  }, [data, page]);

  const isEditable = !!tableContext && primaryKeys.length > 0;
  const hasPendingChanges = pendingChanges.size > 0;

  // --- Cell interaction handlers ---
  const handleCellClick = useCallback(
    (rowIndex: number, col: string) => {
      // If clicking a different cell while editing, commit current edit first
      if (editingCell && (editingCell.row !== rowIndex || editingCell.col !== col)) {
        commitEdit();
      }
      setSelectedCell({ row: rowIndex, col });
    },
    [editingCell],
  );

  const handleCellDoubleClick = useCallback(
    (rowIndex: number, col: string, currentValue: any) => {
      if (!isEditable) return;
      // Check if there's a pending change for this cell
      const key = `${rowIndex}_${col}`;
      const pending = pendingChanges.get(key);
      const value = pending ? pending.newValue : (currentValue !== null && currentValue !== undefined ? String(currentValue) : "");
      setEditingCell({ row: rowIndex, col });
      setEditValue(value);
      setSelectedCell({ row: rowIndex, col });
      // Focus input on next tick
      setTimeout(() => editInputRef.current?.focus(), 0);
    },
    [isEditable, pendingChanges],
  );

  const commitEdit = useCallback(() => {
    if (!editingCell) return;
    const { row, col } = editingCell;
    const originalValue = data[row]?.[col];
    const originalStr = originalValue !== null && originalValue !== undefined ? String(originalValue) : "";
    const key = `${row}_${col}`;

    if (editValue !== originalStr) {
      setPendingChanges((prev) => {
        const next = new Map(prev);
        next.set(key, {
          rowIndex: row,
          column: col,
          originalValue,
          newValue: editValue,
        });
        return next;
      });
    } else {
      // Value reverted to original, remove from pending
      setPendingChanges((prev) => {
        const next = new Map(prev);
        next.delete(key);
        return next;
      });
    }
    setEditingCell(null);
  }, [editingCell, editValue, data]);

  const cancelEdit = useCallback(() => {
    setEditingCell(null);
  }, []);

  const handleEditKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        commitEdit();
      } else if (e.key === "Escape") {
        e.preventDefault();
        cancelEdit();
      }
    },
    [commitEdit, cancelEdit],
  );

  const handleDiscardChanges = useCallback(() => {
    setPendingChanges(new Map());
    setEditingCell(null);
    setSaveError(null);
  }, []);

  const handleCopy = useCallback((text: string) => {
    navigator.clipboard.writeText(text);
  }, []);

  // --- SQL generation & save ---
  const escapeSQL = (value: string): string => {
    return value.replace(/'/g, "''");
  };

  // MySQL uses backticks, PostgreSQL uses double quotes
  const quoteIdent = useCallback(
    (name: string): string => {
      if (tableContext?.driver === "mysql") {
        return `\`${name}\``;
      }
      return `"${name}"`;
    },
    [tableContext?.driver],
  );

  const formatSQLValue = (
    value: string,
    originalValue: any,
    context: "execution" | "copy" = "execution",
  ): string => {
    // Handle NULL
    if (value === "" && (originalValue === null || originalValue === undefined)) {
      return "NULL";
    }

    const trimmed = value.trim();
    const numericRegex = /^-?\d+(\.\d+)?$/;

    // Check if originally numeric
    if (typeof originalValue === "number") {
      if (numericRegex.test(trimmed)) {
        return trimmed;
      }
      if (context === "execution") {
        throw new Error(`Invalid numeric value: "${value}"`);
      }
      // Fallback: quote for copy
    }
    // Check if it looks like a number (for cases where originalValue might be null)
    else if (!isNaN(Number(value)) && trimmed !== "") {
      // Only return raw if it passes strict regex
      if (numericRegex.test(trimmed)) {
        return trimmed;
      }
    }

    // Check for boolean
    if (typeof originalValue === "boolean") {
      const lower = value.toLowerCase();
      if (["true", "t", "1"].includes(lower)) return "TRUE";
      if (["false", "f", "0"].includes(lower)) return "FALSE";

      if (context === "execution") {
        throw new Error(`Invalid boolean value: "${value}"`);
      }
    }

    // Default: string with quotes
    return `'${escapeSQL(value)}'`;
  };

  const generateUpdateSQL = useCallback(() => {
    if (!tableContext || primaryKeys.length === 0) return [];

    // Group changes by rowIndex
    const changesByRow = new Map<number, PendingChange[]>();
    pendingChanges.forEach((change) => {
      const existing = changesByRow.get(change.rowIndex) || [];
      existing.push(change);
      changesByRow.set(change.rowIndex, existing);
    });

    const sqls: string[] = [];
    const { schema, table, driver } = tableContext;

    changesByRow.forEach((changes, rowIndex) => {
      const row = data[rowIndex];
      if (!row) return;

      // Build SET clause - only modified columns
      const setClauses = changes.map((c) => {
        const formattedValue = formatSQLValue(c.newValue, c.originalValue);
        return `${quoteIdent(c.column)} = ${formattedValue}`;
      });

      // Build WHERE clause using primary keys
      const whereClauses = primaryKeys.map((pk) => {
        const pkValue = row[pk];
        if (pkValue === null || pkValue === undefined) {
          return `${quoteIdent(pk)} IS NULL`;
        }
        if (typeof pkValue === "number") {
          return `${quoteIdent(pk)} = ${pkValue}`;
        }
        return `${quoteIdent(pk)} = '${escapeSQL(String(pkValue))}'`;
      });

      // MySQL: `schema`.`table`, PostgreSQL: "schema"."table"
      const tableName = driver === "mysql"
        ? `${quoteIdent(table)}`
        : `${quoteIdent(schema)}.${quoteIdent(table)}`;

      const sql = `UPDATE ${tableName} SET ${setClauses.join(", ")} WHERE ${whereClauses.join(" AND ")}`;
      sqls.push(sql);
    });

    return sqls;
  }, [tableContext, primaryKeys, pendingChanges, data, quoteIdent]);

  const handleSave = useCallback(async () => {
    if (!tableContext || !hasPendingChanges) return;

    setIsSaving(true);
    setSaveError(null);

    let sqls: string[] = [];
    try {
      sqls = generateUpdateSQL();
    } catch (err) {
      setIsSaving(false);
      setSaveError(err instanceof Error ? err.message : String(err));
      return;
    }

    if (sqls.length === 0) {
      setIsSaving(false);
      return;
    }

    const errors: string[] = [];
    for (const sql of sqls) {
      try {
        await api.query.execute(
          tableContext.connectionId,
          sql,
          tableContext.database,
        );
      } catch (e) {
        errors.push(`${sql}\n  -> ${e instanceof Error ? e.message : String(e)}`);
      }
    }

    setIsSaving(false);

    if (errors.length > 0) {
      setSaveError(`${errors.length} update(s) failed:\n${errors.join("\n")}`);
    } else {
      setPendingChanges(new Map());
      setSaveError(null);
      onDataRefresh?.();
    }
  }, [tableContext, hasPendingChanges, generateUpdateSQL, onDataRefresh]);

  // Helper: get display value for a cell (considering pending changes)
  const getCellDisplayValue = useCallback(
    (rowIndex: number, column: string, originalValue: any) => {
      const key = `${rowIndex}_${column}`;
      const pending = pendingChanges.get(key);
      if (pending) return pending.newValue;
      return originalValue;
    },
    [pendingChanges],
  );

  const isCellModified = useCallback(
    (rowIndex: number, column: string) => {
      return pendingChanges.has(`${rowIndex}_${column}`);
    },
    [pendingChanges],
  );

  const resizingRef = useRef<{
    column: string;
    startX: number;
    startWidth: number;
  } | null>(null);

  const DEFAULT_COL_WIDTH = 150;
  const INDEX_COL_WIDTH = 48; // w-12 = 3rem
  const getColWidth = useCallback(
    (column: string) => columnWidths[column] ?? DEFAULT_COL_WIDTH,
    [columnWidths],
  );
  const tableWidthPx =
    INDEX_COL_WIDTH +
    columns.reduce((sum, c) => sum + getColWidth(c), 0);

  // Client-side sorting (used in uncontrolled mode, e.g. SQL query results)
  const sortedData = useMemo(() => {
    if (isControlledSort || !activeSortColumn || !activeSortDirection) {
      return data;
    }
    const col = activeSortColumn;
    const dir = activeSortDirection;
    return [...data].sort((a, b) => {
      const va = a[col];
      const vb = b[col];
      // NULL/undefined always goes to the end
      if (va == null && vb == null) return 0;
      if (va == null) return 1;
      if (vb == null) return -1;
      // Try numeric comparison
      const numA = Number(va);
      const numB = Number(vb);
      if (!isNaN(numA) && !isNaN(numB)) {
        return dir === "asc" ? numA - numB : numB - numA;
      }
      // String comparison
      const strA = String(va);
      const strB = String(vb);
      const cmp = strA.localeCompare(strB);
      return dir === "asc" ? cmp : -cmp;
    });
  }, [data, isControlledSort, activeSortColumn, activeSortDirection]);

  // If using external pagination, totalPages is based on total count
  // Otherwise fallback to filtered data length
  const totalPages = Math.ceil((total || sortedData.length) / pageSize);

  // If external pagination is used (onPageChange provided), we assume data is already the current page
  // Otherwise we slice locally
  const currentData = onPageChange
    ? sortedData
    : sortedData.slice((page - 1) * pageSize, page * pageSize);

  // Correctly calculate start index for display
  const startIndex = (page - 1) * pageSize;

  const handlePrevPage = () => {
    if (page > 1) {
      onPageChange?.(page - 1);
    }
  };

  const handleNextPage = () => {
    if (page < totalPages) {
      onPageChange?.(page + 1);
    }
  };

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!resizingRef.current) return;
    const { column, startX, startWidth } = resizingRef.current;
    const diff = e.clientX - startX;
    const newWidth = Math.max(50, startWidth + diff); // Min width 50px
    setColumnWidths((prev) => ({ ...prev, [column]: newWidth }));
  }, []);

  const handleMouseUp = useCallback(() => {
    resizingRef.current = null;
    document.removeEventListener("mousemove", handleMouseMove);
    document.removeEventListener("mouseup", handleMouseUp);
    document.body.style.cursor = "default";
  }, [handleMouseMove]);

  const handleMouseDown = (e: React.MouseEvent, column: string) => {
    e.preventDefault();
    e.stopPropagation();

    // Get the current actual width from the DOM element
    const currentTh = thRefs.current[column];
    const startWidth = currentTh
      ? currentTh.getBoundingClientRect().width
      : getColWidth(column);

    resizingRef.current = { column, startX: e.clientX, startWidth };
    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
    document.body.style.cursor = "col-resize";
  };

  useEffect(() => {
    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
  }, [handleMouseMove, handleMouseUp]);

  useEffect(() => {
    const handleTableHotkeys = (e: KeyboardEvent) => {
      const container = containerRef.current;
      if (!container) return;

      const activeElement = document.activeElement;
      if (!activeElement || !container.contains(activeElement)) return;

      if (isModKey(e) && e.key.toLowerCase() === "s") {
        if (hasPendingChanges && !isSaving) {
          e.preventDefault();
          void handleSave();
        }
        return;
      }

      if (e.key === "Escape") {
        if (editingCell) {
          e.preventDefault();
          cancelEdit();
          return;
        }

        if (hasPendingChanges && !isEditableTarget(e.target)) {
          e.preventDefault();
          handleDiscardChanges();
        }
      }
    };

    window.addEventListener("keydown", handleTableHotkeys);
    return () => {
      window.removeEventListener("keydown", handleTableHotkeys);
    };
  }, [
    hasPendingChanges,
    isSaving,
    handleSave,
    editingCell,
    cancelEdit,
    handleDiscardChanges,
  ]);

  return (
    <div ref={containerRef} className="h-full flex flex-col bg-background">
      {!hideHeader && (
        <div className="flex items-center justify-between px-4 py-2 border-b border-border">
          <div className="flex items-center gap-2">
            {tableContext && onFilterChange ? (
              <>
                <div className="relative">
                  <Filter className="absolute left-2 top-1/2 transform -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                  <Input
                    type="text"
                    placeholder="WHERE ..."
                    className="pl-8 h-8 w-64 font-mono text-xs"
                    value={whereInput}
                    onChange={(e) => setWhereInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        onFilterChange(whereInput, orderByInput);
                      }
                    }}
                  />
                </div>
                <div className="relative">
                  <ArrowUpDown className="absolute left-2 top-1/2 transform -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                  <Input
                    type="text"
                    placeholder="ORDER BY ..."
                    className="pl-8 h-8 w-48 font-mono text-xs"
                    value={orderByInput}
                    onChange={(e) => setOrderByInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        onFilterChange(whereInput, orderByInput);
                      }
                    }}
                  />
                </div>
              </>
            ) : null}
            {tableContext && (
              <Button
                variant="outline"
                size="sm"
                className="gap-2"
                onClick={handleShowDDL}
                title="View Table Structure (DDL)"
              >
                <FileCode className="w-4 h-4" />
              </Button>
            )}
            {hasPendingChanges && (
              <>
                <div className="w-px h-5 bg-border" />
                <Button
                  variant="default"
                  size="sm"
                  className="gap-1.5"
                  onClick={handleSave}
                  disabled={isSaving}
                  title="Save changes (Cmd/Ctrl+S)"
                >
                  {isSaving ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <Save className="w-4 h-4" />
                  )}
                  Save
                  <span className="bg-primary-foreground/20 text-primary-foreground text-xs px-1.5 py-0.5 rounded-full">
                    {pendingChanges.size}
                  </span>
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  className="gap-1.5"
                  onClick={handleDiscardChanges}
                  disabled={isSaving}
                  title="Discard changes (Esc)"
                >
                  <Undo2 className="w-4 h-4" />
                  Undo
                </Button>
              </>
            )}
            {tableContext && !isEditable && primaryKeys.length === 0 && (
              <span className="text-xs text-muted-foreground italic" title="This table has no primary key and does not support inline editing">
                Read-only
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">
              Showing {startIndex + 1}-{startIndex + currentData.length} of{" "}
              {total || sortedData.length} rows
            </span>
            <Button variant="outline" size="sm" className="gap-2">
              <Download className="w-4 h-4" />
            </Button>
          </div>
        </div>
      )}

      <div className="flex-1 overflow-auto">
        <table
          className="border-collapse table-fixed"
          style={{
            width: tableWidthPx,
          }}
        >
          <colgroup>
            <col className="w-12" style={{ width: INDEX_COL_WIDTH }} />
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
          <thead className="bg-muted/40 sticky top-0 z-10">
            <tr>
              <th className="px-4 py-2 text-left text-xs font-semibold text-muted-foreground border-b border-r border-border w-12">
                #
              </th>
              {columns.map((column) => {
                const isSorted = activeSortColumn === column;
                const direction = isSorted ? activeSortDirection : undefined;
                return (
                  <th
                    key={column}
                    ref={(el) => {
                      thRefs.current[column] = el;
                    }}
                    className="px-4 py-2 text-left text-xs font-semibold text-muted-foreground border-b border-r border-border relative group select-none"
                    style={{
                      width: getColWidth(column),
                      minWidth: 50,
                    }}
                  >
                    <div className="flex items-center justify-between pr-2">
                      <button
                        type="button"
                        className="flex items-center gap-1 cursor-pointer hover:text-foreground transition-colors min-w-0 flex-1"
                        onClick={() => handleSortClick(column)}
                      >
                        <span className="truncate">{column}</span>
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
            {currentData.map((row, rowIndex) => {
              const isEditing = (col: string) =>
                editingCell?.row === rowIndex && editingCell?.col === col;
              const isSelected = (col: string) =>
                selectedCell?.row === rowIndex && selectedCell?.col === col;

              return (
                <ContextMenu key={rowIndex}>
                  <ContextMenuTrigger asChild>
                    <tr className="hover:bg-muted/50 border-b border-border group">
                      <td className="px-4 py-2 text-xs text-muted-foreground border-r border-border">
                        {startIndex + rowIndex + 1}
                      </td>
                      {columns.map((column) => {
                        const modified = isCellModified(rowIndex, column);
                        const displayValue = getCellDisplayValue(rowIndex, column, row[column]);
                        const editing = isEditing(column);
                        const selected = isSelected(column);

                        return (
                          <td
                            key={column}
                            className={[
                              "px-0 py-0 text-sm text-foreground font-mono border-r border-border relative",
                              selected && !editing ? "bg-primary/10 ring-1 ring-inset ring-primary/50" : "",
                              modified && !editing ? "border-l-2 border-l-orange-400" : "",
                              isEditable ? "cursor-pointer" : "",
                            ]
                              .filter(Boolean)
                              .join(" ")}
                            style={{
                              width: getColWidth(column),
                              minWidth: 50,
                            }}
                            onClick={() => handleCellClick(rowIndex, column)}
                            onContextMenu={() => handleCellClick(rowIndex, column)}
                            onDoubleClick={() =>
                              handleCellDoubleClick(rowIndex, column, row[column])
                            }
                          >
                            {editing ? (
                              <input
                                ref={editInputRef}
                                type="text"
                                className="w-full h-full px-4 py-2 bg-background border-2 border-primary outline-none font-mono text-sm"
                                value={editValue}
                                onChange={(e) => setEditValue(e.target.value)}
                                onKeyDown={handleEditKeyDown}
                                onBlur={commitEdit}
                              />
                            ) : (
                              <div className="px-4 py-2 truncate">
                                {displayValue !== null && displayValue !== undefined ? (
                                  <span className={modified ? "text-orange-600 dark:text-orange-400" : ""}>
                                    {String(displayValue)}
                                  </span>
                                ) : (
                                  <span className="text-muted-foreground italic">NULL</span>
                                )}
                              </div>
                            )}
                          </td>
                        );
                      })}
                    </tr>
                  </ContextMenuTrigger>
                  <ContextMenuContent>
                    <ContextMenuItem
                      onClick={() => {
                        if (selectedCell && selectedCell.row === rowIndex) {
                          const val = getCellDisplayValue(
                            rowIndex,
                            selectedCell.col,
                            row[selectedCell.col]
                          );
                          handleCopy(val === null || val === undefined ? "" : String(val));
                        }
                      }}
                    >
                      <Copy className="w-4 h-4 mr-2" />
                      Copy
                    </ContextMenuItem>
                    <ContextMenuItem
                      onClick={() => {
                        const values = columns.map((col) => {
                          const val = getCellDisplayValue(rowIndex, col, row[col]);
                          return val === null || val === undefined ? "" : String(val);
                        });
                        handleCopy(values.join("\t"));
                      }}
                    >
                      <TableIcon className="w-4 h-4 mr-2" />
                      Copy Row
                    </ContextMenuItem>
                    <ContextMenuSeparator />
                    {isEditable && isCellModified(rowIndex, selectedCell?.col || "") && (
                      <>
                        <ContextMenuItem
                          onClick={() => {
                            if (selectedCell && selectedCell.row === rowIndex) {
                              const key = `${rowIndex}_${selectedCell.col}`;
                              setPendingChanges((prev) => {
                                const next = new Map(prev);
                                next.delete(key);
                                return next;
                              });
                            }
                          }}
                        >
                          <Undo2 className="w-4 h-4 mr-2" />
                          Undo This Cell
                        </ContextMenuItem>
                        <ContextMenuSeparator />
                      </>
                    )}
                    <ContextMenuSub>
                      <ContextMenuSubTrigger>
                        <Files className="w-4 h-4 mr-2" />
                        Copy as
                      </ContextMenuSubTrigger>
                      <ContextMenuSubContent>
                        <ContextMenuItem
                          onClick={() => {
                            const values = columns.map((col) => {
                              const val = getCellDisplayValue(rowIndex, col, row[col]);
                              if (val === null || val === undefined) return "";
                              const str = String(val);
                              if (str.includes(",") || str.includes('"') || str.includes("\n")) {
                                return `"${str.replace(/"/g, '""')}"`;
                              }
                              return str;
                            });
                            handleCopy(values.join(","));
                          }}
                        >
                          Copy as CSV
                        </ContextMenuItem>
                        <ContextMenuItem
                          onClick={() => {
                            if (!tableContext) return;
                            const { schema, table, driver } = tableContext;
                            const tableName =
                              driver === "mysql"
                                ? `${quoteIdent(table)}`
                                : `${quoteIdent(schema)}.${quoteIdent(table)}`;

                            const cols = columns.map((c) => quoteIdent(c)).join(", ");
                            const vals = columns
                              .map((col) => {
                                const val = getCellDisplayValue(rowIndex, col, row[col]);
                                return formatSQLValue(
                                  val === null || val === undefined ? "" : String(val),
                                  row[col],
                                  "copy"
                                );
                              })
                              .join(", ");
                            const sql = `INSERT INTO ${tableName} (${cols}) VALUES (${vals});`;
                            handleCopy(sql);
                          }}
                        >
                          Copy as Insert SQL
                        </ContextMenuItem>
                        <ContextMenuItem
                          onClick={() => {
                            if (!tableContext || primaryKeys.length === 0) return;
                            const { schema, table, driver } = tableContext;
                            const tableName =
                              driver === "mysql"
                                ? `${quoteIdent(table)}`
                                : `${quoteIdent(schema)}.${quoteIdent(table)}`;

                            const setClauses = columns.map((col) => {
                              const val = getCellDisplayValue(rowIndex, col, row[col]);
                              const formattedValue = formatSQLValue(
                                val === null || val === undefined ? "" : String(val),
                                row[col],
                                "copy"
                              );
                              return `${quoteIdent(col)} = ${formattedValue}`;
                            });

                            const whereClauses = primaryKeys.map((pk) => {
                              const pkValue = row[pk];
                              if (pkValue === null || pkValue === undefined) {
                                return `${quoteIdent(pk)} IS NULL`;
                              }
                              if (typeof pkValue === "number") {
                                return `${quoteIdent(pk)} = ${pkValue}`;
                              }
                              return `${quoteIdent(pk)} = '${escapeSQL(String(pkValue))}'`;
                            });

                            const sql = `UPDATE ${tableName} SET ${setClauses.join(
                              ", "
                            )} WHERE ${whereClauses.join(" AND ")};`;
                            handleCopy(sql);
                          }}
                        >
                          Copy as Update SQL
                        </ContextMenuItem>
                      </ContextMenuSubContent>
                    </ContextMenuSub>
                  </ContextMenuContent>
                </ContextMenu>
              );
            })}
          </tbody>
        </table>
      </div>

      {saveError && (
        <div className="px-4 py-2 border-t border-destructive/30 bg-destructive/10 text-destructive text-xs font-mono whitespace-pre-wrap">
          {saveError}
          <button
            className="ml-2 underline hover:no-underline"
            onClick={() => setSaveError(null)}
          >
            Close
          </button>
        </div>
      )}

      <div className="flex items-center justify-between px-4 py-1 border-t border-border bg-muted/40">
        <div className="text-sm text-muted-foreground">
          Query executed in{" "}
          {executionTimeMs ? (executionTimeMs / 1000).toFixed(3) : "0.000"}s •{" "}
          {sortedData.length} rows returned
          {hasPendingChanges && (
            <span className="text-orange-600 dark:text-orange-400 ml-2">
              • {pendingChanges.size} unsaved change(s)
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            className="h-7"
            onClick={handlePrevPage}
            disabled={page <= 1}
          >
            <ChevronLeft className="w-4 h-4" />
          </Button>
          <span className="text-sm text-muted-foreground">
            Page {page} of {totalPages || 1}
          </span>
          <Button
            variant="outline"
            size="sm"
            className="h-7"
            onClick={handleNextPage}
            disabled={page >= totalPages}
          >
            <ChevronRight className="w-4 h-4" />
          </Button>
        </div>
      </div>
    </div>
  );
}
