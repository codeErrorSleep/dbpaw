import { useState, useMemo, useCallback, useRef, useEffect } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import CodeMirror, { Extension } from "@uiw/react-codemirror";
import {
  sql,
  PostgreSQL,
  MySQL,
  SQLite,
  StandardSQL,
  SQLNamespace,
} from "@codemirror/lang-sql";
import { oneDark } from "@codemirror/theme-one-dark";
import { keymap, EditorView } from "@codemirror/view";
import { CompletionContext, acceptCompletion } from "@codemirror/autocomplete";
import { Prec } from "@codemirror/state";
import { insertTab } from "@codemirror/commands";
import { Button } from "@/components/ui/button";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import {
  Play,
  Save,
  Trash2,
  Database,
  Braces,
  Download,
  CheckCircle2,
  XCircle,
} from "lucide-react";
import { TableView } from "@/components/business/DataGrid/TableView";
import { useTheme } from "@/components/theme-provider";
import {
  SchemaOverview,
  api,
  SavedQuery,
  TransferFormat,
  isTauri,
} from "@/services/api";
import { format } from "sql-formatter";
import { SaveQueryDialog } from "./SaveQueryDialog";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { toast } from "sonner";
import { sqlEditorThemeDark, sqlEditorThemeLight } from "./codemirrorTheme";
import { getThemePreset } from "@/theme/themeRegistry";

const editorFontSizeExtension = EditorView.theme({
  ".cm-scroller": {
    fontSize: "1rem",
  },
});

interface SqlEditorProps {
  queryResults?: {
    data: any[];
    columns: string[];
    executionTime?: string;
    error?: string;
  } | null;
  onExecute?: (sql: string) => void;
  onCancel?: () => void;
  databaseName?: string;
  value?: string;
  onChange?: (value: string) => void;
  connectionId?: number;
  driver?: string;
  schemaOverview?: SchemaOverview;
  savedQueryId?: number;
  initialName?: string;
  initialDescription?: string;
  onSaveSuccess?: (savedQuery: SavedQuery) => void;
}

