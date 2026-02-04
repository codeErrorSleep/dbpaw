import { useEffect, useState } from "react";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import { DatabaseSidebar } from "@/components/business/Sidebar/DatabaseSidebar";
import { SqlEditor } from "@/components/business/Editor/SqlEditor";
import { TableView } from "@/components/business/DataGrid/TableView";
import { AISidebar } from "@/components/business/Sidebar/AISidebar";
import {
  FileCode,
  Table,
  X,
  Settings,
  User,
  Bell,
  LogOut,
  Sparkles,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@/components/ui/avatar";
import { api, isTauri } from "@/services/api";
import type { ConnectionForm } from "@/services/api";
import { listen } from "@tauri-apps/api/event";

interface TabItem {
  id: string;
  type: "editor" | "table";
  title: string;
  connection?: string;
  database?: string;
  tableName?: string;
  data?: any[];
  columns?: string[];
}

export default function App() {
  const [tabs, setTabs] = useState<TabItem[]>([
    { id: "editor", type: "editor", title: "SQL Editor" },
  ]);
  const [activeTab, setActiveTab] = useState("editor");
  const [aiVisible, setAiVisible] = useState(true);
  const [queryResults, setQueryResults] = useState<{
    data: any[];
    columns: string[];
    executionTime: string;
  } | null>(null);
  const [activeConn, setActiveConn] = useState<ConnectionForm | null>(null);
  useEffect(() => {
    if (!isTauri()) return;

    const unlistenChunk = listen("query.chunk", (evt: any) => {
      const rows = (evt?.payload?.rows ?? []) as any[];
      setQueryResults((prev) => {
        if (!prev) {
          return {
            data: rows,
            columns: rows.length ? Object.keys(rows[0]) : [],
            executionTime: "streaming",
          };
        }
        const merged = [...prev.data, ...rows];
        return { ...prev, data: merged };
      });
    });

    const unlistenProgress = listen("query.progress", () => {});
    const unlistenDone = listen("query.done", () => {});

    // Cleanup function
    return () => {
      unlistenChunk.then((f) => f());
      unlistenProgress.then((f) => f());
      unlistenDone.then((f) => f());
    };
  }, []);

  const handleExecuteQuery = async (sql: string) => {
    const start = performance.now();
    try {
      const result = await api.query.execute("unused", sql);
      const columns = (result.columns || []).map((c) => c.name);
      const execMs = Math.round(
        result.timeTakenMs ?? performance.now() - start,
      );
      setQueryResults({
        data: result.data || [],
        columns,
        executionTime: `${execMs}ms`,
      });
    } catch (e) {
      console.error("execute_query failed", e);
      setQueryResults({
        data: [],
        columns: [],
        executionTime: "0ms",
      });
    }
  };

  const handleTableSelect = async (
    connection: string,
    database: string,
    table: string,
    form: ConnectionForm,
  ) => {
    const tabId = `${connection}-${database}-${table}`;
    const existingTab = tabs.find((t) => t.id === tabId);
    if (existingTab) {
      setActiveTab(tabId);
      return;
    }
    try {
      const resp = await api.tableData.getByConn(
        form,
        form.schema || "public",
        table,
        1,
        50,
      );
      const columns = resp.data.length > 0 ? Object.keys(resp.data[0]) : [];
      const newTab: TabItem = {
        id: tabId,
        type: "table",
        title: table,
        connection,
        database,
        tableName: table,
        data: resp.data,
        columns,
        columns,
      };
      setTabs([...tabs, newTab]);
      setActiveTab(tabId);
      setActiveConn(form);
    } catch (e) {
      console.error("get_table_data failed", e);
    }
  };

  const handleCloseTab = (tabId: string) => {
    if (tabId === "editor") return; // Don't close SQL Editor tab

    const newTabs = tabs.filter((t) => t.id !== tabId);
    setTabs(newTabs);

    if (activeTab === tabId) {
      setActiveTab(newTabs[newTabs.length - 1]?.id || "editor");
    }
  };

  return (
    <div className="h-screen w-screen flex flex-col bg-gray-100">
      {/* Header */}
      <header className="h-12 bg-white border-b border-gray-200 flex items-center justify-between px-2 shadow-sm">
        <div className="flex items-center gap-2">
          <img
            src="/product-icon.png"
            alt="DbPaw"
            className="w-8 h-8 rounded-lg object-cover"
          />
          <h1 className="font-semibold text-lg">DbPaw</h1>
        </div>

        <div className="flex items-center gap-2">
          <Button variant="ghost" size="sm" className="h-8 w-8 p-0">
            <Bell className="w-4 h-4" />
          </Button>

          <Button variant="ghost" size="sm" className="h-8 w-8 p-0">
            <Settings className="w-4 h-4" />
          </Button>
          <Button
            variant={aiVisible ? "default" : "ghost"}
            size="sm"
            className="h-8 w-8 p-0"
            onClick={() => setAiVisible((v) => !v)}
            title={aiVisible ? "隐藏 AI 面板" : "显示 AI 面板"}
            aria-label={aiVisible ? "Hide AI panel" : "Show AI panel"}
          >
            <Sparkles className="w-4 h-4" />
          </Button>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" className="h-8 w-8 p-0 justify-center">
                <Avatar className="w-6 h-6">
                  <AvatarImage src="https://github.com/shadcn.png" />
                  <AvatarFallback>AD</AvatarFallback>
                </Avatar>
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-48">
              <DropdownMenuLabel>我的账号</DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuItem>
                <User className="w-4 h-4 mr-2" />
                个人资料
              </DropdownMenuItem>
              <DropdownMenuItem>
                <Settings className="w-4 h-4 mr-2" />
                设置
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem>
                <LogOut className="w-4 h-4 mr-2" />
                退出登录
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </header>

      {/* Main Content */}
      <div className="flex-1 overflow-hidden">
        <ResizablePanelGroup direction="horizontal">
          {/* Left Sidebar - Database Connections */}
          <ResizablePanel defaultSize={20} minSize={15} maxSize={30}>
            <DatabaseSidebar
              onTableSelect={handleTableSelect}
              onConnect={setActiveConn}
            />
          </ResizablePanel>

          <ResizableHandle withHandle />

          {/* Main Panel - SQL Editor & Results */}
          <ResizablePanel defaultSize={60} minSize={40}>
            <Tabs
              value={activeTab}
              onValueChange={setActiveTab}
              className="h-full flex flex-col"
            >
              <div className="bg-muted/30 border-b border-gray-200 px-2">
                <TabsList className="h-10">
                  <TabsTrigger value="editor" className="gap-2">
                    <FileCode className="w-4 h-4" />
                    SQL Editor
                  </TabsTrigger>
                  {tabs
                    .filter((t) => t.type === "table")
                    .map((tab) => (
                      <TabsTrigger
                        key={tab.id}
                        value={tab.id}
                        className="gap-2 group"
                      >
                        <Table className="w-3 h-3" />
                        {tab.title}
                        <button
                          className="ml-2 opacity-0 group-hover:opacity-100 hover:bg-gray-200 rounded p-0.5"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleCloseTab(tab.id);
                          }}
                        >
                          <X className="w-3 h-3" />
                        </button>
                      </TabsTrigger>
                    ))}
                </TabsList>
              </div>

              <div className="flex-1 overflow-hidden">
                <TabsContent value="editor" className="h-full m-0">
                  <SqlEditor
                    onExecute={(sql) => {
                      if (!activeConn) return handleExecuteQuery(sql);
                      api.query
                        .executeByConn(activeConn, sql)
                        .then((result) => {
                          const cols = (result.columns || []).map(
                            (c) => c.name,
                          );
                          setQueryResults({
                            data: result.data || [],
                            columns: cols,
                            executionTime: `${result.timeTakenMs || 0}ms`,
                          });
                        })
                        .catch((e) => {
                          console.error(e);
                        });
                    }}
                    onCancel={() => api.query.cancel("unused", "q-1")}
                    queryResults={queryResults}
                  />
                </TabsContent>

                {tabs
                  .filter((t) => t.type === "table")
                  .map((tab) => (
                    <TabsContent
                      key={tab.id}
                      value={tab.id}
                      className="h-full m-0"
                    >
                      <TableView data={tab.data} columns={tab.columns} />
                    </TabsContent>
                  ))}
              </div>
            </Tabs>
          </ResizablePanel>

          <ResizableHandle withHandle />

          {/* Right Sidebar - AI Assistant */}
          {aiVisible && (
            <ResizablePanel defaultSize={20} minSize={15} maxSize={30}>
              <AISidebar />
            </ResizablePanel>
          )}
        </ResizablePanelGroup>
      </div>
    </div>
  );
}
