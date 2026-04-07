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
import {
  CompletionContext,
  CompletionResult,
  acceptCompletion,
} from "@codemirror/autocomplete";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { Prec } from "@codemirror/state";
import { insertTab } from "@codemirror/commands";
import { tags as t } from "@lezer/highlight";
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
  Loader2,
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { toast } from "sonner";
import { sqlEditorThemeDark, sqlEditorThemeLight } from "./codemirrorTheme";
import { collectSelectedSql } from "./sqlSelection";
import { getThemePreset, type ThemeId } from "@/theme/themeRegistry";
import { CLICKHOUSE_COMPLETIONS } from "./clickhouseKeywords";
import { useTranslation } from "react-i18next";

const editorFontSizeExtension = EditorView.theme({
  ".cm-scroller": {
    fontSize: "1rem",
  },
});

type SqlSyntaxPalette = {
  keyword: string;
  function: string;
  type: string;
  string: string;
  number: string;
  variable: string;
  operator: string;
  comment: string;
  constant: string;
};

const createSqlSyntaxTheme = (palette: SqlSyntaxPalette): Extension[] => [
  syntaxHighlighting(
    HighlightStyle.define([
      { tag: t.keyword, color: palette.keyword },
      { tag: t.operatorKeyword, color: palette.keyword },
      { tag: t.typeName, color: palette.type },
      { tag: t.className, color: palette.type },
      { tag: t.function(t.variableName), color: palette.function },
      { tag: t.function(t.propertyName), color: palette.function },
      { tag: t.propertyName, color: palette.variable },
      { tag: t.variableName, color: palette.variable },
      { tag: t.string, color: palette.string },
      { tag: t.special(t.string), color: palette.string },
      { tag: t.number, color: palette.number },
      { tag: t.bool, color: palette.constant },
      { tag: t.atom, color: palette.constant },
      { tag: t.operator, color: palette.operator },
      { tag: t.comment, color: palette.comment, fontStyle: "italic" },
    ]),
  ),
];

const SQL_SYNTAX_THEME_MAP: Record<ThemeId, Extension[]> = {
  default: [],
  "one-dark": [oneDark],
  "github-light": createSqlSyntaxTheme({
    keyword: "#cf222e",
    function: "#8250df",
    type: "#0550ae",
    string: "#0a3069",
    number: "#0550ae",
    variable: "#24292f",
    operator: "#57606a",
    comment: "#6e7781",
    constant: "#953800",
  }),
  "github-dark": createSqlSyntaxTheme({
    keyword: "#ff7b72",
    function: "#d2a8ff",
    type: "#79c0ff",
    string: "#a5d6ff",
    number: "#79c0ff",
    variable: "#c9d1d9",
    operator: "#8b949e",
    comment: "#8b949e",
    constant: "#ffa657",
  }),
  "monokai-pro": createSqlSyntaxTheme({
    keyword: "#ff6188",
    function: "#a9dc76",
    type: "#78dce8",
    string: "#ffd866",
    number: "#ab9df2",
    variable: "#fcfcfa",
    operator: "#f8f8f2",
    comment: "#939293",
    constant: "#fc9867",
  }),
  "night-owl": createSqlSyntaxTheme({
    keyword: "#c792ea",
    function: "#82aaff",
    type: "#7fdbca",
    string: "#ecc48d",
    number: "#f78c6c",
    variable: "#d6deeb",
    operator: "#7fdbca",
    comment: "#637777",
    constant: "#ff5874",
  }),
  "shades-of-purple": createSqlSyntaxTheme({
    keyword: "#ff9d00",
    function: "#b362ff",
    type: "#9effff",
    string: "#a5ff90",
    number: "#ff628c",
    variable: "#ffffff",
    operator: "#ff9d00",
    comment: "#b9b4f5",
    constant: "#fad000",
  }),
  palenight: createSqlSyntaxTheme({
    keyword: "#c792ea",
    function: "#82aaff",
    type: "#89ddff",
    string: "#c3e88d",
    number: "#f78c6c",
    variable: "#c7cbe6",
    operator: "#89ddff",
    comment: "#7f85a3",
    constant: "#ffcb6b",
  }),
  cyberpunk: createSqlSyntaxTheme({
    keyword: "#ff2bd6",
    function: "#00f5ff",
    type: "#7df9ff",
    string: "#ffe66d",
    number: "#ff8fab",
    variable: "#f8f7ff",
    operator: "#00f5ff",
    comment: "#9f88c5",
    constant: "#faff00",
  }),
  nord: createSqlSyntaxTheme({
    keyword: "#81a1c1",
    function: "#88c0d0",
    type: "#8fbcbb",
    string: "#a3be8c",
    number: "#b48ead",
    variable: "#eceff4",
    operator: "#d8dee9",
    comment: "#616e88",
    constant: "#ebcb8b",
  }),
  dracula: createSqlSyntaxTheme({
    keyword: "#ff79c6",
    function: "#50fa7b",
    type: "#8be9fd",
    string: "#f1fa8c",
    number: "#bd93f9",
    variable: "#f8f8f2",
    operator: "#ff79c6",
    comment: "#6272a4",
    constant: "#ffb86c",
  }),
};

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
  availableDatabases?: string[];
  value?: string;
  onChange?: (value: string) => void;
  onDatabaseChange?: (database: string) => void;
  connectionId?: number;
  driver?: string;
  schemaOverview?: SchemaOverview;
  savedQueryId?: number;
  initialName?: string;
  initialDescription?: string;
  onSaveSuccess?: (savedQuery: SavedQuery) => void;
  isExecuting?: boolean;
}