export function SqlEditor({
  queryResults,
  onExecute,
  onCancel,
  databaseName,
  value,
  onChange,
  connectionId: _connectionId,
  driver,
  schemaOverview,
  savedQueryId,
  initialName,
  initialDescription,
  onSaveSuccess,
}: SqlEditorProps) {
  const [internalSql, setInternalSql] = useState("");
  const { theme } = useTheme();
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [isSaveDialogOpen, setIsSaveDialogOpen] = useState(false);
  const resultStatus = useMemo(() => {
    if (!queryResults) return null;
    if (queryResults.error) {
      return {
        text: "Result: Execution failed.",
        toneClass: "text-destructive",
        Icon: XCircle,
      };
    }

    const returnedRows = queryResults.data.length;
    const hasResultSet = queryResults.columns.length > 0;
    const suffix = hasResultSet
      ? ` (${returnedRows} row${returnedRows === 1 ? "" : "s"})`
      : "";

    return {
      text: `Result: Execution successful.${suffix}`,
      toneClass: "text-emerald-600 dark:text-emerald-400",
      Icon: CheckCircle2,
    };
  }, [queryResults]);

  // Use controlled value if provided, otherwise internal state
  const code = value !== undefined ? value : internalSql;

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  // Debounce onChange to prevent excessive parent re-renders
  const handleSqlChange = useCallback(
    (val: string) => {
      // Always update internal state immediately if we are using it
      if (value === undefined) {
        setInternalSql(val);
      }

      // Clear previous timeout
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }

      // Debounce the callback to parent
      timeoutRef.current = setTimeout(() => {
        if (onChange) {
          onChange(val);
        }
      }, 300);
    },
    [onChange, value],
  );

  const handleExecute = useCallback(() => {
    if (onExecute) {
      onExecute(code);
    }
  }, [onExecute, code]);

  const executeFromEditorSelection = useCallback(
    (view: EditorView) => {
      if (!onExecute) {
        return;
      }

      const selectedSql = view.state.selection.ranges
        .map((range) => view.state.sliceDoc(range.from, range.to))
        .filter((text) => text.trim().length > 0)
        .join("\n");

      onExecute(selectedSql || view.state.doc.toString());
    },
    [onExecute],
  );

  const handleClear = () => {
    handleSqlChange("");
  };

  const handleFormat = useCallback(() => {
    try {
      const dialectMap: Record<string, string> = {
        postgres: "postgresql",
        postgresql: "postgresql",
        mysql: "mysql",
        sqlite: "sqlite",
        clickhouse: "sql",
        mssql: "transactsql",
      };
      const language = ((driver && dialectMap[driver]) || "sql") as any;
      const formatted = format(code, {
        language,
        keywordCase: "upper",
        tabWidth: 2,
      });
      handleSqlChange(formatted);
    } catch (e) {
      console.error("Format failed:", e);
    }
  }, [code, driver, handleSqlChange]);

  const savedQueryIdRef = useRef(savedQueryId);
  useEffect(() => {
    savedQueryIdRef.current = savedQueryId;
  }, [savedQueryId]);

  const executeSave = useCallback(
    async (name: string, description: string) => {
      try {
        const currentId = savedQueryIdRef.current;
        let result: SavedQuery;
        if (currentId) {
          result = await api.queries.update(currentId, {
            name,
            description,
            query: code,
            connectionId: _connectionId || undefined,
            database: databaseName,
          });
        } else {
          result = await api.queries.create({
            name,
            description,
            query: code,
            connectionId: _connectionId || undefined,
            database: databaseName,
          });
        }
        if (onSaveSuccess) {
          onSaveSuccess(result);
        }
      } catch (e) {
        console.error("Failed to save query", e);
      }
    },
    [code, _connectionId, databaseName, onSaveSuccess],
  );

  const handleSave = async (name: string, description: string) => {
    await executeSave(name, description);
  };

  const handleExportResult = useCallback(
    async (format: TransferFormat) => {
      if (!_connectionId) {
        toast.error("Please run query with a saved connection to export.");
        return;
      }
      if (!isTauri()) {
        toast.error("Export dialog is only available in Tauri desktop mode.");
        return;
      }

      const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
      const defaultPath = `query_result_${timestamp}.${format}`;
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

      try {
        const result = await api.transfer.exportQueryResult({
          id: _connectionId,
          database: databaseName,
          sql: code,
          driver: driver || "postgres",
          format,
          filePath,
        });
        toast.success(`Export completed (${result.rowCount} rows)`, {
          description: result.filePath,
        });
      } catch (e) {
        toast.error("Export failed", {
          description: e instanceof Error ? e.message : String(e),
        });
      }
    },
    [_connectionId, databaseName, code, driver],
  );

  const triggerSave = useCallback(() => {
    const currentId = savedQueryIdRef.current;
    if (currentId) {
      executeSave(initialName || "Untitled", initialDescription || "");
    } else {
      setIsSaveDialogOpen(true);
    }
  }, [initialName, initialDescription, executeSave]);

  // Determine Dialect
  const dialect = useMemo(() => {
    switch (driver) {
      case "postgres":
        return PostgreSQL;
      case "mysql":
        return MySQL;
      case "sqlite":
        return SQLite;
      case "clickhouse":
        return StandardSQL;
      case "mssql":
        return StandardSQL;
      default:
        return StandardSQL;
    }
  }, [driver]);

  // Build Schema for CodeMirror
  const sqlSchema = useMemo(() => {
    if (!schemaOverview) {
      return {};
    }

    const schemaMap: SQLNamespace = {};

    schemaOverview.tables.forEach((t) => {
      const colNames = t.columns.map((c) => c.name);
      // Add table
      schemaMap[t.name] = colNames;
      // Add schema.table
      if (t.schema) {
        schemaMap[`${t.schema}.${t.name}`] = colNames;
      }
    });

    return schemaMap;
  }, [schemaOverview]);

  // Create a custom completion source for global column suggestions
  const globalCompletion = useMemo(() => {
    if (!schemaOverview) return null;

    // Flatten all columns from all tables
    const options = schemaOverview.tables.flatMap((t) =>
      t.columns.map((c) => ({
        label: c.name,
        type: "property", // Icon type
        detail: t.name, // Show table name as detail
        boost: -1, // Lower priority than keywords/tables usually, but available
      })),
    );

    // Add tables as well for quick access without context
    const tableOptions = schemaOverview.tables.map((t) => ({
      label: t.name,
      type: "class",
      detail: t.schema || "table",
      boost: 0,
    }));

    const allOptions = [...options, ...tableOptions];

    return (context: CompletionContext) => {
      let word = context.matchBefore(/[\w\.]*/);
      if (!word || (word.from === word.to && !context.explicit)) return null;

      // If typing after a dot, let the default SQL completer handle it (it's context aware)
      if (word.text.includes(".")) return null;

      return {
        from: word.from,
        options: allOptions,
      };
    };
  }, [schemaOverview]);

  // Extensions
  const extensions = useMemo(() => {
    const exts: Extension[] = [
      EditorView.lineWrapping,
      editorFontSizeExtension,
      sql({
        dialect,
        schema: sqlSchema,
        upperCaseKeywords: true,
      }),
      Prec.high(
        keymap.of([
          {
            key: "Tab",
            run: (view) => acceptCompletion(view) || insertTab(view),
          },
          {
            key: "Mod-Enter",
            run: (view) => {
              executeFromEditorSelection(view);
              return true;
            },
          },
          {
            key: "Shift-Alt-f",
            run: () => {
              handleFormat();
              return true;
            },
          },
          {
            key: "Mod-s",
            run: () => {
              triggerSave();
              return true;
            },
          },
        ]),
      ),
    ];

    // Inject global completion if available
    if (globalCompletion) {
      exts.push(
        dialect.language.data.of({
          autocomplete: globalCompletion,
        }),
      );
    }

    return exts;
  }, [
    dialect,
    sqlSchema,
    executeFromEditorSelection,
    handleFormat,
    globalCompletion,
    triggerSave,
  ]);

  // Theme
  const editorTheme = useMemo(() => {
    const preset = getThemePreset(theme);
    const syntaxTheme =
      preset.editorTheme === "one-dark" ? [oneDark] : [];
    return preset.appearance === "dark"
      ? [...syntaxTheme, sqlEditorThemeDark]
      : [...syntaxTheme, sqlEditorThemeLight];
  }, [theme]);

  return (
    <div className="h-full flex flex-col bg-background">
      <div className="flex items-center justify-between px-4 py-2 border-b border-border">
        <div className="flex items-center gap-2">
          {databaseName && (
            <div className="flex items-center gap-2 px-3 py-1 bg-muted/50 rounded text-xs text-muted-foreground border border-border">
              <Database
                className={`w-3 h-3 ${schemaOverview ? "text-green-500" : "text-muted-foreground"}`}
              />
              <span>{databaseName}</span>
              {savedQueryId && (
                <span className="text-[10px] opacity-50 ml-1">
                  #{savedQueryId}
                </span>
              )}
            </div>
          )}

          <div className="w-px h-4 bg-border mx-2" />

          <TooltipProvider>
            <div className="flex items-center gap-1">
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    onClick={handleExecute}
                    size="icon"
                    variant="outline"
                    className="h-8 w-8"
                  >
                    <Play className="w-4 h-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  <p>Run SQL (Cmd/Ctrl+Enter)</p>
                </TooltipContent>
              </Tooltip>

              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="outline"
                    size="icon"
                    className="h-8 w-8"
                    onClick={handleFormat}
                  >
                    <Braces className="w-4 h-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  <p>Format SQL (Shift+Alt+F)</p>
                </TooltipContent>
              </Tooltip>

              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="outline"
                    size="icon"
                    className="h-8 w-8"
                    onClick={onCancel}
                  >
                    <span className="h-3 w-3 bg-foreground/80 rounded-[1px]" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  <p>Cancel Query</p>
                </TooltipContent>
              </Tooltip>

              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="outline"
                    size="icon"
                    className="h-8 w-8"
                    onClick={triggerSave}
                  >
                    <Save className="w-4 h-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  <p>Save Query (Cmd/Ctrl+S)</p>
                </TooltipContent>
              </Tooltip>

              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="outline"
                    size="icon"
                    className="h-8 w-8"
                    onClick={handleClear}
                  >
                    <Trash2 className="w-4 h-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  <p>Clear Editor</p>
                </TooltipContent>
              </Tooltip>
            </div>
          </TooltipProvider>
        </div>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          {resultStatus && (
            <>
              <span
                className={`text-xs inline-flex items-center gap-1 ${resultStatus.toneClass}`}
              >
                <resultStatus.Icon className="w-3.5 h-3.5" />
                {resultStatus.text}
              </span>
            </>
          )}
          {queryResults && !queryResults.error && (
            <>
              <div className="w-px h-3 bg-border mx-2" />
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline" size="sm" className="h-8 gap-1.5">
                    <Download className="w-4 h-4" />
                    Export Result
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem
                    onClick={() => void handleExportResult("csv")}
                  >
                    CSV
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onClick={() => void handleExportResult("json")}
                  >
                    JSON
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onClick={() => void handleExportResult("sql")}
                  >
                    SQL
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </>
          )}
        </div>
      </div>

      <div className="flex-1 overflow-hidden">
        <ResizablePanelGroup direction="vertical">
          <ResizablePanel defaultSize={queryResults ? 50 : 100} minSize={30}>
            <div className="h-full flex flex-col text-base">
              <CodeMirror
                value={code}
                height="100%"
                extensions={extensions}
                theme={editorTheme}
                onChange={handleSqlChange}
                className="h-full"
                basicSetup={{
                  lineNumbers: true,
                  foldGutter: true,
                  dropCursor: true,
                  allowMultipleSelections: true,
                  indentOnInput: true,
                  autocompletion: true,
                }}
              />
            </div>
          </ResizablePanel>

          {queryResults && (
            <>
              <ResizableHandle withHandle />
              <ResizablePanel defaultSize={50} minSize={20}>
                <div className="h-full flex flex-col">
                  {queryResults.error ? (
                    <div className="h-full p-4 bg-destructive/10 text-destructive overflow-auto font-mono text-sm whitespace-pre-wrap">
                      <div className="font-bold mb-2">
                        Error executing query:
                      </div>
                      {queryResults.error}
                    </div>
                  ) : (
                    <div className="flex-1 overflow-hidden">
                      <TableView
                        data={queryResults.data}
                        columns={queryResults.columns}
                        hideHeader
                      />
                    </div>
                  )}
                </div>
              </ResizablePanel>
            </>
          )}
        </ResizablePanelGroup>
      </div>

      <SaveQueryDialog
        open={isSaveDialogOpen}
        onOpenChange={setIsSaveDialogOpen}
        onSave={handleSave}
        initialName={initialName}
        initialDescription={initialDescription}
      />
    </div>
  );
}
