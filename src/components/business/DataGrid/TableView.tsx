import { useState, useRef, useEffect, useCallback } from "react";
import {
  Download,
  Filter,
  Search,
  ChevronLeft,
  ChevronRight,
  Copy,
  Table as TableIcon,
  Files,
  FileCode,
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";
import Editor from "@monaco-editor/react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { api } from "@/services/api";

interface TableViewProps {
  data?: any[];
  columns?: string[];
  hideHeader?: boolean;
  total?: number;
  page?: number;
  pageSize?: number;
  executionTimeMs?: number;
  onPageChange?: (page: number) => void;
  tableContext?: {
    connectionId: number;
    database: string;
    schema: string;
    table: string;
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
  tableContext,
}: TableViewProps) {
  const [searchTerm, setSearchTerm] = useState("");
  const [columnWidths, setColumnWidths] = useState<Record<string, number>>({});
  const [isDDLModalOpen, setIsDDLModalOpen] = useState(false);
  const [ddlContent, setDDLContent] = useState("");
  const [isLoadingDDL, setIsLoadingDDL] = useState(false);

  // Refs for table header cells to measure actual width
  const thRefs = useRef<Record<string, HTMLTableCellElement | null>>({});

  const handleShowDDL = async () => {
    if (!tableContext) return;
    setIsDDLModalOpen(true);
    if (!ddlContent || ddlContent.startsWith("-- Error")) {
      setIsLoadingDDL(true);
      try {
        const ddl = await api.metadata.getTableDDL(
          tableContext.connectionId,
          tableContext.database,
          tableContext.schema,
          tableContext.table,
        );
        setDDLContent(ddl);
      } catch (error) {
        setDDLContent(`-- Error fetching DDL\n-- ${error}`);
      } finally {
        setIsLoadingDDL(false);
      }
    }
  };

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

  const filteredData = data.filter((row) =>
    Object.values(row).some((value) =>
      String(value).toLowerCase().includes(searchTerm.toLowerCase()),
    ),
  );

  // If using external pagination, totalPages is based on total count
  // Otherwise fallback to filtered data length
  const totalPages = Math.ceil((total || filteredData.length) / pageSize);

  // If external pagination is used (onPageChange provided), we assume data is already the current page
  // Otherwise we slice locally
  const currentData = onPageChange
    ? filteredData
    : filteredData.slice((page - 1) * pageSize, page * pageSize);

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

  return (
    <div className="h-full flex flex-col bg-background">
      {!hideHeader && (
        <div className="flex items-center justify-between px-4 py-2 border-b border-border">
          <div className="flex items-center gap-2">
            <div className="relative">
              <Search className="absolute left-2 top-1/2 transform -translate-y-1/2 w-4 h-4 text-muted-foreground" />
              <Input
                type="text"
                placeholder="Search..."
                className="pl-8 h-8 w-64"
                value={searchTerm}
                onChange={(e) => setSearchTerm(e.target.value)}
              />
            </div>
            <Button variant="outline" size="sm" className="gap-2">
              <Filter className="w-4 h-4" />
            </Button>
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
          </div>
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">
              Showing {startIndex + 1}-{startIndex + currentData.length} of{" "}
              {total || filteredData.length} rows
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
            minWidth: "100%",
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
              {columns.map((column) => (
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
                  <div className="flex items-center justify-between">
                    <span className="truncate">{column}</span>
                    <div
                      className="absolute right-0 top-0 bottom-0 w-1 cursor-col-resize hover:bg-primary/50 group-hover:bg-muted-foreground/20 select-none touch-none"
                      onMouseDown={(e) => handleMouseDown(e, column)}
                    />
                  </div>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {currentData.map((row, rowIndex) => (
              <ContextMenu key={rowIndex}>
                <ContextMenuTrigger asChild>
                  <tr className="hover:bg-muted/50 border-b border-border group">
                    <td className="px-4 py-2 text-xs text-muted-foreground border-r border-border">
                      {startIndex + rowIndex + 1}
                    </td>
                    {columns.map((column) => (
                      <td
                        key={column}
                        className="px-4 py-2 text-sm text-foreground font-mono truncate border-r border-border"
                        style={{
                          width: getColWidth(column),
                          minWidth: 50,
                        }}
                      >
                        {row[column] !== null && row[column] !== undefined ? (
                          String(row[column])
                        ) : (
                          <span className="text-muted-foreground italic">NULL</span>
                        )}
                      </td>
                    ))}
                  </tr>
                </ContextMenuTrigger>
                <ContextMenuContent>
                  <ContextMenuItem>
                    <Copy className="w-4 h-4 mr-2" />
                    Copy
                  </ContextMenuItem>
                  <ContextMenuItem>
                    <TableIcon className="w-4 h-4 mr-2" />
                    Copy Row
                  </ContextMenuItem>
                  <ContextMenuSeparator />
                  <ContextMenuSub>
                    <ContextMenuSubTrigger>
                      <Files className="w-4 h-4 mr-2" />
                      Copy as
                    </ContextMenuSubTrigger>
                    <ContextMenuSubContent>
                      <ContextMenuItem>Copy as CSV</ContextMenuItem>
                      <ContextMenuItem>Copy as Insert SQL</ContextMenuItem>
                      <ContextMenuItem>Copy as Update SQL</ContextMenuItem>
                    </ContextMenuSubContent>
                  </ContextMenuSub>
                </ContextMenuContent>
              </ContextMenu>
            ))}
          </tbody>
        </table>
      </div>

      <div className="flex items-center justify-between px-4 py-1 border-t border-border bg-muted/40">
        <div className="text-sm text-muted-foreground">
          Query executed in{" "}
          {executionTimeMs ? (executionTimeMs / 1000).toFixed(3) : "0.000"}s •{" "}
          {filteredData.length} rows returned
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
      <Dialog open={isDDLModalOpen} onOpenChange={setIsDDLModalOpen}>
        <DialogContent className="max-w-4xl h-[80vh] flex flex-col p-0 gap-0">
          <DialogHeader className="px-4 py-3 border-b border-border">
            <DialogTitle>Table Structure: {tableContext?.table}</DialogTitle>
          </DialogHeader>
          <div className="flex-1 min-h-0 relative">
            <Editor
              height="100%"
              defaultLanguage="sql"
              value={isLoadingDDL ? "-- Loading..." : ddlContent}
              options={{
                readOnly: true,
                minimap: { enabled: false },
                fontSize: 13,
                scrollBeyondLastLine: false,
                automaticLayout: true,
              }}
            />
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
