import { useState, useRef, useEffect, useCallback } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useTranslation } from "react-i18next";
import { Skeleton } from "@/components/ui/skeleton";
import { VirtualTableBody } from "./tableView/VirtualTableBody";
import { ColumnViewBody } from "./tableView/ColumnViewBody";
import { ComplexValueViewer } from "./ComplexValueViewer";
import { TableToolbar } from "./tableView/TableToolbar";
import { ColumnSearchControl } from "./tableView/ColumnSearchControl";
import { useColumnSearch } from "./tableView/hooks/useColumnSearch";
import { useTableSort } from "./tableView/hooks/useTableSort";
import { useTablePagination } from "./tableView/hooks/useTablePagination";
import { useColumnState } from "./tableView/hooks/useColumnState";
import { useCellSelection } from "./tableView/hooks/useCellSelection";
import { useCellEditing } from "./tableView/hooks/useCellEditing";
import { useTableMutation } from "./tableView/hooks/useTableMutation";
import { useTableSearch } from "./tableView/hooks/useTableSearch";
import { useTableClipboard } from "./tableView/hooks/useTableClipboard";
import { useTableHotkeys } from "./tableView/hooks/useTableHotkeys";
import { useTableData } from "./tableView/hooks/useTableData";
import { useTableFilter } from "./tableView/hooks/useTableFilter";
import { DeleteConfirmDialog } from "./tableView/DeleteConfirmDialog";
import { SaveErrorBanner } from "./tableView/SaveErrorBanner";
import { TableStatusBar } from "./tableView/TableStatusBar";
import type { TableContext, TableRow } from "./tableView/types";

const EMPTY_ROWS: TableRow[] = [];
const EMPTY_COLUMNS: string[] = [];

interface TableViewProps {
  data?: TableRow[];
  columns?: string[];
  hideHeader?: boolean;
  total?: number | null;
  page?: number;
  pageSize?: number;
  includeTotal?: boolean;
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
  onOpenERDiagram?: (ctx: {
    connectionId: number;
    database: string;
    schema: string;
    table: string;
    driver: string;
  }) => void;
  onDataRefresh?: (params?: {
    page?: number;
    limit?: number;
    filter?: string;
    orderBy?: string;
    includeTotal?: boolean;
  }) => void | Promise<unknown>;
  onIncludeTotalChange?: (includeTotal: boolean) => void | Promise<unknown>;
  onCreateQuery?: (
    connectionId: number,
    database: string,
    driver: string,
  ) => void;
  tableContext?: TableContext;
  isLoading?: boolean;
  showColumnComments?: boolean;
  showRowNumbers?: boolean;
  showZebraStripes?: boolean;
}