export function SqlEditor({
  queryResults,
  onExecute,
  onCancel,
  databaseName,
  availableDatabases,
  value,
  onChange,
  onDatabaseChange,
  connectionId: _connectionId,
  driver,
  schemaOverview,
  savedQueryId,
  initialName,
  initialDescription,
  onSaveSuccess,
  isExecuting,
}: SqlEditorProps) {
  const { t } = useTranslation();
  const [internalSql, setInternalSql] = useState("");
  const { theme } = useTheme();
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [isSaveDialogOpen, setIsSaveDialogOpen] = useState(false);
  const [isFormatting, setIsFormatting] = useState(false);
  const canSwitchDatabase =
    !!databaseName &&
    !!onDatabaseChange &&
    !!availableDatabases &&
    availableDatabases.length > 1;
  const resultStatus = useMemo(() => {
    if (!queryResults) return null;
    if (queryResults.error) {
      return {
        text: t("sqlEditor.result.failed"),
        toneClass: "text-destructive",
        Icon: XCircle,
      };
    }

    const returnedRows = queryResults.data.length;
    const hasResultSet = queryResults.columns.length > 0;
    const suffix = hasResultSet
      ? returnedRows === 1
        ? t("sqlEditor.result.rowsSuffix", { count: returnedRows })
        : t("sqlEditor.result.rowsSuffixPlural", { count: returnedRows })
      : "";

    return {
      text: `${t("sqlEditor.result.success")}${suffix}`,
      toneClass: "text-emerald-600 dark:text-emerald-400",
      Icon: CheckCircle2,
    };
  }, [queryResults, t]);

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

  const executeFromEditorSelection = useCallback(
    (view: EditorView) => {
      if (!onExecute) {
        return;
      }
      const sqlToRun = collectSelectedSql({
        ranges: view.state.selection.ranges,
        sliceDoc: (from, to) => view.state.sliceDoc(from, to),
        fullDoc: () => view.state.doc.toString(),
      });

      onExecute(sqlToRun);
    },
    [onExecute],
  );

  const handleExecute = useCallback(() => {
    if (!onExecute) return;
    const view = editorViewRef.current;
    if (view) {
      executeFromEditorSelection(view);
      return;
    }
    onExecute(code);
  }, [onExecute, code, executeFromEditorSelection]);

  const handleClear = () => {
    handleSqlChange("");
  };

  const handleFormat = useCallback(async () => {
    if (isFormatting) return;

    setIsFormatting(true);
    try {
      const { format } = await import("sql-formatter");
      const dialectMap: Record<string, string> = {
        postgres: "postgresql",
        postgresql: "postgresql",
        mysql: "mysql",
        tidb: "mysql",
        mariadb: "mysql",
        sqlite: "sqlite",
        duckdb: "sqlite",
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
      toast.error(t("sqlEditor.error.formatFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsFormatting(false);
    }
  }, [code, driver, handleSqlChange, isFormatting, t]);

  const savedQueryIdRef = useRef(savedQueryId);
  const editorViewRef = useRef<EditorView | null>(null);
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
        toast.error(t("sqlEditor.export.runWithSavedConnection"));
        return;
      }
      if (!isTauri()) {
        toast.error(t("sqlEditor.export.desktopOnly"));
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
          title: t("sqlEditor.export.saveFileTitle"),
          defaultPath,
          filters,
        });
        if (!selected) return;
        filePath = Array.isArray(selected) ? selected[0] : selected;
        if (!filePath) return;
      } catch (e) {
        toast.error(t("sqlEditor.export.openSaveDialogFailed"), {
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
        toast.success(
          t("sqlEditor.export.completed", { count: result.rowCount }),
          {
            description: result.filePath,
          },
        );
      } catch (e) {
        toast.error(t("sqlEditor.export.failed"), {
          description: e instanceof Error ? e.message : String(e),
        });
      }
    },
    [_connectionId, databaseName, code, driver, t],
  );

  const triggerSave = useCallback(() => {
    const currentId = savedQueryIdRef.current;
    if (currentId) {
      executeSave(
        initialName || t("sqlEditor.untitled"),
        initialDescription || "",
      );
    } else {
      setIsSaveDialogOpen(true);
    }
  }, [initialName, initialDescription, executeSave, t]);

  // Determine Dialect
  const dialect = useMemo(() => {
    switch (driver) {
      case "postgres":
        return PostgreSQL;
      case "mysql":
      case "tidb":
      case "mariadb":
        return MySQL;
      case "sqlite":
      case "duckdb":
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

  const clickhouseCompletion = useMemo(() => {
    if (driver !== "clickhouse") return null;

    return (context: CompletionContext): CompletionResult | null => {
      const word = context.matchBefore(/[\w\s]*/);
      if (!word || (word.from === word.to && !context.explicit)) return null;

      return {
        from: word.from,
        options: CLICKHOUSE_COMPLETIONS,
      };
    };
  }, [driver]);

  const mergedCompletion = useMemo(() => {
    if (!globalCompletion && !clickhouseCompletion) return null;

    return (context: CompletionContext): CompletionResult | null => {
      const results = [globalCompletion, clickhouseCompletion]
        .map((provider) => provider?.(context))
        .filter((item): item is CompletionResult => !!item);
      if (!results.length) return null;

      const from = results.reduce(
        (min, curr) => Math.min(min, curr.from),
        results[0].from,
      );
      const options: NonNullable<CompletionResult["options"]>[number][] = [];
      const seen = new Set<string>();
      for (const result of results) {
        for (const option of result.options) {
          const key = `${option.label}::${option.type ?? ""}`;
          if (seen.has(key)) continue;
          seen.add(key);
          options.push(option);
        }
      }
      return { from, options };
    };
  }, [globalCompletion, clickhouseCompletion]);

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
              void handleFormat();
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
    if (mergedCompletion) {
      exts.push(
        dialect.language.data.of({
          autocomplete: mergedCompletion,
        }),
      );
    }

    return exts;
  }, [
    dialect,
    sqlSchema,
    executeFromEditorSelection,
    handleFormat,
    mergedCompletion,
    triggerSave,
  ]);

  // Theme
  const editorTheme = useMemo(() => {
    const preset = getThemePreset(theme);
    const syntaxTheme = SQL_SYNTAX_THEME_MAP[preset.editorTheme] ?? [];
    return preset.appearance === "dark"
      ? [...syntaxTheme, sqlEditorThemeDark]
      : [...syntaxTheme, sqlEditorThemeLight];
  }, [theme]);

  return (
    <div className="h-full flex flex-col bg-background">
      <div className="flex items-center justify-between px-4 py-2 border-b border-border">
        <div className="flex items-center gap-2">
          {databaseName &&
            (canSwitchDatabase ? (
              <div className="flex items-center gap-2">
                <Database
                  className={`w-3 h-3 ${schemaOverview ? "text-green-500" : "text-muted-foreground"}`}
                />
                <Select value={databaseName} onValueChange={onDatabaseChange}>
                  <SelectTrigger
                    size="sm"
                    className="h-8 min-w-[180px] bg-muted/50 text-xs"
                    aria-label={t("sqlEditor.database.ariaLabel")}
                  >
                    <SelectValue
                      placeholder={t("sqlEditor.database.placeholder")}
                    />
                  </SelectTrigger>
                  <SelectContent>
                    {availableDatabases?.map((database) => (
                      <SelectItem key={database} value={database}>
                        {database}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                {savedQueryId && (
                  <span className="text-[10px] opacity-50">
                    #{savedQueryId}
                  </span>
                )}
              </div>
            ) : (
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
            ))}

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
                    disabled={isExecuting}
                  >
                    {isExecuting ? (
                      <Loader2 className="w-4 h-4 animate-spin" />
                    ) : (
                      <Play className="w-4 h-4" />
                    )}
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  <p>{t("sqlEditor.tooltip.runSql")}</p>
                </TooltipContent>
              </Tooltip>

              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="outline"
                    size="icon"
                    className="h-8 w-8"
                    onClick={() => void handleFormat()}
                    disabled={isFormatting}
                  >
                    <Braces className="w-4 h-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  <p>{t("sqlEditor.tooltip.formatSql")}</p>
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
                  <p>{t("sqlEditor.tooltip.cancelQuery")}</p>
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
                  <p>{t("sqlEditor.tooltip.saveQuery")}</p>
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
                  <p>{t("sqlEditor.tooltip.clearEditor")}</p>
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
                    {t("sqlEditor.export.result")}
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
                onCreateEditor={(view) => {
                  editorViewRef.current = view;
                }}
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
                        {t("sqlEditor.error.executingQuery")}
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
