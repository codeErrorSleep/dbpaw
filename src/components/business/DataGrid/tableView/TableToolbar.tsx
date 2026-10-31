import React from "react";
import { useTranslation } from "react-i18next";
import {
  Download,
  Filter,
  ChevronLeft,
  ChevronRight,
  ArrowUpDown,
  Columns,
  Rows,
  FileCode,
  Hash,
  Save,
  Undo2,
  Loader2,
  RotateCw,
  Search,
  Plus,
  SquareTerminal,
  Trash2,
  X,
  Table,
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
import { ColumnAutocompleteInput } from "./ColumnAutocompleteInput";
import type { ColumnAutocompleteOption } from "./columnAutocomplete";

interface TableToolbarProps {
  hideHeader: boolean;
  columnSearchControl?: React.ReactNode;
  // Pagination
  page: number;
  totalPages: number | null;
  canGoNext: boolean;
  pageInput: string;
  pageSizeInput: string;
  PAGE_SIZE_OPTIONS: readonly string[];
  handlePrevPage: () => void;
  handleNextPage: () => void;
  handlePageInputCommit: () => void;
  setPageInput: (v: string) => void;
  handlePageSizeChange: (v: string) => void;
  // Refresh
  tableContext?: {
    connectionId: number;
    database: string;
    schema: string;
    table: string;
    driver: string;
  };
  isRefreshing: boolean;
  handleRefreshClick: () => void;
  includeTotal: boolean;
  onIncludeTotalChange?: (includeTotal: boolean) => void | Promise<unknown>;
  // View mode
  viewMode: "table" | "column";
  setViewMode: (m: "table" | "column") => void;
  // Search
  isSearchOpen: boolean;
  setIsSearchOpen: (open: boolean) => void;
  searchKeyword: string;
  setSearchKeyword: (kw: string) => void;
  normalizedSearchKeyword: string;
  matchedRowsSize: number;
  searchMatchesLength: number;
  currentSearchMatch: unknown;
  searchCursorIndex: number;
  handleSearchEnter: () => void;
  searchInputRef: React.RefObject<HTMLInputElement | null>;
  // DDL/ER/Query
  onCreateQuery?: (
    connectionId: number,
    database: string,
    driver: string,
  ) => void;
  onShowDDL: () => void;
  onOpenERDiagram?: (ctx: {
    connectionId: number;
    database: string;
    schema: string;
    table: string;
    driver: string;
  }) => void;
  // Mutations
  canInsert: boolean;
  canUpdateDelete: boolean;
  hasPendingChanges: boolean;
  pendingMutationCount: number;
  isSaving: boolean;
  isDeleting: boolean;
  selectedRowsSize: number;
  saveButtonRef: React.RefObject<HTMLButtonElement | null>;
  handleAddDraftRow: () => void;
  setDeleteDialogOpen: (open: boolean) => void;
  handleSave: () => void;
  handleDiscardChanges: () => void;
  // Export
  isExporting: boolean;
  handleExport: (
    scope: "current_page" | "filtered" | "full_table",
    format: "csv" | "json" | "sql_dml",
  ) => void;
  // Filter
  whereInput: string;
  setWhereInput: (v: string) => void;
  orderByInput: string;
  setOrderByInput: (v: string) => void;
  onFilterChange?: (filter: string, orderBy: string) => void;
  columnAutocompleteOptions: ColumnAutocompleteOption[];
  // Mutability
  mutabilityHint: string | null;
}

export const TableToolbar = React.memo(function TableToolbar({
  hideHeader,
  columnSearchControl,
  page,
  totalPages,
  canGoNext,
  pageInput,
  pageSizeInput,
  PAGE_SIZE_OPTIONS,
  handlePrevPage,
  handleNextPage,
  handlePageInputCommit,
  setPageInput,
  handlePageSizeChange,
  tableContext,
  isRefreshing,
  handleRefreshClick,
  includeTotal,
  onIncludeTotalChange,
  viewMode,
  setViewMode,
  isSearchOpen,
  setIsSearchOpen,
  searchKeyword,
  setSearchKeyword,
  normalizedSearchKeyword,
  matchedRowsSize,
  searchMatchesLength,
  currentSearchMatch,
  searchCursorIndex,
  handleSearchEnter,
  searchInputRef,
  onCreateQuery,
  onShowDDL,
  onOpenERDiagram,
  canInsert,
  canUpdateDelete,
  hasPendingChanges,
  pendingMutationCount,
  isSaving,
  isDeleting,
  selectedRowsSize,
  saveButtonRef,
  handleAddDraftRow,
  setDeleteDialogOpen,
  handleSave,
  handleDiscardChanges,
  isExporting,
  handleExport,
  whereInput,
  setWhereInput,
  orderByInput,
  setOrderByInput,
  onFilterChange,
  columnAutocompleteOptions,
  mutabilityHint,
}: TableToolbarProps) {
  const { t } = useTranslation();

  if (hideHeader) {
    return (
      <div className="flex justify-end border-b border-border px-4 py-1">
        {columnSearchControl}
      </div>
    );
  }

  return (
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
              aria-label="Previous page"
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
                / {totalPages ?? "?"}
              </span>
            </div>
            <Button
              variant="ghost"
              size="sm"
              className="h-6 w-6 p-0 hover:bg-background"
              onClick={handleNextPage}
              disabled={!canGoNext}
              aria-label="Next page"
            >
              <ChevronRight className="w-3.5 h-3.5" />
            </Button>
          </div>

          {/* Page size selector */}
          <div className="flex items-center gap-2 ml-1">
            <span className="text-xs text-muted-foreground">Limit</span>
            <Select value={pageSizeInput} onValueChange={handlePageSizeChange}>
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
              aria-label="Refresh"
            >
              <RotateCw
                className={["w-3.5 h-3.5", isRefreshing ? "animate-spin" : ""]
                  .filter(Boolean)
                  .join(" ")}
              />
            </Button>
          )}
          {tableContext && onIncludeTotalChange && (
            <Button
              variant={includeTotal ? "secondary" : "ghost"}
              size="sm"
              className="h-6 w-6 p-0 hover:bg-muted/60"
              onClick={() => onIncludeTotalChange(!includeTotal)}
              title={
                includeTotal
                  ? t("tableView.toolbar.totalOn")
                  : t("tableView.toolbar.totalOff")
              }
              aria-label={
                includeTotal
                  ? t("tableView.toolbar.totalOn")
                  : t("tableView.toolbar.totalOff")
              }
            >
              <Hash className="w-3.5 h-3.5" />
            </Button>
          )}
          <Button
            variant="ghost"
            size="sm"
            className="h-6 w-6 p-0 hover:bg-muted/60"
            onClick={() =>
              setViewMode(viewMode === "table" ? "column" : "table")
            }
            title={
              viewMode === "table"
                ? "Switch to column view"
                : "Switch to table view"
            }
            aria-label={
              viewMode === "table" ? "Toggle column view" : "Toggle table view"
            }
          >
            {viewMode === "table" ? (
              <Columns className="w-3.5 h-3.5" />
            ) : (
              <Rows className="w-3.5 h-3.5" />
            )}
          </Button>
          {columnSearchControl}
          <Popover open={isSearchOpen} onOpenChange={setIsSearchOpen}>
            <PopoverTrigger asChild>
              <Button
                variant={isSearchOpen ? "secondary" : "ghost"}
                size="sm"
                className="h-6 w-6 p-0 hover:bg-muted/60"
                title="Search in current table (Ctrl/Cmd+F)"
                aria-label="Search"
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
                  {matchedRowsSize} row(s), {searchMatchesLength} match(es)
                  {currentSearchMatch
                    ? ` • ${searchCursorIndex + 1}/${searchMatchesLength}`
                    : ""}
                </div>
              ) : (
                <div className="text-[11px] text-muted-foreground">
                  Enter keyword, press Enter to jump next match
                </div>
              )}
              {normalizedSearchKeyword && searchMatchesLength === 0 && (
                <div className="text-[11px] text-muted-foreground">
                  No matches in current table view
                </div>
              )}
            </PopoverContent>
          </Popover>
        </div>

        <div className="flex items-center gap-1.5">
          {tableContext && onCreateQuery && (
            <>
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
                aria-label={t("connection.menu.newQuery")}
              >
                <SquareTerminal className="w-3.5 h-3.5" />
                {t("connection.menu.newQuery")}
              </Button>

              <Button
                variant="ghost"
                size="sm"
                className="h-6 gap-1 px-2 hover:bg-muted/60"
                onClick={onShowDDL}
                title="View Table Structure (DDL)"
                aria-label="DDL"
              >
                <FileCode className="w-3.5 h-3.5" />
                <span className="text-xs font-medium leading-none">ddl</span>
              </Button>

              <Button
                variant="ghost"
                size="sm"
                className="h-6 gap-1 px-2 hover:bg-muted/60"
                onClick={() => {
                  if (tableContext && onOpenERDiagram) {
                    onOpenERDiagram(tableContext);
                  }
                }}
                title="Open ER Diagram"
                aria-label="ER Diagram"
              >
                <Table className="w-3.5 h-3.5" />
                <span className="text-xs font-medium leading-none">ER</span>
              </Button>
            </>
          )}
          {(canInsert || canUpdateDelete) && (
            <>
              {canInsert && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-6 w-6 p-0 hover:bg-muted/60"
                  onClick={handleAddDraftRow}
                  disabled={isSaving || isDeleting}
                  title="Add a new row draft"
                  aria-label="Add row"
                >
                  <Plus className="w-3.5 h-3.5" />
                </Button>
              )}
              {canUpdateDelete && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-6 w-6 p-0 hover:bg-destructive/10 text-destructive disabled:text-muted-foreground"
                  onClick={() => setDeleteDialogOpen(true)}
                  disabled={!selectedRowsSize || isSaving || isDeleting}
                  title={
                    selectedRowsSize
                      ? `Delete ${selectedRowsSize} selected row(s)`
                      : "Select rows to delete"
                  }
                  aria-label="Delete selected rows"
                >
                  <Trash2 className="w-3.5 h-3.5" />
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
                aria-label="Save"
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
                aria-label="Discard"
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
                aria-label="Export"
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
                    onClick={() => void handleExport("current_page", "json")}
                  >
                    JSON
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onClick={() => void handleExport("current_page", "sql_dml")}
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
                    onClick={() => void handleExport("filtered", "sql_dml")}
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
                    onClick={() => void handleExport("full_table", "sql_dml")}
                  >
                    SQL
                  </DropdownMenuItem>
                </DropdownMenuSubContent>
              </DropdownMenuSub>
            </DropdownMenuContent>
          </DropdownMenu>
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
  );
});
