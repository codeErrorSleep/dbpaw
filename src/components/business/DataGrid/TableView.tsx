import { useState, useRef, useEffect, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
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
  Search,
  Plus,
  SquareTerminal,
  Trash2,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
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
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { api, isTauri } from "@/services/api";
import type { ColumnInfo, TransferFormat } from "@/services/api";
import { isEditableTarget, isModKey } from "@/lib/keyboard";
import {
  buildDeleteStatement,
  buildUpdateStatement,
  calculateAutoColumnWidths,
  canMutateClickHouseTable,
  collectSearchMatches,
  escapeSQL,
  cellValueToString,
  formatCellValue,
  formatInsertSQLValue,
  formatSQLValue,
  getQualifiedTableName,
  isClickHouseMergeTreeEngine,
  isComplexValue,
  isInsertColumnRequired,
  quoteIdent,
  sortRows,
} from "./tableView/utils";
import { ComplexValueViewer } from "./ComplexValueViewer";
import { ColumnAutocompleteInput } from "./tableView/ColumnAutocompleteInput";
import type { ColumnAutocompleteOption } from "./tableView/columnAutocomplete";
import { toast } from "sonner";

interface PendingChange {
  rowIndex: number;
  sourceRowIndex: number;
  column: string;
  originalValue: any;
  newValue: string;
}

interface InsertDraftRow {
  tempId: string;
  values: Record<string, string>;
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
  }) => void | Promise<unknown>;
  onCreateQuery?: (
    connectionId: number,
    database: string,
    driver: string,
  ) => void;
  tableContext?: {
    connectionId: number;
    database: string;
    schema: string;
    table: string;
    driver: string;
  };
  isLoading?: boolean;
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
  onCreateQuery,
  tableContext,
  isLoading,
}: TableViewProps) {
  const { t } = useTranslation();
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
    const newWidths = calculateAutoColumnWidths({
      data,
      columns,
      columnWidths,
    });
    const hasChanges = Object.keys(newWidths).length > 0;

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
    setPageSizeInput(
      PAGE_SIZE_OPTIONS.includes(next as (typeof PAGE_SIZE_OPTIONS)[number])
        ? next
        : "100",
    );
  }, [pageSize]);

  // --- Cell selection & editing state ---
  const [selectedCell, setSelectedCell] = useState<{
    row: number;
    col: string;
  } | null>(null);
  const [selectedRows, setSelectedRows] = useState<Set<number>>(new Set());
  const [rowSelectionAnchor, setRowSelectionAnchor] = useState<number | null>(
    null,
  );
  const [isRowSelecting, setIsRowSelecting] = useState(false);
  const [editingCell, setEditingCell] = useState<{
    row: number;
    col: string;
  } | null>(null);
  const [editValue, setEditValue] = useState<string>("");
  const [pendingChanges, setPendingChanges] = useState<
    Map<string, PendingChange>
  >(new Map());
  const [insertDraftRows, setInsertDraftRows] = useState<InsertDraftRow[]>([]);
  const [primaryKeys, setPrimaryKeys] = useState<string[]>([]);
  const [clickhouseEngine, setClickhouseEngine] = useState<string | null>(null);
  const [tableColumns, setTableColumns] = useState<ColumnInfo[]>([]);
  const [columnComments, setColumnComments] = useState<Record<string, string>>(
    {},
  );
  const columnAutocompleteOptions = useMemo<ColumnAutocompleteOption[]>(() => {
    if (tableColumns.length > 0) {
      return tableColumns.map((column) => ({
        name: column.name,
        type: column.type,
      }));
    }

    return columns.map((column) => ({ name: column }));
  }, [columns, tableColumns]);
  const [isSaving, setIsSaving] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [lastRefreshedAt, setLastRefreshedAt] = useState<Date | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [searchKeyword, setSearchKeyword] = useState("");
  const [searchCursorIndex, setSearchCursorIndex] = useState(-1);
  const [pendingFocusDraftId, setPendingFocusDraftId] = useState<string | null>(
    null,
  );
  const [complexViewer, setComplexViewer] = useState<{
    value: unknown;
    columnName: string;
  } | null>(null);
  const editInputRef = useRef<HTMLInputElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const saveButtonRef = useRef<HTMLButtonElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const selectedCellRef = useRef<{ row: number; col: string } | null>(null);
  const selectedRowsRef = useRef<Set<number>>(new Set());

  useEffect(() => {
    selectedCellRef.current = selectedCell;
  }, [selectedCell]);

  useEffect(() => {
    selectedRowsRef.current = selectedRows;
  }, [selectedRows]);

  // Sort state: controlled (via props) or uncontrolled (internal state for client-side sorting)
  const [internalSortColumn, setInternalSortColumn] = useState<
    string | undefined
  >();
  const [internalSortDirection, setInternalSortDirection] = useState<
    "asc" | "desc" | undefined
  >();

  const isControlledSort = !!onSortChange;
  const activeSortColumn = isControlledSort
    ? controlledSortColumn
    : internalSortColumn;
  const activeSortDirection = isControlledSort
    ? controlledSortDirection
    : internalSortDirection;
  const hasLocalClientSort =
    !isControlledSort && !!activeSortColumn && !!activeSortDirection;

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
      setClickhouseEngine(null);
      setTableColumns([]);
      setColumnComments({});
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
        const pks = meta.columns.filter((c) => c.primaryKey).map((c) => c.name);
        setPrimaryKeys(pks);
        setClickhouseEngine(meta.clickhouseExtra?.engine || null);
        setTableColumns(meta.columns);

        const comments: Record<string, string> = {};
        meta.columns.forEach((c) => {
          const comment = c.comment?.trim();
          if (comment) {
            comments[c.name] = comment;
          }
        });
        setColumnComments(comments);
      })
      .catch((e) => {
        console.error("Failed to fetch primary keys:", e);
        setPrimaryKeys([]);
        setClickhouseEngine(null);
        setTableColumns([]);
        setColumnComments({});
      });
  }, [
    tableContext?.connectionId,
    tableContext?.database,
    tableContext?.schema,
    tableContext?.table,
  ]);

  // Clear pending changes when data/page changes
  useEffect(() => {
    setPendingChanges(new Map());
    setInsertDraftRows([]);
    setEditingCell(null);
    selectedCellRef.current = null;
    setSelectedCell(null);
    const nextSelectedRows = new Set<number>();
    selectedRowsRef.current = nextSelectedRows;
    setSelectedRows(nextSelectedRows);
    setRowSelectionAnchor(null);
    setIsRowSelecting(false);
    setDeleteDialogOpen(false);
    setIsDeleting(false);
    setSaveError(null);
  }, [data, page]);

  const isClickHouseDriver = tableContext?.driver === "clickhouse";
  const hasPrimaryKeys = primaryKeys.length > 0;
  const canInsert =
    !!tableContext &&
    (isClickHouseDriver
      ? isClickHouseMergeTreeEngine(clickhouseEngine)
      : hasPrimaryKeys);
  const canUpdateDelete =
    !!tableContext &&
    (isClickHouseDriver
      ? canMutateClickHouseTable(clickhouseEngine, primaryKeys)
      : hasPrimaryKeys);
  const isEditableForUpdates = canUpdateDelete && !hasLocalClientSort;
  const mutabilityHint = useMemo(() => {
    if (!tableContext) return null;
    if (hasLocalClientSort) {
      return "Inline cell editing is disabled while client-side sorting is active.";
    }
    if (isClickHouseDriver) {
      if (!isClickHouseMergeTreeEngine(clickhouseEngine)) {
        return "ClickHouse inline write is only enabled for MergeTree-family tables.";
      }
      if (!hasPrimaryKeys) {
        return "ClickHouse table update/delete requires primary key columns.";
      }
      return null;
    }
    if (!hasPrimaryKeys) {
      return "This table has no primary key and does not support inline editing";
    }
    return null;
  }, [
    tableContext,
    hasLocalClientSort,
    isClickHouseDriver,
    clickhouseEngine,
    hasPrimaryKeys,
  ]);
  const pendingMutationCount = pendingChanges.size + insertDraftRows.length;
  const hasPendingChanges = pendingMutationCount > 0;

  // Client-side sorting (used in uncontrolled mode, e.g. SQL query results)
  const sortedData = useMemo(() => {
    if (isControlledSort || !activeSortColumn || !activeSortDirection) {
      return data;
    }
    return sortRows(data, activeSortColumn, activeSortDirection);
  }, [data, isControlledSort, activeSortColumn, activeSortDirection]);

  // If external pagination is used (onPageChange provided), we assume data is already the current page
  // Otherwise we slice locally
  const currentData = useMemo(
    () =>
      onPageChange
        ? sortedData
        : sortedData.slice((page - 1) * pageSize, page * pageSize),
    [onPageChange, page, pageSize, sortedData],
  );

  // If using external pagination, totalPages is based on total count
  // Otherwise fallback to filtered data length
  const totalPages = Math.ceil((total || sortedData.length) / pageSize);

  // --- Cell interaction handlers ---
  const handleCellClick = useCallback(
    (rowIndex: number, col: string) => {
      // If clicking a different cell while editing, commit current edit first
      if (
        editingCell &&
        (editingCell.row !== rowIndex || editingCell.col !== col)
      ) {
        commitEdit();
      }
      const nextSelectedRows = new Set<number>();
      selectedRowsRef.current = nextSelectedRows;
      setSelectedRows(nextSelectedRows);
      setRowSelectionAnchor(null);
      setIsRowSelecting(false);
      const nextSelectedCell = { row: rowIndex, col };
      selectedCellRef.current = nextSelectedCell;
      setSelectedCell(nextSelectedCell);
    },
    [editingCell],
  );

  const handleCellDoubleClick = useCallback(
    (rowIndex: number, col: string, currentValue: any) => {
      if (!isEditableForUpdates) return;
      // Check if there's a pending change for this cell
      const key = `${rowIndex}_${col}`;
      const pending = pendingChanges.get(key);
      const value = pending ? pending.newValue : cellValueToString(currentValue);
      setEditingCell({ row: rowIndex, col });
      setEditValue(value);
      setSelectedCell({ row: rowIndex, col });
      // Focus input on next tick
      setTimeout(() => editInputRef.current?.focus(), 0);
    },
    [isEditableForUpdates, pendingChanges],
  );

  const commitEdit = useCallback(() => {
    if (!editingCell) return;
    const { row, col } = editingCell;
    const originalRow = currentData[row];
    if (!originalRow) {
      setEditingCell(null);
      return;
    }
    const sourceRowIndex = data.indexOf(originalRow);
    const originalValue = originalRow[col];
    const originalStr = cellValueToString(originalValue);
    const key = `${row}_${col}`;

    if (editValue !== originalStr) {
      setPendingChanges((prev) => {
        const next = new Map(prev);
        next.set(key, {
          rowIndex: row,
          sourceRowIndex: sourceRowIndex >= 0 ? sourceRowIndex : row,
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
  }, [editingCell, editValue, data, currentData]);

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
    setInsertDraftRows([]);
    setEditingCell(null);
    setSaveError(null);
  }, []);

  const handleCopy = useCallback((text: string) => {
    void navigator.clipboard.writeText(text).catch((error) => {
      toast.error("Failed to copy", {
        description: error instanceof Error ? error.message : String(error),
      });
    });
  }, []);

  const selectSingleRow = useCallback((rowIndex: number) => {
    const nextSelectedRows = new Set([rowIndex]);
    selectedRowsRef.current = nextSelectedRows;
    setSelectedRows(nextSelectedRows);
  }, []);

  const selectRowRange = useCallback((anchor: number, current: number) => {
    const start = Math.min(anchor, current);
    const end = Math.max(anchor, current);
    const next = new Set<number>();
    for (let i = start; i <= end; i++) {
      next.add(i);
    }
    selectedRowsRef.current = next;
    setSelectedRows(next);
  }, []);

  const handleIndexMouseDown = useCallback(
    (e: React.MouseEvent, rowIndex: number) => {
      if (e.button !== 0) return;
      e.preventDefault();
      selectedCellRef.current = null;
      setSelectedCell(null);
      setIsRowSelecting(true);
      setRowSelectionAnchor(rowIndex);
      selectSingleRow(rowIndex);
    },
    [selectSingleRow],
  );

  const handleIndexMouseEnter = useCallback(
    (rowIndex: number) => {
      if (!isRowSelecting || rowSelectionAnchor === null) return;
      selectRowRange(rowSelectionAnchor, rowIndex);
    },
    [isRowSelecting, rowSelectionAnchor, selectRowRange],
  );

  // --- SQL generation & save ---

  const generateUpdateSQL = useCallback(() => {
    if (!tableContext || !canUpdateDelete || primaryKeys.length === 0)
      return [];

    // Group changes by source row index
    const changesByRow = new Map<number, PendingChange[]>();
    pendingChanges.forEach((change) => {
      const existing = changesByRow.get(change.sourceRowIndex) || [];
      existing.push(change);
      changesByRow.set(change.sourceRowIndex, existing);
    });

    const sqls: string[] = [];
    const { schema, table, driver } = tableContext;

    changesByRow.forEach((changes, rowIndex) => {
      const row = data[rowIndex] ?? currentData[changes[0]?.rowIndex ?? -1];
      if (!row) return;

      // Build SET clause - only modified columns
      const setClauses = changes.map((c) => {
        const formattedValue = formatSQLValue(
          c.newValue,
          c.originalValue,
          "execution",
          tableContext.driver,
        );
        return `${quoteIdent(tableContext.driver, c.column)} = ${formattedValue}`;
      });

      // Build WHERE clause using primary keys
      const whereClauses = primaryKeys.map((pk) => {
        const pkValue = row[pk];
        if (pkValue === null || pkValue === undefined) {
          return `${quoteIdent(tableContext.driver, pk)} IS NULL`;
        }
        if (typeof pkValue === "number") {
          return `${quoteIdent(tableContext.driver, pk)} = ${pkValue}`;
        }
        return `${quoteIdent(tableContext.driver, pk)} = '${escapeSQL(String(pkValue))}'`;
      });

      const tableName = getQualifiedTableName(driver, schema, table);

      const sql = buildUpdateStatement(
        driver,
        tableName,
        setClauses.join(", "),
        whereClauses.join(" AND "),
      );
      sqls.push(sql);
    });

    return sqls;
  }, [
    tableContext,
    canUpdateDelete,
    primaryKeys,
    pendingChanges,
    data,
    currentData,
  ]);

  const generateInsertSQL = useCallback(() => {
    if (!tableContext || !canInsert || !insertDraftRows.length) return [];
    const tableName = getQualifiedTableName(
      tableContext.driver,
      tableContext.schema,
      tableContext.table,
    );
    const metadataByName = new Map(tableColumns.map((col) => [col.name, col]));
    const sqls: string[] = [];

    insertDraftRows.forEach((draft, index) => {
      const insertColumns: string[] = [];
      const insertValues: string[] = [];

      columns.forEach((columnName) => {
        const raw = draft.values[columnName] ?? "";
        const trimmed = raw.trim();
        const meta = metadataByName.get(columnName);

        if (trimmed === "") {
          if (meta && isInsertColumnRequired(meta)) {
            throw new Error(
              `Row ${index + 1}: column "${columnName}" is required`,
            );
          }
          return;
        }

        const formatted = formatInsertSQLValue(
          raw,
          { name: columnName, type: meta?.type || "text" },
          tableContext.driver,
        );
        insertColumns.push(quoteIdent(tableContext.driver, columnName));
        insertValues.push(formatted);
      });

      if (!insertColumns.length) {
        throw new Error(
          `Row ${index + 1}: at least one column value is required`,
        );
      }

      sqls.push(
        `INSERT INTO ${tableName} (${insertColumns.join(", ")}) VALUES (${insertValues.join(", ")})`,
      );
    });

    return sqls;
  }, [tableContext, canInsert, insertDraftRows, tableColumns, columns]);

  const buildDeleteSQL = useCallback(() => {
    if (
      !tableContext ||
      !canUpdateDelete ||
      !selectedRows.size ||
      primaryKeys.length === 0
    ) {
      return "";
    }

    const selectedIndexes = Array.from(selectedRows).sort((a, b) => a - b);
    const rowClauses = selectedIndexes
      .map((rowIndex) => {
        const row = currentData[rowIndex];
        if (!row) return "";
        const pkClauses = primaryKeys.map((pk) => {
          const pkValue = row[pk];
          if (pkValue === null || pkValue === undefined) {
            return `${quoteIdent(tableContext.driver, pk)} IS NULL`;
          }
          if (typeof pkValue === "number") {
            return `${quoteIdent(tableContext.driver, pk)} = ${pkValue}`;
          }
          return `${quoteIdent(tableContext.driver, pk)} = '${escapeSQL(String(pkValue))}'`;
        });
        return `(${pkClauses.join(" AND ")})`;
      })
      .filter((clause) => clause.length > 0);

    if (!rowClauses.length) return "";

    const tableName = getQualifiedTableName(
      tableContext.driver,
      tableContext.schema,
      tableContext.table,
    );
    return buildDeleteStatement(
      tableContext.driver,
      tableName,
      rowClauses.join(" OR "),
    );
  }, [tableContext, canUpdateDelete, selectedRows, primaryKeys, currentData]);

  const handleAddDraftRow = useCallback(() => {
    if (!canInsert) return;
    const tempId = `draft_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    const values = columns.reduce<Record<string, string>>((acc, column) => {
      acc[column] = "";
      return acc;
    }, {});
    setInsertDraftRows((prev) => [...prev, { tempId, values }]);
    setPendingFocusDraftId(tempId);
  }, [canInsert, columns]);

  const handleDraftValueChange = useCallback(
    (tempId: string, column: string, value: string) => {
      setInsertDraftRows((prev) =>
        prev.map((draft) =>
          draft.tempId === tempId
            ? { ...draft, values: { ...draft.values, [column]: value } }
            : draft,
        ),
      );
    },
    [],
  );

  const refreshAfterMutation = useCallback(async () => {
    if (!onDataRefresh) return;
    const runRefresh = async () => {
      const ret = onDataRefresh();
      if (ret && typeof (ret as Promise<unknown>).then === "function") {
        await ret;
      }
    };

    await runRefresh();
    if (tableContext?.driver === "clickhouse") {
      await new Promise((resolve) => setTimeout(resolve, 350));
      await runRefresh();
    }
  }, [onDataRefresh, tableContext?.driver]);

  const handleConfirmDelete = useCallback(async () => {
    if (!tableContext || !canUpdateDelete || !selectedRows.size || isDeleting) {
      return;
    }

    const sql = buildDeleteSQL();
    if (!sql) {
      setDeleteDialogOpen(false);
      return;
    }

    setIsDeleting(true);
    setSaveError(null);
    try {
      await api.query.execute(
        tableContext.connectionId,
        sql,
        tableContext.database,
        "table_view_save",
      );
      setDeleteDialogOpen(false);
      const nextSelectedRows = new Set<number>();
      selectedRowsRef.current = nextSelectedRows;
      setSelectedRows(nextSelectedRows);
      selectedCellRef.current = null;
      setSelectedCell(null);
      setEditingCell(null);
      await refreshAfterMutation();
    } catch (e) {
      setSaveError(
        `Delete failed:\n${sql}\n  -> ${e instanceof Error ? e.message : String(e)}`,
      );
    } finally {
      setIsDeleting(false);
    }
  }, [
    tableContext,
    canUpdateDelete,
    selectedRows,
    isDeleting,
    buildDeleteSQL,
    refreshAfterMutation,
  ]);

  const handleSave = useCallback(async () => {
    if (!tableContext || !hasPendingChanges) return;

    setIsSaving(true);
    setSaveError(null);

    let sqls: string[] = [];
    try {
      const updateSqls = generateUpdateSQL();
      const insertSqls = generateInsertSQL();
      sqls = [...updateSqls, ...insertSqls];
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
          "table_view_save",
        );
      } catch (e) {
        errors.push(
          `${sql}\n  -> ${e instanceof Error ? e.message : String(e)}`,
        );
      }
    }

    setIsSaving(false);

    if (errors.length > 0) {
      setSaveError(
        `${errors.length} statement(s) failed:\n${errors.join("\n")}`,
      );
    } else {
      setPendingChanges(new Map());
      setInsertDraftRows([]);
      setSaveError(null);
      await refreshAfterMutation();
    }
  }, [
    tableContext,
    hasPendingChanges,
    generateUpdateSQL,
    generateInsertSQL,
    refreshAfterMutation,
  ]);

  const handleRefreshClick = useCallback(async () => {
    if (isRefreshing) return;
    if (hasPendingChanges) {
      const confirmed = window.confirm(
        "You have unsaved changes. Refreshing may discard your editing context. Continue?",
      );
      if (!confirmed) return;
    }

    const parsedPage = Number.parseInt(pageInput, 10);
    const nextPage =
      Number.isNaN(parsedPage) || parsedPage < 1 ? page : parsedPage;
    const parsedLimit = Number.parseInt(pageSizeInput, 10);
    const nextLimit =
      Number.isNaN(parsedLimit) || parsedLimit < 1 || parsedLimit > 10000
        ? pageSize
        : parsedLimit;
    const nextFilter = whereInput.trim() || undefined;
    const nextOrderBy = orderByInput.trim() || undefined;

    if (!onDataRefresh) return;

    setIsRefreshing(true);
    try {
      const ret = onDataRefresh({
        page: nextPage,
        limit: nextLimit,
        filter: nextFilter,
        orderBy: nextOrderBy,
      });
      if (ret && typeof (ret as Promise<unknown>).then === "function") {
        await ret;
      } else {
        await new Promise((r) => setTimeout(r, 300));
      }
      setLastRefreshedAt(new Date());
    } catch (e) {
      void e;
    } finally {
      setIsRefreshing(false);
    }
  }, [
    hasPendingChanges,
    pageInput,
    page,
    pageSizeInput,
    pageSize,
    whereInput,
    orderByInput,
    onDataRefresh,
    isRefreshing,
  ]);

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
    INDEX_COL_WIDTH + columns.reduce((sum, c) => sum + getColWidth(c), 0);

  const buildRowsTSV = useCallback(
    (rowIndexes: number[]) => {
      const orderedRows = [...rowIndexes].sort((a, b) => a - b);
      return orderedRows
        .map((rowIndex) => {
          const row = currentData[rowIndex];
          if (!row) return "";
          return columns
            .map((col) => {
              const value = getCellDisplayValue(rowIndex, col, row[col]);
              if (value === null || value === undefined) return "";
              return cellValueToString(value);
            })
            .join("\t");
        })
        .filter((line) => line.length > 0)
        .join("\n");
    },
    [columns, currentData, getCellDisplayValue],
  );

  const getSelectedCellCopyText = useCallback(() => {
    const selectedCell = selectedCellRef.current;
    if (!selectedCell) return null;
    const row = currentData[selectedCell.row];
    if (!row) return null;
    const value = getCellDisplayValue(
      selectedCell.row,
      selectedCell.col,
      row[selectedCell.col],
    );
    if (value === null || value === undefined) return "";
    return cellValueToString(value);
  }, [currentData, getCellDisplayValue]);

  const buildRowsCSV = useCallback(
    (rowIndexes: number[]) => {
      const orderedRows = [...rowIndexes].sort((a, b) => a - b);
      return orderedRows
        .map((rowIndex) => {
          const row = currentData[rowIndex];
          if (!row) return "";
          return columns
            .map((col) => {
              const value = getCellDisplayValue(rowIndex, col, row[col]);
              if (value === null || value === undefined) return "";
              const str = cellValueToString(value);
              if (
                str.includes(",") ||
                str.includes('"') ||
                str.includes("\n")
              ) {
                return `"${str.replace(/"/g, '""')}"`;
              }
              return str;
            })
            .join(",");
        })
        .filter((line) => line.length > 0)
        .join("\n");
    },
    [columns, currentData, getCellDisplayValue],
  );

  const buildRowsInsertSQL = useCallback(
    (rowIndexes: number[]) => {
      if (!tableContext) return "";
      const orderedRows = [...rowIndexes].sort((a, b) => a - b);
      const { schema, table, driver } = tableContext;
      const tableName = getQualifiedTableName(driver, schema, table);
      const cols = columns.map((c) => quoteIdent(driver, c)).join(", ");

      return orderedRows
        .map((rowIndex) => {
          const row = currentData[rowIndex];
          if (!row) return "";
          const vals = columns
            .map((col) => {
              const val = getCellDisplayValue(rowIndex, col, row[col]);
              return formatSQLValue(
                val === null || val === undefined ? "" : String(val),
                row[col],
                "copy",
                driver,
              );
            })
            .join(", ");
          return `INSERT INTO ${tableName} (${cols}) VALUES (${vals});`;
        })
        .filter((line) => line.length > 0)
        .join("\n");
    },
    [columns, currentData, getCellDisplayValue, tableContext],
  );

  const buildRowsUpdateSQL = useCallback(
    (rowIndexes: number[]) => {
      if (!tableContext || !canUpdateDelete || primaryKeys.length === 0)
        return "";
      const orderedRows = [...rowIndexes].sort((a, b) => a - b);
      const { schema, table, driver } = tableContext;
      const tableName = getQualifiedTableName(driver, schema, table);

      return orderedRows
        .map((rowIndex) => {
          const row = currentData[rowIndex];
          if (!row) return "";

          const setClauses = columns.map((col) => {
            const val = getCellDisplayValue(rowIndex, col, row[col]);
            const formattedValue = formatSQLValue(
              val === null || val === undefined ? "" : String(val),
              row[col],
              "copy",
              driver,
            );
            return `${quoteIdent(driver, col)} = ${formattedValue}`;
          });

          const whereClauses = primaryKeys.map((pk) => {
            const pkValue = row[pk];
            if (pkValue === null || pkValue === undefined) {
              return `${quoteIdent(driver, pk)} IS NULL`;
            }
            if (typeof pkValue === "number") {
              return `${quoteIdent(driver, pk)} = ${pkValue}`;
            }
            return `${quoteIdent(driver, pk)} = '${escapeSQL(String(pkValue))}'`;
          });

          return `${buildUpdateStatement(driver, tableName, setClauses.join(", "), whereClauses.join(" AND "))};`;
        })
        .filter((line) => line.length > 0)
        .join("\n");
    },
    [
      columns,
      currentData,
      getCellDisplayValue,
      canUpdateDelete,
      primaryKeys,
      tableContext,
    ],
  );

  const normalizedSearchKeyword = searchKeyword.trim().toLowerCase();

  const searchMatches = useMemo(() => {
    return collectSearchMatches(
      currentData,
      columns,
      normalizedSearchKeyword,
      getCellDisplayValue,
    );
  }, [normalizedSearchKeyword, currentData, columns, getCellDisplayValue]);

  const matchedRows = useMemo(() => {
    const rows = new Set<number>();
    searchMatches.forEach((match) => {
      rows.add(match.row);
    });
    return rows;
  }, [searchMatches]);

  const matchedCellKeys = useMemo(() => {
    const keys = new Set<string>();
    searchMatches.forEach((match) => {
      keys.add(`${match.row}::${match.col}`);
    });
    return keys;
  }, [searchMatches]);

  const currentSearchMatch =
    searchCursorIndex >= 0 && searchCursorIndex < searchMatches.length
      ? searchMatches[searchCursorIndex]
      : null;

  // Correctly calculate start index for display
  const startIndex = (page - 1) * pageSize;

  const focusSearchInput = useCallback(() => {
    setTimeout(() => searchInputRef.current?.focus(), 0);
  }, []);

  const jumpToSearchMatch = useCallback(
    (matchIndex: number) => {
      if (!searchMatches.length) return;
      const safeIndex =
        ((matchIndex % searchMatches.length) + searchMatches.length) %
        searchMatches.length;
      const nextMatch = searchMatches[safeIndex];

      if (editingCell) {
        commitEdit();
      }

      setSelectedCell({ row: nextMatch.row, col: nextMatch.col });
      setSearchCursorIndex(safeIndex);

      requestAnimationFrame(() => {
        const row = nextMatch.row;
        const colIndex = nextMatch.colIndex;
        const target = containerRef.current?.querySelector<HTMLElement>(
          `td[data-row-index="${row}"][data-col-index="${colIndex}"]`,
        );
        target?.scrollIntoView({
          behavior: "smooth",
          block: "nearest",
          inline: "nearest",
        });
      });
    },
    [searchMatches, editingCell, commitEdit],
  );

  const handleSearchEnter = useCallback(() => {
    if (!searchMatches.length) return;
    const nextIndex = searchCursorIndex < 0 ? 0 : searchCursorIndex + 1;
    jumpToSearchMatch(nextIndex);
  }, [searchMatches, searchCursorIndex, jumpToSearchMatch]);

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
    const nextPage = Number.isNaN(parsed)
      ? page
      : Math.min(Math.max(parsed, 1), maxPage);
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
    setSearchCursorIndex(-1);
  }, [normalizedSearchKeyword]);

  useEffect(() => {
    if (!searchMatches.length) {
      setSearchCursorIndex(-1);
      return;
    }
    if (searchCursorIndex >= searchMatches.length) {
      setSearchCursorIndex(0);
    }
  }, [searchMatches, searchCursorIndex]);

  useEffect(() => {
    if (isSearchOpen) {
      focusSearchInput();
    }
  }, [isSearchOpen, focusSearchInput]);

  useEffect(() => {
    if (!pendingFocusDraftId) return;
    const selector = `input[data-draft-id="${pendingFocusDraftId}"][data-draft-col-index="0"]`;
    requestAnimationFrame(() => {
      const target =
        containerRef.current?.querySelector<HTMLInputElement>(selector);
      if (!target) return;
      target.scrollIntoView({
        behavior: "smooth",
        block: "nearest",
        inline: "nearest",
      });
      target.focus();
      setPendingFocusDraftId(null);
    });
  }, [insertDraftRows, pendingFocusDraftId]);

  useEffect(() => {
    const handleGlobalMouseUp = () => {
      setIsRowSelecting(false);
    };
    window.addEventListener("mouseup", handleGlobalMouseUp);
    return () => {
      window.removeEventListener("mouseup", handleGlobalMouseUp);
    };
  }, []);

  useEffect(() => {
    const handleTableHotkeys = (e: KeyboardEvent) => {
      const container = containerRef.current;
      if (!container) return;

      const eventTarget = e.target instanceof Node ? e.target : null;
      const eventInsideTable = eventTarget
        ? container.contains(eventTarget)
        : false;

      // Only handle save when actively editing or having pending changes
      const shouldHandleSave =
        eventInsideTable || !!editingCell || hasPendingChanges;

      if (isModKey(e) && e.key.toLowerCase() === "s") {
        if (!shouldHandleSave) return;
        e.preventDefault();
        if (hasPendingChanges && !isSaving) {
          saveButtonRef.current?.click();
        }
        return;
      }

      if (isModKey(e) && e.key.toLowerCase() === "f") {
        if (isEditableTarget(e.target)) return;
        e.preventDefault();
        setIsSearchOpen(true);
        focusSearchInput();
        return;
      }

      if (isModKey(e) && e.key.toLowerCase() === "c") {
        if (isEditableTarget(e.target)) {
          return;
        }
        const selectedRows = selectedRowsRef.current;
        if (selectedRows.size) {
          e.preventDefault();
          const tsv = buildRowsTSV(Array.from(selectedRows));
          if (tsv) {
            handleCopy(tsv);
          }
          return;
        }
        const selectedCellText = getSelectedCellCopyText();
        if (selectedCellText !== null) {
          e.preventDefault();
          handleCopy(selectedCellText);
        }
        return;
      }

      // Only handle Escape when actively editing, inside table, or having pending changes
      const shouldHandleEscape =
        eventInsideTable || !!editingCell || hasPendingChanges;

      if (e.key === "Escape") {
        if (!shouldHandleEscape) return;

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
    selectedCell,
    selectedRows,
    hasPendingChanges,
    isSaving,
    editingCell,
    cancelEdit,
    handleDiscardChanges,
    focusSearchInput,
    buildRowsTSV,
    getSelectedCellCopyText,
    handleCopy,
  ]);

  if (isLoading) {
    return (
      <div className="flex flex-col gap-3 p-4">
        <Skeleton className="h-8 w-full" />
        <Skeleton className="h-6 w-full" />
        <Skeleton className="h-6 w-full" />
        <Skeleton className="h-6 w-full" />
        <Skeleton className="h-6 w-3/4" />
      </div>
    );
  }

  return (
    <div ref={containerRef} className="h-full flex flex-col bg-background">
      {!hideHeader && (
        <div className="flex flex-col gap-1.5 px-4 py-2 border-b border-border bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60 sticky top-0 z-20">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-1.5">
              {/* Modern pagination control */}
              <div className="flex items-center gap-1 bg-muted/40 rounded-lg p-0.5 border border-border/50">
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-6 w-6 p-0 hover:bg-background"
                  onClick={handlePrevPage}
                  disabled={page <= 1}
                >
                  <ChevronLeft className="w-3.5 h-3.5" />
                </Button>
                <div className="flex items-center gap-1 px-1">
                  <span className="text-xs text-muted-foreground">Page</span>
                  <Input
                    type="text"
                    inputMode="numeric"
                    className="h-5 w-10 px-1.5 text-xs text-center bg-background border-border/50"
                    value={pageInput}
                    onChange={(e) =>
                      setPageInput(e.target.value.replace(/\D/g, ""))
                    }
                    onBlur={handlePageInputCommit}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        handlePageInputCommit();
                      }
                    }}
                  />
                  <span className="text-xs text-muted-foreground">
                    / {totalPages}
                  </span>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-6 w-6 p-0 hover:bg-background"
                  onClick={handleNextPage}
                  disabled={page >= totalPages}
                >
                  <ChevronRight className="w-3.5 h-3.5" />
                </Button>
              </div>

              {/* Page size selector */}
              <div className="flex items-center gap-2 ml-1">
                <span className="text-xs text-muted-foreground">Limit</span>
                <Select
                  value={pageSizeInput}
                  onValueChange={handlePageSizeChange}
                >
                  <SelectTrigger
                    size="sm"
                    className="w-[70px] text-xs border-border/50 bg-muted/40 [&_svg]:size-3 px-2 gap-1 data-[size=sm]:h-6 data-[size=sm]:py-0"
                  >
                    <SelectValue placeholder="100" />
                  </SelectTrigger>
                  <SelectContent>
                    {PAGE_SIZE_OPTIONS.map((size) => (
                      <SelectItem key={size} value={size} className="text-xs">
                        {size}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              {tableContext && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-6 w-6 p-0 hover:bg-muted/60"
                  onClick={handleRefreshClick}
                  disabled={isRefreshing}
                  title={isRefreshing ? "Refreshing..." : "Refresh"}
                >
                  <RotateCw
                    className={[
                      "w-3.5 h-3.5",
                      isRefreshing ? "animate-spin" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                  />
                </Button>
              )}
              <Popover open={isSearchOpen} onOpenChange={setIsSearchOpen}>
                <PopoverTrigger asChild>
                  <Button
                    variant={isSearchOpen ? "secondary" : "ghost"}
                    size="sm"
                    className="h-6 w-6 p-0 hover:bg-muted/60"
                    title="Search in current table (Ctrl/Cmd+F)"
                  >
                    <Search className="w-3.5 h-3.5" />
                  </Button>
                </PopoverTrigger>
                <PopoverContent
                  align="start"
                  side="bottom"
                  sideOffset={6}
                  className="w-[320px] p-3 space-y-2 shadow-lg"
                >
                  <div className="flex items-center gap-2">
                    <div className="relative flex-1">
                      <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground" />
                      <Input
                        ref={searchInputRef}
                        type="text"
                        placeholder="Search keyword..."
                        className="h-8 pl-8 pr-8 text-xs"
                        value={searchKeyword}
                        onChange={(e) => setSearchKeyword(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            handleSearchEnter();
                          } else if (e.key === "Escape") {
                            e.preventDefault();
                            setIsSearchOpen(false);
                          }
                        }}
                      />
                      {searchKeyword && (
                        <button
                          onClick={() => setSearchKeyword("")}
                          className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                        >
                          <X className="w-3.5 h-3.5" />
                        </button>
                      )}
                    </div>
                  </div>
                  {normalizedSearchKeyword ? (
                    <div className="text-[11px] text-muted-foreground">
                      {matchedRows.size} row(s), {searchMatches.length}{" "}
                      match(es)
                      {currentSearchMatch
                        ? ` • ${searchCursorIndex + 1}/${searchMatches.length}`
                        : ""}
                    </div>
                  ) : (
                    <div className="text-[11px] text-muted-foreground">
                      Enter keyword, press Enter to jump next match
                    </div>
                  )}
                  {normalizedSearchKeyword && searchMatches.length === 0 && (
                    <div className="text-[11px] text-muted-foreground">
                      No matches in current table view
                    </div>
                  )}
                </PopoverContent>
              </Popover>
            </div>

            <div className="flex items-center gap-1.5">
              {tableContext && onCreateQuery && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-6 gap-1 px-2 text-xs hover:bg-muted/60"
                  onClick={() =>
                    onCreateQuery(
                      tableContext.connectionId,
                      tableContext.database,
                      tableContext.driver,
                    )
                  }
                  title={t("connection.menu.newQuery")}
                >
                  <SquareTerminal className="w-3.5 h-3.5" />
                  {t("connection.menu.newQuery")}
                </Button>
              )}
              {(canInsert || canUpdateDelete) && (
                <>
                  {canInsert && (
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-6 gap-1 px-2 text-xs hover:bg-muted/60"
                      onClick={handleAddDraftRow}
                      disabled={isSaving || isDeleting}
                      title="Add a new row draft"
                    >
                      <Plus className="w-3.5 h-3.5" />
                      Add
                    </Button>
                  )}
                  {canUpdateDelete && (
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-6 gap-1 px-2 text-xs hover:bg-destructive/10 text-destructive disabled:text-muted-foreground"
                      onClick={() => setDeleteDialogOpen(true)}
                      disabled={!selectedRows.size || isSaving || isDeleting}
                      title={
                        selectedRows.size
                          ? `Delete ${selectedRows.size} selected row(s)`
                          : "Select rows to delete"
                      }
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                      Delete
                    </Button>
                  )}
                </>
              )}

              {hasPendingChanges && (
                <div className="flex items-center gap-1 bg-amber-500/10 rounded-lg p-0.5 border border-amber-500/20">
                  <Button
                    ref={saveButtonRef}
                    variant="ghost"
                    size="sm"
                    className="h-6 gap-1.5 text-xs hover:bg-amber-500/20 text-amber-700 dark:text-amber-400"
                    onClick={handleSave}
                    disabled={isSaving}
                    title="Save changes (Cmd/Ctrl+S)"
                  >
                    {isSaving ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin" />
                    ) : (
                      <Save className="w-3.5 h-3.5" />
                    )}
                    Save
                    <span className="bg-amber-500/20 text-amber-700 dark:text-amber-400 text-[10px] px-1.5 py-0 rounded-full font-medium">
                      {pendingMutationCount}
                    </span>
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 p-0 hover:bg-amber-500/20 text-amber-700 dark:text-amber-400"
                    onClick={handleDiscardChanges}
                    disabled={isSaving}
                    title="Discard changes (Esc)"
                  >
                    <Undo2 className="w-3.5 h-3.5" />
                  </Button>
                </div>
              )}

              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 p-0 hover:bg-muted/60"
                    disabled={!tableContext || isExporting}
                    title="Export data"
                  >
                    <Download className="w-3.5 h-3.5" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuSub>
                    <DropdownMenuSubTrigger>
                      Export Current Page
                    </DropdownMenuSubTrigger>
                    <DropdownMenuSubContent>
                      <DropdownMenuItem
                        onClick={() => void handleExport("current_page", "csv")}
                      >
                        CSV
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        onClick={() =>
                          void handleExport("current_page", "json")
                        }
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
                    <DropdownMenuSubTrigger>
                      Export Filtered Result
                    </DropdownMenuSubTrigger>
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
                    <DropdownMenuSubTrigger>
                      Export Full Table
                    </DropdownMenuSubTrigger>
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
                  variant="ghost"
                  size="sm"
                  className="h-6 w-6 p-0 hover:bg-muted/60"
                  onClick={handleShowDDL}
                  title="View Table Structure (DDL)"
                >
                  <FileCode className="w-3.5 h-3.5" />
                </Button>
              )}
            </div>
          </div>

          {tableContext && onFilterChange ? (
            <div className="pt-1 border-t border-border/40 flex items-center gap-2">
              <div className="relative flex-1 min-w-0">
                <Filter className="absolute left-2 top-1/2 transform -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                <ColumnAutocompleteInput
                  placeholder="WHERE ..."
                  className="pl-8 h-7 w-full font-mono text-xs"
                  value={whereInput}
                  onValueChange={setWhereInput}
                  onSubmit={() => onFilterChange(whereInput, orderByInput)}
                  options={columnAutocompleteOptions}
                />
              </div>
              <div className="relative flex-1 min-w-0">
                <ArrowUpDown className="absolute left-2 top-1/2 transform -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                <ColumnAutocompleteInput
                  placeholder="ORDER BY ..."
                  className="pl-8 h-7 w-full font-mono text-xs"
                  value={orderByInput}
                  onValueChange={setOrderByInput}
                  onSubmit={() => onFilterChange(whereInput, orderByInput)}
                  options={columnAutocompleteOptions}
                />
              </div>
              {tableContext && mutabilityHint && (
                <span
                  className="text-xs text-muted-foreground italic"
                  title={mutabilityHint}
                >
                  {canInsert ? "Partial write" : "Read-only"}
                </span>
              )}
            </div>
          ) : (
            tableContext &&
            mutabilityHint && (
              <span
                className="text-xs text-muted-foreground italic"
                title={mutabilityHint}
              >
                {canInsert ? "Partial write" : "Read-only"}
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
          <thead className="bg-muted/90 sticky top-0 z-10">
            <tr>
              <th className="px-4 py-2 text-left text-xs font-semibold text-muted-foreground border-b border-r border-border w-12">
                #
              </th>
              {columns.map((column) => {
                const isSorted = activeSortColumn === column;
                const direction = isSorted ? activeSortDirection : undefined;
                const comment = columnComments[column]?.trim();
                const headerTooltip = comment || column;
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
              const isRowSelected = selectedRows.has(rowIndex);
              const isMultiRowCopyTarget =
                isRowSelected && selectedRows.size > 1;
              const copyTargetRows = isMultiRowCopyTarget
                ? Array.from(selectedRows)
                : [rowIndex];

              return (
                <ContextMenu key={rowIndex}>
                  <ContextMenuTrigger asChild>
                    <tr className="hover:bg-muted/50 border-b border-border group">
                      <td
                        className={[
                          "px-4 py-2 text-xs text-muted-foreground border-r border-border cursor-pointer select-none",
                          isRowSelected
                            ? "bg-accent text-accent-foreground"
                            : "",
                        ]
                          .filter(Boolean)
                          .join(" ")}
                        onMouseDown={(e) => handleIndexMouseDown(e, rowIndex)}
                        onMouseEnter={() => handleIndexMouseEnter(rowIndex)}
                      >
                        {startIndex + rowIndex + 1}
                      </td>
                      {columns.map((column, colIndex) => {
                        const modified = isCellModified(rowIndex, column);
                        const displayValue = getCellDisplayValue(
                          rowIndex,
                          column,
                          row[column],
                        );
                        const editing = isEditing(column);
                        const selected = isSelected(column);
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
                              selected && !editing
                                ? "bg-accent text-accent-foreground"
                                : "",
                              isRowSelected && !selected && !editing
                                ? "bg-accent/60"
                                : "",
                              matched && !editing
                                ? "bg-amber-100/60 dark:bg-amber-900/20"
                                : "",
                              activeSearchMatch && !editing
                                ? "border-b-2 border-b-amber-500/70"
                                : "",
                              modified && !editing
                                ? "border-l-2 border-l-orange-400"
                                : "",
                              isEditableForUpdates ? "cursor-pointer" : "",
                            ]
                              .filter(Boolean)
                              .join(" ")}
                            style={{
                              width: getColWidth(column),
                              minWidth: 50,
                            }}
                            onClick={() => handleCellClick(rowIndex, column)}
                            onContextMenu={() => {
                              if (
                                selectedRows.size > 1 &&
                                selectedRows.has(rowIndex)
                              ) {
                                return;
                              }
                              handleCellClick(rowIndex, column);
                            }}
                            onDoubleClick={() =>
                              handleCellDoubleClick(
                                rowIndex,
                                column,
                                row[column],
                              )
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
                                {displayValue !== null &&
                                displayValue !== undefined ? (
                                  <span
                                    className={modified ? "text-orange-600 dark:text-orange-400" : ""}
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
                                      setComplexViewer({ value: displayValue, columnName: column });
                                    }}
                                  >
                                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
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
                  </ContextMenuTrigger>
                  <ContextMenuContent>
                    <ContextMenuItem
                      onClick={() => {
                        if (selectedCell && selectedCell.row === rowIndex) {
                          const text = getSelectedCellCopyText();
                          if (text !== null) {
                            handleCopy(text);
                          }
                        }
                      }}
                    >
                      <Copy className="w-4 h-4 mr-2" />
                      Copy Cell
                    </ContextMenuItem>
                    <ContextMenuItem
                      onClick={() => {
                        if (isMultiRowCopyTarget) {
                          handleCopy(buildRowsTSV(copyTargetRows));
                          return;
                        }
                        const values = columns
                          .map((col) => {
                            const val = getCellDisplayValue(
                              rowIndex,
                              col,
                              row[col],
                            );
                            return val === null || val === undefined
                              ? ""
                              : String(val);
                          })
                          .join("\t");
                        handleCopy(values);
                      }}
                    >
                      <TableIcon className="w-4 h-4 mr-2" />
                      {isMultiRowCopyTarget ? "Copy Selected Rows" : "Copy Row"}
                    </ContextMenuItem>
                    <ContextMenuSeparator />
                    {canUpdateDelete &&
                      isCellModified(rowIndex, selectedCell?.col || "") && (
                        <>
                          <ContextMenuItem
                            onClick={() => {
                              if (
                                selectedCell &&
                                selectedCell.row === rowIndex
                              ) {
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
                            handleCopy(buildRowsCSV(copyTargetRows));
                          }}
                        >
                          {isMultiRowCopyTarget
                            ? "Copy Selected as CSV"
                            : "Copy as CSV"}
                        </ContextMenuItem>
                        {!!tableContext && (
                          <ContextMenuItem
                            onClick={() => {
                              const sql = buildRowsInsertSQL(copyTargetRows);
                              handleCopy(sql);
                            }}
                          >
                            {isMultiRowCopyTarget
                              ? "Copy Selected as Insert SQL"
                              : "Copy as Insert SQL"}
                          </ContextMenuItem>
                        )}
                        {canUpdateDelete && (
                          <ContextMenuItem
                            onClick={() => {
                              const sql = buildRowsUpdateSQL(copyTargetRows);
                              handleCopy(sql);
                            }}
                          >
                            {isMultiRowCopyTarget
                              ? "Copy Selected as Update SQL"
                              : "Copy as Update SQL"}
                          </ContextMenuItem>
                        )}
                      </ContextMenuSubContent>
                    </ContextMenuSub>
                  </ContextMenuContent>
                </ContextMenu>
              );
            })}
            {insertDraftRows.map((draft, draftIndex) => (
              <tr
                key={draft.tempId}
                className="border-b border-border bg-emerald-500/5"
              >
                <td className="px-4 py-2 text-xs text-emerald-700 dark:text-emerald-300 border-r border-border font-medium">
                  new
                  {insertDraftRows.length > 1 ? ` ${draftIndex + 1}` : ""}
                </td>
                {columns.map((column, colIndex) => (
                  <td
                    key={`${draft.tempId}_${column}`}
                    className="px-0 py-0 text-sm text-foreground font-mono border-r border-border"
                    style={{
                      width: getColWidth(column),
                      minWidth: 50,
                    }}
                  >
                    <input
                      type="text"
                      autoCapitalize="none"
                      data-draft-id={draft.tempId}
                      data-draft-col-index={colIndex}
                      className="w-full h-full px-4 py-2 bg-transparent outline-none"
                      placeholder={column}
                      value={draft.values[column] ?? ""}
                      onChange={(e) =>
                        handleDraftValueChange(
                          draft.tempId,
                          column,
                          e.target.value,
                        )
                      }
                    />
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <AlertDialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete selected rows?</AlertDialogTitle>
            <AlertDialogDescription>
              This action will permanently delete {selectedRows.size} row(s)
              from the table.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={isDeleting}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              disabled={isDeleting}
              onClick={async (e) => {
                e.preventDefault();
                await handleConfirmDelete();
              }}
            >
              {isDeleting ? "Deleting..." : "Delete"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

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

      {complexViewer && (
        <ComplexValueViewer
          value={complexViewer.value}
          columnName={complexViewer.columnName}
          open={true}
          onOpenChange={(open) => { if (!open) setComplexViewer(null); }}
        />
      )}

      <div className="flex items-center px-4 py-1 border-t border-border bg-muted/40">
        <div className="text-sm text-muted-foreground">
          Query executed in{" "}
          {executionTimeMs ? (executionTimeMs / 1000).toFixed(3) : "0.000"}s •{" "}
          {sortedData.length} rows returned
          {normalizedSearchKeyword && (
            <span className="ml-2">
              • {matchedRows.size} row(s) matched "{searchKeyword.trim()}"
            </span>
          )}
          {isRefreshing && <span className="ml-2">• Refreshing…</span>}
          {lastRefreshedAt && !isRefreshing && (
            <span className="ml-2">
              • Updated {lastRefreshedAt.toLocaleTimeString()}
            </span>
          )}
          {hasPendingChanges && (
            <span className="text-orange-600 dark:text-orange-400 ml-2">
              • {pendingMutationCount} unsaved change(s)
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