export function TableView({
  data = EMPTY_ROWS,
  columns = EMPTY_COLUMNS,
  hideHeader = false,
  total = 0,
  page = 1,
  pageSize = 100,
  includeTotal = false,
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
  onOpenERDiagram,
  onDataRefresh,
  onIncludeTotalChange,
  onCreateQuery,
  tableContext,
  isLoading,
  showColumnComments = false,
  showRowNumbers = true,
  showZebraStripes = false,
}: TableViewProps) {
  const { t } = useTranslation();
  const [viewMode, setViewMode] = useState<"table" | "column">("table");
  const {
    getColWidth,
    tableWidthPx,
    thRefs,
    handleMouseDown,
    INDEX_COL_WIDTH,
  } = useColumnState({ data, columns });
  const headerClickStateRef = useRef<
    Record<string, { timerId: ReturnType<typeof setTimeout> | null }>
  >({});

  // --- Cell selection & editing state ---
  const {
    selectedCell,
    selectedCellRef,
    setSelectedCell,
    selectedRows,
    selectedRowsRef,
    setSelectedRows,
    cellSelectionRange,
    cellSelectionRangeRef,
    handleCellClick: handleCellClickBase,
    handleCellMouseDownForRange: handleCellMouseDownForRangeBase,
    handleCellMouseMoveForRange,
    handleCellMouseUpForRange,
    handleIndexMouseDown,
    handleIndexMouseEnter,
    clearSelection,
  } = useCellSelection();
  const [complexViewer, setComplexViewer] = useState<{
    value: unknown;
    columnName: string;
  } | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [contextMenuRow, setContextMenuRow] = useState<number | null>(null);

  const {
    activeSortColumn,
    activeSortDirection,
    handleSortClick,
    hasLocalClientSort,
    isControlledSort,
  } = useTableSort({
    controlledSortColumn,
    controlledSortDirection,
    onSortChange,
  });

  const { sortedData, currentData, totalPages, canGoNext } = useTableData({
    data,
    activeSortColumn,
    activeSortDirection,
    isControlledSort,
    total,
    page,
    pageSize,
    onPageChange,
  });

  const {
    whereInput,
    setWhereInput,
    orderByInput,
    setOrderByInput,
    pageInput,
    setPageInput,
    pageSizeInput,
    handlePageInputCommit,
    handlePageSizeChange,
    handlePrevPage,
    handleNextPage,
    PAGE_SIZE_OPTIONS,
  } = useTablePagination({
    page,
    pageSize,
    controlledFilter,
    controlledOrderBy,
    totalPages,
    canGoNext,
    onPageChange,
    onPageSizeChange,
  });

  // --- Cell editing hook ---
  const {
    editingCell,
    editValue,
    setEditValue,
    insertDraftRows,
    primaryKeys,
    tableColumns,
    columnComments,
    columnAutocompleteOptions,
    isSaving,
    isRefreshing,
    isDeleting,
    deleteDialogOpen,
    setDeleteDialogOpen,
    lastRefreshedAt,
    saveError,
    setSaveError,
    pendingFocusDraftId,
    setPendingFocusDraftId,
    canInsert,
    canUpdateDelete,
    hasPendingChanges,
    pendingMutationCount,
    mutabilityHint,
    isEditableForUpdates,
    editInputRef,
    saveButtonRef,
    commitEdit,
    cancelEdit,
    handleEditKeyDown,
    handleCellDoubleClick,
    handleSave,
    handleConfirmDelete,
    handleDiscardChanges,
    handleAddDraftRow,
    handleDraftValueChange,
    handleRefreshClick,
    getCellDisplayValue,
    isCellModified,
    setPendingChanges,
    editingCellRef,
    commitEditRef,
  } = useCellEditing({
    data,
    currentData,
    columns,
    tableContext,
    onDataRefresh,
    selectedCell,
    selectedCellRef,
    selectedRows,
    selectedRowsRef,
    setSelectedCell,
    setSelectedRows,
    clearSelection,
    hasLocalClientSort,
    whereInput,
    orderByInput,
    pageInput,
    pageSizeInput,
    page,
    pageSize,
  });

  // --- Search hook ---
  const {
    isSearchOpen,
    setIsSearchOpen,
    searchKeyword,
    setSearchKeyword,
    searchCursorIndex,
    searchInputRef,
    normalizedSearchKeyword,
    searchMatches,
    matchedRows,
    matchedCellKeys,
    currentSearchMatch,
    focusSearchInput,
    handleSearchEnter,
  } = useTableSearch({
    currentData,
    columns,
    editingCell,
    commitEdit,
    getCellDisplayValue,
    setSelectedCell,
    containerRef,
  });

  const columnSearch = useColumnSearch({
    columns,
    columnComments,
    scrollContainerRef,
    viewMode,
    isLoading,
  });

  // Virtual scrolling — only render visible rows
  const virtualizer = useVirtualizer({
    count: currentData.length + insertDraftRows.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: () => 36,
    overscan: 10,
  });

  const handleShowDDL = () => {
    if (!tableContext) return;
    onOpenDDL?.(tableContext);
  };

  const { isExporting, handleExport } = useTableMutation({
    tableContext,
    controlledFilter,
    orderByInput,
    activeSortColumn,
    activeSortDirection,
    page,
    pageSize,
  });

  // --- Cell interaction handlers ---
  const handleCellClick = useCallback(
    (rowIndex: number, col: string) => {
      // If clicking a different cell while editing, commit current edit first
      const ec = editingCellRef.current;
      if (ec && (ec.row !== rowIndex || ec.col !== col)) {
        commitEditRef.current?.();
      }
      handleCellClickBase(rowIndex, col);
    },
    [handleCellClickBase, commitEditRef, editingCellRef],
  );

  const handleCellMouseDownForRange = useCallback(
    (e: React.MouseEvent, rowIndex: number, colIndex: number) => {
      if (editingCellRef.current) return;
      handleCellMouseDownForRangeBase(e, rowIndex, colIndex, columns);
    },
    [handleCellMouseDownForRangeBase, columns, editingCellRef],
  );

  const {
    handleCopy,
    handleHeaderCopy,
    buildRowsTSV,
    getSelectedCellCopyText,
    getNormalizedCellRange,
    handleCopySelection,
    buildSelectionCSV,
    buildSelectionInsertSQL,
    buildSelectionUpdateSQL,
    handlePaste,
    buildRowsCSV,
    buildRowsInsertSQL,
    buildRowsUpdateSQL,
  } = useTableClipboard({
    t,
    data,
    currentData,
    columns,
    tableContext,
    canUpdateDelete,
    primaryKeys,
    isEditableForUpdates,
    selectedCellRef,
    cellSelectionRange,
    getCellDisplayValue,
    setPendingChanges,
  });

  const { applyFilter } = useTableFilter({
    selectedCell,
    currentData,
    tableColumns,
    tableContext,
    orderByInput,
    onFilterChange,
    setWhereInput,
  });

  // Correctly calculate start index for display
  const startIndex = (page - 1) * pageSize;

  useEffect(() => {
    const clickStates = headerClickStateRef.current;
    return () => {
      Object.values(clickStates).forEach((state) => {
        if (state.timerId) {
          clearTimeout(state.timerId);
          state.timerId = null;
        }
      });
    };
  }, []);

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
  }, [insertDraftRows, pendingFocusDraftId, setPendingFocusDraftId]);

  useTableHotkeys({
    containerRef,
    editingCell,
    hasPendingChanges,
    isSaving,
    saveButtonRef,
    setIsSearchOpen,
    focusSearchInput,
    selectedRowsRef,
    cellSelectionRangeRef,
    buildRowsTSV,
    getSelectedCellCopyText,
    handleCopy,
    handleCopySelection,
    cancelEdit,
    handleDiscardChanges,
  });

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
    <div
      ref={containerRef}
      className="h-full flex flex-col bg-background"
      onMouseUp={handleCellMouseUpForRange}
      onPaste={isEditableForUpdates ? handlePaste : undefined}
    >
      <TableToolbar
        hideHeader={hideHeader}
        columnSearchControl={<ColumnSearchControl search={columnSearch} />}
        page={page}
        totalPages={totalPages}
        canGoNext={canGoNext}
        pageInput={pageInput}
        pageSizeInput={pageSizeInput}
        PAGE_SIZE_OPTIONS={PAGE_SIZE_OPTIONS}
        handlePrevPage={handlePrevPage}
        handleNextPage={handleNextPage}
        handlePageInputCommit={handlePageInputCommit}
        setPageInput={setPageInput}
        handlePageSizeChange={handlePageSizeChange}
        tableContext={tableContext}
        isRefreshing={isRefreshing}
        handleRefreshClick={handleRefreshClick}
        includeTotal={includeTotal}
        onIncludeTotalChange={onIncludeTotalChange}
        viewMode={viewMode}
        setViewMode={setViewMode}
        isSearchOpen={isSearchOpen}
        setIsSearchOpen={setIsSearchOpen}
        searchKeyword={searchKeyword}
        setSearchKeyword={setSearchKeyword}
        normalizedSearchKeyword={normalizedSearchKeyword}
        matchedRowsSize={matchedRows.size}
        searchMatchesLength={searchMatches.length}
        currentSearchMatch={currentSearchMatch}
        searchCursorIndex={searchCursorIndex}
        handleSearchEnter={handleSearchEnter}
        searchInputRef={searchInputRef}
        onCreateQuery={onCreateQuery}
        onShowDDL={handleShowDDL}
        onOpenERDiagram={onOpenERDiagram}
        canInsert={canInsert}
        canUpdateDelete={canUpdateDelete}
        hasPendingChanges={hasPendingChanges}
        pendingMutationCount={pendingMutationCount}
        isSaving={isSaving}
        isDeleting={isDeleting}
        selectedRowsSize={selectedRows.size}
        saveButtonRef={saveButtonRef}
        handleAddDraftRow={handleAddDraftRow}
        setDeleteDialogOpen={setDeleteDialogOpen}
        handleSave={handleSave}
        handleDiscardChanges={handleDiscardChanges}
        isExporting={isExporting}
        handleExport={handleExport}
        whereInput={whereInput}
        setWhereInput={setWhereInput}
        orderByInput={orderByInput}
        setOrderByInput={setOrderByInput}
        onFilterChange={onFilterChange}
        columnAutocompleteOptions={columnAutocompleteOptions}
        mutabilityHint={mutabilityHint}
      />

      <div ref={scrollContainerRef} className="flex-1 overflow-auto">
        {viewMode === "column" ? (
          <ColumnViewBody
            columns={columns}
            columnSearch={columnSearch.highlight}
            currentData={currentData}
            startIndex={startIndex}
            showRowNumbers={showRowNumbers}
            showZebraStripes={showZebraStripes}
            showColumnComments={showColumnComments}
            columnComments={columnComments}
            normalizedSearchKeyword={normalizedSearchKeyword}
            matchedCellKeys={matchedCellKeys}
            isEditableForUpdates={isEditableForUpdates}
            editingCell={editingCell}
            selectedCell={selectedCell}
            editValue={editValue}
            editInputRef={editInputRef}
            cellSelectionRange={cellSelectionRange}
            getCellDisplayValue={getCellDisplayValue}
            isCellModified={isCellModified}
            handleCellClick={handleCellClick}
            handleCellDoubleClick={handleCellDoubleClick}
            handleCellMouseDownForRange={handleCellMouseDownForRange}
            handleCellMouseMoveForRange={handleCellMouseMoveForRange}
            handleEditKeyDown={handleEditKeyDown}
            setEditValue={setEditValue}
            commitEdit={commitEdit}
            setComplexViewer={setComplexViewer}
          />
        ) : (
          <VirtualTableBody
            columns={columns}
            columnSearch={columnSearch.highlight}
            currentData={currentData}
            virtualizer={virtualizer}
            startIndex={startIndex}
            showRowNumbers={showRowNumbers}
            showZebraStripes={showZebraStripes}
            showColumnComments={showColumnComments}
            columnComments={columnComments}
            getColWidth={getColWidth}
            tableWidthPx={tableWidthPx}
            INDEX_COL_WIDTH={INDEX_COL_WIDTH}
            thRefs={thRefs}
            activeSortColumn={activeSortColumn}
            activeSortDirection={activeSortDirection}
            selectedCell={selectedCell}
            selectedRows={selectedRows}
            editingCell={editingCell}
            editValue={editValue}
            cellSelectionRange={cellSelectionRange}
            normalizedSearchKeyword={normalizedSearchKeyword}
            matchedCellKeys={matchedCellKeys}
            currentSearchMatch={currentSearchMatch}
            isEditableForUpdates={isEditableForUpdates}
            editInputRef={editInputRef}
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
            handleSortClick={handleSortClick}
            handleHeaderCopy={handleHeaderCopy}
            handleMouseDown={handleMouseDown}
            insertDraftRows={insertDraftRows}
            handleDraftValueChange={handleDraftValueChange}
            contextMenuRow={contextMenuRow}
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
            applyFilter={applyFilter}
            setPendingChanges={setPendingChanges}
            headerClickStateRef={headerClickStateRef}
            t={t}
          />
        )}
      </div>

      <DeleteConfirmDialog
        open={deleteDialogOpen}
        onOpenChange={setDeleteDialogOpen}
        selectedRowsSize={selectedRows.size}
        isDeleting={isDeleting}
        onConfirmDelete={handleConfirmDelete}
      />

      <SaveErrorBanner
        error={saveError}
        onDismiss={() => setSaveError(null)}
      />

      {complexViewer && (
        <ComplexValueViewer
          value={complexViewer.value}
          columnName={complexViewer.columnName}
          open={true}
          onOpenChange={(open) => {
            if (!open) setComplexViewer(null);
          }}
        />
      )}

      <TableStatusBar
        executionTimeMs={executionTimeMs}
        sortedDataLength={sortedData.length}
        normalizedSearchKeyword={normalizedSearchKeyword}
        matchedRowsSize={matchedRows.size}
        searchKeyword={searchKeyword}
        isRefreshing={isRefreshing}
        lastRefreshedAt={lastRefreshedAt}
        hasPendingChanges={hasPendingChanges}
        pendingMutationCount={pendingMutationCount}
      />
    </div>
  );
}
