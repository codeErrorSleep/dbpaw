import { useState, useMemo, useCallback, useRef, useEffect } from "react";
import CodeMirror, { Extension } from "@uiw/react-codemirror";
import { sql, PostgreSQL, MySQL, SQLite, StandardSQL, SQLNamespace } from "@codemirror/lang-sql";
import { oneDark } from "@codemirror/theme-one-dark";
import { keymap } from "@codemirror/view";
import { CompletionContext } from "@codemirror/autocomplete";
import { linter, lintGutter, Diagnostic } from "@codemirror/lint";
import { syntaxTree } from "@codemirror/language";
import { Button } from "@/components/ui/button";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { Play, Save, Trash2, Clock, Database, Braces } from "lucide-react";
import { TableView } from "@/components/business/DataGrid/TableView";
import { useTheme } from "@/components/theme-provider";
import { SchemaOverview } from "@/services/api";
import { format } from "sql-formatter";

interface SqlEditorProps {
  queryResults?: {
    data: any[];
    columns: string[];
    executionTime?: string;
  } | null;
  onExecute?: (sql: string) => void;
  onCancel?: () => void;
  databaseName?: string;
  value?: string;
  onChange?: (value: string) => void;
  connectionId?: number;
  driver?: string;
  schemaOverview?: SchemaOverview;
}

export function SqlEditor({
  queryResults,
  onExecute,
  onCancel,
  databaseName,
  value,
  onChange,
  connectionId,
  driver,
  schemaOverview,
}: SqlEditorProps) {
  const [internalSql, setInternalSql] = useState("-- Enter your SQL query here\n");
  const { theme } = useTheme();
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  
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
  const handleSqlChange = useCallback((val: string) => {
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
  }, [onChange, value]);

  const handleExecute = useCallback(() => {
    if (onExecute) {
      onExecute(code);
    }
  }, [onExecute, code]);

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

  // Determine Dialect
  const dialect = useMemo(() => {
    switch (driver) {
      case "postgres": return PostgreSQL;
      case "mysql": return MySQL;
      case "sqlite": return SQLite;
      default: return StandardSQL;
    }
  }, [driver]);

  // Build Schema for CodeMirror
  const sqlSchema = useMemo(() => {
    if (!schemaOverview) {
      return {};
    }
    
    const schemaMap: SQLNamespace = {};
    
    schemaOverview.tables.forEach(t => {
       const colNames = t.columns.map(c => c.name);
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
    const options = schemaOverview.tables.flatMap(t => 
      t.columns.map(c => ({
        label: c.name,
        type: "property", // Icon type
        detail: t.name,   // Show table name as detail
        boost: -1         // Lower priority than keywords/tables usually, but available
      }))
    );

    // Add tables as well for quick access without context
    const tableOptions = schemaOverview.tables.map(t => ({
      label: t.name,
      type: "class",
      detail: t.schema || "table",
      boost: 0
    }));

    const allOptions = [...options, ...tableOptions];

    return (context: CompletionContext) => {
      let word = context.matchBefore(/[\w\.]*/);
      if (!word || (word.from === word.to && !context.explicit)) return null;
      
      // If typing after a dot, let the default SQL completer handle it (it's context aware)
      if (word.text.includes(".")) return null;

      return {
        from: word.from,
        options: allOptions
      };
    };
  }, [schemaOverview]);

  // Extensions
  const extensions = useMemo(() => {
    const exts: Extension[] = [
      sql({
        dialect,
        schema: sqlSchema,
        upperCaseKeywords: true,
      }),
      keymap.of([
        {
          key: "Mod-Enter",
          run: () => {
            handleExecute();
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
      ]),
    ];

    // Inject global completion if available
    if (globalCompletion) {
      exts.push(dialect.language.data.of({
        autocomplete: globalCompletion
      }));
    }

    return exts;
  }, [dialect, sqlSchema, handleExecute, handleFormat, globalCompletion]);

  // Theme
  const editorTheme = useMemo(() => {
      const isDark = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
      return isDark ? oneDark : [];
  }, [theme]);

  return (
    <div className="h-full flex flex-col bg-background">
      <div className="flex items-center justify-between px-4 py-2 border-b border-border">
          <div className="flex items-center gap-2">
          {databaseName && (
            <div className="flex items-center gap-2 px-3 py-1 bg-muted/50 rounded text-xs text-muted-foreground border border-border">
              <Database className={`w-3 h-3 ${schemaOverview ? "text-green-500" : "text-muted-foreground"}`} />
              <span>{databaseName}</span>
            </div>
          )}

          <div className="w-px h-4 bg-border mx-2" />

          <Button
            onClick={handleExecute}
            size="sm"
            className="gap-2"
            title="Run SQL (Cmd+Enter)"
          >
            <Play className="w-4 h-4" />
            Run
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="gap-2"
            onClick={handleFormat}
            title="Format SQL (Shift+Alt+F)"
          >
            <Braces className="w-4 h-4" />
            Format
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="gap-2"
            onClick={onCancel}
          >
            Cancel
          </Button>
          <Button variant="outline" size="sm" className="gap-2">
            <Save className="w-4 h-4" />
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="gap-2"
            onClick={handleClear}
          >
            <Trash2 className="w-4 h-4" />
          </Button>
        </div>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Clock className="w-4 h-4" />
          <span>
            {queryResults?.executionTime
              ? `Last executed: ${queryResults.executionTime}`
              : "Ready"}
          </span>
          <div className="w-px h-3 bg-border mx-2" />
          <span className="text-xs">
              {schemaOverview ? "Schema Loaded" : "Loading Schema..."}
          </span>
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
                  <div className="px-4 py-2 border-b border-border bg-muted/40">
                    <span className="text-sm font-semibold text-foreground">
                      Query Results ({queryResults.data.length} rows)
                    </span>
                  </div>
                  <div className="flex-1 overflow-hidden">
                    <TableView
                      data={queryResults.data}
                      columns={queryResults.columns}
                      hideHeader
                    />
                  </div>
                </div>
              </ResizablePanel>
            </>
          )}
        </ResizablePanelGroup>
      </div>
    </div>
  );
}
