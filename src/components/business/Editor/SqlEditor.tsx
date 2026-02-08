import { useState } from "react";
import Editor from "@monaco-editor/react";
import { Button } from "@/components/ui/button";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { Play, Save, Trash2, Clock, Database } from "lucide-react";
import { TableView } from "@/components/business/DataGrid/TableView";
import { useTheme } from "@/components/theme-provider";

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
}

export function SqlEditor({
  queryResults,
  onExecute,
  onCancel,
  databaseName,
  value,
  onChange,
}: SqlEditorProps) {
  const [internalSql, setInternalSql] = useState("-- Enter your SQL query here\n");
  const { theme } = useTheme();

  // Use controlled value if provided, otherwise internal state
  const sql = value !== undefined ? value : internalSql;

  // Determine Monaco theme based on app theme
  const monacoTheme = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches) ? "vs-dark" : "light";

  const handleSqlChange = (val: string | undefined) => {
    const newValue = val || "";
    if (onChange) {
      onChange(newValue);
    } else {
      setInternalSql(newValue);
    }
  };

  const handleExecute = () => {
    if (onExecute) {
      onExecute(sql);
    }
  };

  const handleClear = () => {
    handleSqlChange("");
  };

  return (
    <div className="h-full flex flex-col bg-background">
      <div className="flex items-center justify-between px-4 py-2 border-b border-border">
        <div className="flex items-center gap-2">
          {databaseName && (
            <div className="flex items-center gap-2 px-3 py-1 bg-muted/50 rounded text-xs text-muted-foreground border border-border">
              <Database className="w-3 h-3" />
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
        </div>
      </div>

      <div className="flex-1 overflow-hidden">
        <ResizablePanelGroup direction="vertical">
          {/* SQL Editor Panel */}
          <ResizablePanel defaultSize={queryResults ? 50 : 100} minSize={30}>
            <div className="h-full flex flex-col">
              <div className="flex-1 relative">
                <Editor
                  height="100%"
                  defaultLanguage="sql"
                  value={sql}
                  onChange={handleSqlChange}
                  theme={monacoTheme}
                  options={{
                    minimap: { enabled: false },
                    fontSize: 14,
                    lineNumbers: "on",
                    roundedSelection: false,
                    scrollBeyondLastLine: false,
                    automaticLayout: true,
                    tabSize: 2,
                    wordWrap: "on",
                  }}
                />
              </div>
            </div>
          </ResizablePanel>

          {/* Results Panel */}
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
