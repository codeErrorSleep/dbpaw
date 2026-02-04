import { useState } from "react";
import Editor from "@monaco-editor/react";
import { Button } from "@/components/ui/button";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { Play, Square, Save, Download, Eraser, Trash2, Clock } from "lucide-react";
import { TableView } from "@/components/business/DataGrid/TableView";

interface SqlEditorProps {
  queryResults?: {
    data: any[];
    columns: string[];
    executionTime?: string;
  } | null;
  onExecute?: (sql: string) => void;
  onCancel?: () => void;
}

export function SqlEditor({
  queryResults,
  onExecute,
  onCancel,
}: SqlEditorProps) {
  const [sql, setSql] = useState(
    `-- Select all users\nSELECT id, email, name, created_at\nFROM users\nWHERE created_at > '2024-01-01'\nORDER BY created_at DESC\nLIMIT 100;`,
  );

  const handleExecute = () => {
    if (onExecute) {
      onExecute(sql);
    }
  };

  const queryHistory = [
    'SELECT * FROM users WHERE email LIKE "%@gmail.com"',
    'SELECT COUNT(*) FROM orders WHERE status = "completed"',
    "UPDATE products SET stock = stock - 1 WHERE id = 123",
  ];

  return (
    <div className="h-full flex flex-col bg-white">
      <div className="flex items-center justify-between px-4 py-2 border-b border-gray-200">
        <div className="flex items-center gap-2">
          <Button onClick={handleExecute} size="sm" className="gap-2">
            <Play className="w-4 h-4" />
            Execute
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
            Save
          </Button>
          <Button variant="outline" size="sm" className="gap-2">
            <Trash2 className="w-4 h-4" />
            Clear
          </Button>
        </div>
        <div className="flex items-center gap-2 text-sm text-gray-600">
          <Clock className="w-4 h-4" />
          <span>
            {queryResults?.executionTime
              ? `Last executed: ${queryResults.executionTime}`
              : "Ready to execute"}
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
                  onChange={(value) => setSql(value || "")}
                  theme="vs-light"
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

              <div className="border-t border-gray-200 bg-gray-50">
                <div className="px-4 py-2">
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm font-semibold text-gray-700">
                      Query History
                    </span>
                    <Button variant="ghost" size="sm" className="h-6 text-xs">
                      Clear All
                    </Button>
                  </div>
                  <div className="space-y-1">
                    {queryHistory.map((query, index) => (
                      <div
                        key={index}
                        className="text-xs font-mono bg-white px-2 py-1 rounded border border-gray-200 hover:border-blue-300 cursor-pointer truncate"
                        onClick={() => setSql(query)}
                      >
                        {query}
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          </ResizablePanel>

          {/* Results Panel */}
          {queryResults && (
            <>
              <ResizableHandle withHandle />
              <ResizablePanel defaultSize={50} minSize={20}>
                <div className="h-full flex flex-col">
                  <div className="px-4 py-2 border-b border-gray-200 bg-gray-50">
                    <span className="text-sm font-semibold text-gray-700">
                      Query Results
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
