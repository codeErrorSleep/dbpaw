import { useState, useRef, useEffect, useCallback, useMemo } from "react";
import { save } from "@tauri-apps/plugin-dialog";
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
  RotateCw,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
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
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { api, isTauri } from "@/services/api";
import type { TransferFormat } from "@/services/api";
import { isEditableTarget, isModKey } from "@/lib/keyboard";
import { toast } from "sonner";

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
  onPageSizeChange?: (pageSize: number) => void;
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
  onDataRefresh?: (params?: {
    page?: number;
    limit?: number;
    filter?: string;
    orderBy?: string;
  }) => void;
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
  pageSize = 100,
  executionTimeMs = 0,
  onPageChange,
  onPageSizeChange,
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
  const PAGE_SIZE_OPTIONS = ["10", "50", "100", "200", "500", "1000"] as const;
  const [whereInput, setWhereInput] = useState(controlledFilter || "");
  const [orderByInput, setOrderByInput] = useState(controlledOrderBy || "");
  const [pageInput, setPageInput] = useState(String(page));
  const [pageSizeInput, setPageSizeInput] = useState(String(pageSize));
  const [columnWidths, setColumnWidths] = useState<Record<string, number>>({});

  // Reset column widths when columns definition changes (e.g. switching tables)
  const prevColumnsRef = useRef<string>("");
  useEffect(() => {
    const colsKey = columns.join(",");
    if (prevColumnsRef.current !== colsKey) {
      setColumnWidths({});
      prevColumnsRef.current = colsKey;
    }
  }, [columns]);

  // Auto-calculate column widths based on content
  useEffect(() => {
    if (!data.length || !columns.length) return;

    const newWidths: Record<string, number> = {};
    let hasChanges = false;

    // Configuration for auto-sizing
    const DATA_CHAR_WIDTH = 9; // Approximate width per character in px
    const DATA_PADDING = 36; // Padding for cell content
    const HEADER_CHAR_WIDTH = 9; // Approximate width per character in px
    const HEADER_PADDING = 56; // Header padding + sort icon + resize affordance
    // Dynamically adjust min width based on column count to fill space better for small tables
    const MIN_WIDTH = columns.length <= 3 ? 250 : 100;
    const MAX_WIDTH = 900;

    columns.forEach((col) => {
      // Only calculate if width is not already set (preserve manual resizes and previous calcs)
      if (columnWidths[col] !== undefined) return;

      let sampledMaxLen = 0;
      // Sample up to 20 rows to estimate width
      const sampleSize = Math.min(data.length, 20);

      for (let i = 0; i < sampleSize; i++) {
        const val = data[i][col];
        if (val !== null && val !== undefined) {
          const str = String(val);
          // Simple length check, capping at 100 chars
          const len = str.length > 100 ? 100 : str.length;
          if (len > sampledMaxLen) sampledMaxLen = len;
        }
      }

      const headerRequiredWidth = col.length * HEADER_CHAR_WIDTH + HEADER_PADDING;
      const sampledDataWidth = sampledMaxLen * DATA_CHAR_WIDTH + DATA_PADDING;
      const calculatedWidth = Math.min(
        MAX_WIDTH,
        Math.max(MIN_WIDTH, headerRequiredWidth, sampledDataWidth),
      );

      newWidths[col] = calculatedWidth;
      hasChanges = true;
    });

    if (hasChanges) {
      setColumnWidths((prev) => ({ ...prev, ...newWidths }));
    }
  }, [data, columns, columnWidths]);

  useEffect(() => {
    setWhereInput(controlledFilter || "");
  }, [controlledFilter]);

  useEffect(() => {
    setOrderByInput(controlledOrderBy || "");
  }, [controlledOrderBy]);

  useEffect(() => {
    setPageInput(String(page));
  }, [page]);

  useEffect(() => {
    const next = String(pageSize);
    setPageSizeInput(PAGE_SIZE_OPTIONS.includes(next as typeof PAGE_SIZE_OPTIONS[number]) ? next : "100");
  }, [pageSize]);

  // --- Cell selection & editing state ---
  const [selectedCell, setSelectedCell] = useState<{ row: number; col: string } | null>(null);
  const [editingCell, setEditingCell] = useState<{ row: number; col: string } | null>(null);
  const [editValue, setEditValue] = useState<string>("");
  const [pendingChanges, setPendingChanges] = useState<Map<string, PendingChange>>(new Map());
  const [primaryKeys, setPrimaryKeys] = useState<string[]>([]);
  const [isSaving, setIsSaving] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
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

  const handleExport = useCallback(
    async (
      scope: "current_page" | "filtered" | "full_table",
      format: TransferFormat,
    ) => {
      if (!tableContext) return;
      if (!isTauri()) {
        toast.error("Export dialog is only available in Tauri desktop mode.");
        return;
      }

      const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
      const defaultPath = `${tableContext.table}_${timestamp}.${format}`;
      const filters =
        format === "csv"
          ? [{ name: "CSV", extensions: ["csv"] }]
          : format === "json"
            ? [{ name: "JSON", extensions: ["json"] }]
            : [{ name: "SQL", extensions: ["sql"] }];

      let filePath: string | undefined;
      try {
        const selected = await save({
          title: "Save Export File",
          defaultPath,
          filters,
        });
        if (!selected) return;
        filePath = Array.isArray(selected) ? selected[0] : selected;
        if (!filePath) return;
      } catch (e) {
        toast.error("Failed to open save dialog", {
          description: e instanceof Error ? e.message : String(e),
        });
        return;
      }

      setIsExporting(true);
      try {
        const result = await api.transfer.exportTable({
          id: tableContext.connectionId,
          database: tableContext.database,
          schema: tableContext.schema,
          table: tableContext.table,
          driver: tableContext.driver,
          format,
          scope,
          filter: controlledFilter || undefined,
          orderBy: orderByInput || undefined,
          sortColumn: activeSortColumn,
          sortDirection: activeSortDirection,
          page,
          limit: pageSize,
          filePath,
        });
        toast.success(`Export completed (${result.rowCount} rows)`, {
          description: result.filePath,
        });
      } catch (e) {
        toast.error("Export failed", {
          description: e instanceof Error ? e.message : String(e),
        });
      } finally {
        setIsExporting(false);
      }
    },
    [
      tableContext,
      controlledFilter,
      orderByInput,
      activeSortColumn,
      activeSortDirection,
      page,
      pageSize,
    ],
  );

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

  const handleRefreshClick = useCallback(() => {
    if (hasPendingChanges) {
      const confirmed = window.confirm(
        "You have unsaved changes. Refreshing may discard your editing context. Continue?",
      );
      if (!confirmed) return;
    }

    const parsedPage = Number.parseInt(pageInput, 10);
    const nextPage = Number.isNaN(parsedPage) || parsedPage < 1 ? page : parsedPage;
    const parsedLimit = Number.parseInt(pageSizeInput, 10);
    const nextLimit =
      Number.isNaN(parsedLimit) || parsedLimit < 1 || parsedLimit > 10000
        ? pageSize
        : parsedLimit;
    const nextFilter = whereInput.trim() || undefined;
    const nextOrderBy = orderByInput.trim() || undefined;

    onDataRefresh?.({
      page: nextPage,
      limit: nextLimit,
      filter: nextFilter,
      orderBy: nextOrderBy,
    });
  }, [hasPendingChanges, pageInput, page, pageSizeInput, pageSize, whereInput, orderByInput, onDataRefresh]);

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

  const handlePageInputCommit = () => {
    const parsed = Number.parseInt(pageInput, 10);
    const maxPage = Math.max(totalPages, 1);
    const nextPage = Number.isNaN(parsed) ? page : Math.min(Math.max(parsed, 1), maxPage);
    setPageInput(String(nextPage));
    if (nextPage !== page) {
      onPageChange?.(nextPage);
    }
  };

  const handlePageSizeChange = (value: string) => {
    setPageSizeInput(value);
    const nextPageSize = Number.parseInt(value, 10);
    if (!Number.isNaN(nextPageSize) && nextPageSize !== pageSize) {
      onPageSizeChange?.(nextPageSize);
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
        <div className="flex flex-col gap-1.5 px-4 py-2 border-b border-border">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted-foreground">page</span>
              <Input
                type="text"
                inputMode="numeric"
                className="h-7 w-8 px-2 text-xs"
                value={pageInput}
                onChange={(e) => setPageInput(e.target.value.replace(/\D/g, ""))}
                onBlur={handlePageInputCommit}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    handlePageInputCommit();
                  }
                }}
              />
              <Button
                variant="outline"
                size="sm"
                className="h-7"
                onClick={handlePrevPage}
                disabled={page <= 1}
              >
                <ChevronLeft className="w-4 h-4" />
              </Button>
              <Button
                variant="outline"
                size="sm"
                className="h-7"
                onClick={handleNextPage}
                disabled={page >= totalPages}
              >
                <ChevronRight className="w-4 h-4" />
              </Button>
              <span className="text-xs text-muted-foreground">limit</span>
              <Select value={pageSizeInput} onValueChange={handlePageSizeChange}>
                <SelectTrigger className="!h-7 w-20 text-xs [&_svg]:size-3">
                  <SelectValue placeholder="100" />
                </SelectTrigger>
                <SelectContent>
                  {PAGE_SIZE_OPTIONS.map((size) => (
                    <SelectItem key={size} value={size}>
                      {size}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {tableContext && (
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 gap-2"
                  onClick={handleRefreshClick}
                  title="Refresh"
                >
                  <RotateCw className="w-4 h-4" />
                </Button>
              )}
            </div>

            <div className="flex items-center gap-2">
              {hasPendingChanges && (
                <>
                  <Button
                    variant="default"
                    size="sm"
                    className="h-7 gap-1.5"
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
                    className="h-7 gap-1.5"
                    onClick={handleDiscardChanges}
                    disabled={isSaving}
                    title="Discard changes (Esc)"
                  >
                    <Undo2 className="w-4 h-4" />
                    Undo
                  </Button>
                </>
              )}

              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 gap-2"
                    disabled={!tableContext || isExporting}
                  >
                    <Download className="w-4 h-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuSub>
                    <DropdownMenuSubTrigger>Export Current Page</DropdownMenuSubTrigger>
                    <DropdownMenuSubContent>
                      <DropdownMenuItem
                        onClick={() => void handleExport("current_page", "csv")}
                      >
                        CSV
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        onClick={() => void handleExport("current_page", "json")}
                      >
                        JSON
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        onClick={() => void handleExport("current_page", "sql")}
                      >
                        SQL
                      </DropdownMenuItem>
                    </DropdownMenuSubContent>
                  </DropdownMenuSub>
                  <DropdownMenuSub>
                    <DropdownMenuSubTrigger>Export Filtered Result</DropdownMenuSubTrigger>
                    <DropdownMenuSubContent>
                      <DropdownMenuItem
                        onClick={() => void handleExport("filtered", "csv")}
                      >
                        CSV
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        onClick={() => void handleExport("filtered", "json")}
                      >
                        JSON
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        onClick={() => void handleExport("filtered", "sql")}
                      >
                        SQL
                      </DropdownMenuItem>
                    </DropdownMenuSubContent>
                  </DropdownMenuSub>
                  <DropdownMenuSub>
                    <DropdownMenuSubTrigger>Export Full Table</DropdownMenuSubTrigger>
                    <DropdownMenuSubContent>
                      <DropdownMenuItem
                        onClick={() => void handleExport("full_table", "csv")}
                      >
                        CSV
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        onClick={() => void handleExport("full_table", "json")}
                      >
                        JSON
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        onClick={() => void handleExport("full_table", "sql")}
                      >
                        SQL
                      </DropdownMenuItem>
                    </DropdownMenuSubContent>
                  </DropdownMenuSub>
                </DropdownMenuContent>
              </DropdownMenu>

              {tableContext && (
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 gap-2"
                  onClick={handleShowDDL}
                  title="View Table Structure (DDL)"
                >
                  <FileCode className="w-4 h-4" />
                </Button>
              )}
            </div>
          </div>

          {tableContext && onFilterChange ? (
            <div className="pt-1 border-t border-border/40 flex items-center gap-2">
              <div className="relative flex-1 min-w-0">
                <Filter className="absolute left-2 top-1/2 transform -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                <Input
                  type="text"
                  placeholder="WHERE ..."
                  className="pl-8 h-7 w-full font-mono text-xs"
                  value={whereInput}
                  onChange={(e) => setWhereInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      onFilterChange(whereInput, orderByInput);
                    }
                  }}
                />
              </div>
              <div className="relative flex-1 min-w-0">
                <ArrowUpDown className="absolute left-2 top-1/2 transform -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                <Input
                  type="text"
                  placeholder="ORDER BY ..."
                  className="pl-8 h-7 w-full font-mono text-xs"
                  value={orderByInput}
                  onChange={(e) => setOrderByInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      onFilterChange(whereInput, orderByInput);
                    }
                  }}
                />
              </div>
              {tableContext && !isEditable && primaryKeys.length === 0 && (
                <span className="text-xs text-muted-foreground italic" title="This table has no primary key and does not support inline editing">
                  Read-only
                </span>
              )}
            </div>
          ) : (
            tableContext &&
            !isEditable &&
            primaryKeys.length === 0 && (
              <span className="text-xs text-muted-foreground italic" title="This table has no primary key and does not support inline editing">
                Read-only
              </span>
            )
          )}
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
                        <span className="truncate" title={column}>
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
                                autoCapitalize="off"
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

      <div className="flex items-center px-4 py-1 border-t border-border bg-muted/40">
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
      </div>
    </div>
  );
}
