import { useEffect, useState } from "react";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Sidebar } from "@/components/business/Sidebar/Sidebar";
import { SqlEditor } from "@/components/business/Editor/SqlEditor";
import { TableView } from "@/components/business/DataGrid/TableView";
import { TableMetadataView } from "@/components/business/Metadata/TableMetadataView";
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
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { api, isTauri, SchemaOverview, SavedQuery } from "@/services/api";
import { listen } from "@tauri-apps/api/event";
import { SettingsDialog } from "@/components/settings/SettingsDialog";

interface TabItem {
  id: string;
  type: "editor" | "table" | "ddl";
  title: string;
  connection?: string;
  database?: string;
  schema?: string;
  tableName?: string;
  data?: any[];
  columns?: string[];
  total?: number;
  page?: number;
  pageSize?: number;
  executionTimeMs?: number;
  connectionId?: number;
  driver?: string;
  sqlContent?: string;
  sortColumn?: string;
  sortDirection?: "asc" | "desc";
  filter?: string;
  orderBy?: string;
  queryResults?: {
    data: any[];
    columns: string[];
    executionTime: string;
    error?: string;
  } | null;
  schemaOverview?: SchemaOverview;
  savedQueryId?: number;
}

const TAB_TRIGGER_CLASS =
  "gap-2 group relative pr-8 bg-transparent data-[state=active]:bg-background border-b-2 border-b-transparent data-[state=active]:border-b-primary rounded-none h-9 hover:bg-muted/50 border-r border-r-border/40 last:border-r-0";

export default function App() {
  const [tabs, setTabs] = useState<TabItem[]>([]);
  const [activeTab, setActiveTab] = useState<string>("");
  const [aiVisible, setAiVisible] = useState(false);
  const [openSettings, setOpenSettings] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;

    // Remove global connections fetching as it's handled by Sidebar
    // api.connections.list().then(setConnections).catch(console.error);

    const unlistenChunk = listen("query.chunk", (_evt: any) => {
      // TODO: Handle streaming chunks for specific tabs if backend supports session/request ID
      // For now, simple execute returns full result, so this might not be needed for basic execution
      // If needed, we'd need to map evt to a specific tab
    });

    const unlistenProgress = listen("query.progress", () => { });
    const unlistenDone = listen("query.done", () => { });

    return () => {
      unlistenChunk.then((f) => f());
      unlistenProgress.then((f) => f());
      unlistenDone.then((f) => f());
    };
  }, []);

  const handleCreateQuery = (
    connectionId: number,
    databaseName: string,
    driver: string,
  ) => {
    const newTabId = `query-${connectionId}-${databaseName}-${Date.now()}`;
    const newTab: TabItem = {
      id: newTabId,
      type: "editor",
      title: `Query (${databaseName})`,
      connectionId,
      database: databaseName,
      driver,
      sqlContent: "-- Enter your SQL query here\n",
      queryResults: null,
    };
    setTabs((prev) => [...prev, newTab]);
    setActiveTab(newTabId);

    // Fetch schema overview for completion
    api.metadata.getSchemaOverview(connectionId, databaseName)
      .then((schemaOverview) => {
        setTabs((prev) =>
          prev.map((t) => (t.id === newTabId ? { ...t, schemaOverview } : t))
        );
      })
      .catch((e) => console.error("Failed to fetch schema overview:", e instanceof Error ? e.message : String(e)));
  };

  const handleOpenSavedQuery = async (query: SavedQuery) => {
    const newTabId = `saved-query-${query.id}`;

    // Check if tab already exists
    const existingTab = tabs.find(t => t.id === newTabId);
    if (existingTab) {
      setActiveTab(newTabId);
      return;
    }

    let connectionId = query.connectionId || undefined;
    let driver: string | undefined = undefined;
    let database: string | undefined = query.database || undefined;

    // If query is linked to a connection, try to fetch connection details
    if (connectionId) {
      try {
        // We need to get connection details to know driver and default database
        // But api.connections.list returns all connections. 
        // We can iterate or assume if we have a way to get single connection.
        // For now, let's just list and find.
        // Optimized approach: add get_connection_by_id to api if needed, 
        // but for now list is cached/fast enough locally? 
        // Actually, we can just let the user select connection if it's missing details,
        // but we want to be helpful.

        // NOTE: Ideally we should have api.connections.get(id). 
        // But for now, let's just leave driver/database empty if we can't easily get them,
        // or fetch list.
        const conns = await api.connections.list();
        const conn = conns.find((c: any) => c.id === connectionId);
        if (conn) {
          driver = conn.dbType;
          // Only fallback to connection default if no specific database was saved
          if (!database) {
            database = conn.database;
          }
        }
      } catch (e) {
        console.error("Failed to fetch connection details for saved query", e);
      }
    }

    const newTab: TabItem = {
      id: newTabId,
      type: "editor",
      title: query.name,
      connectionId,
      database,
      driver,
      sqlContent: query.query,
      savedQueryId: query.id,
      queryResults: null,
    };
    setTabs((prev) => [...prev, newTab]);
    setActiveTab(newTabId);
  };

  const handleSqlChange = (tabId: string, sql: string) => {
    setTabs((prev) =>
      prev.map((t) => {
        if (t.id !== tabId) return t;
        return { ...t, sqlContent: sql };
      }),
    );
  };

  const handleExecuteQuery = async (tabId: string, sql: string) => {
    const tab = tabs.find((t) => t.id === tabId);
    if (!tab || !tab.connectionId) {
      // TODO: Prompt user to select connection if missing
      alert("Please select a connection first (feature pending)");
      return;
    }

    const start = performance.now();
    try {
      const result = await api.query.execute(tab.connectionId, sql, tab.database);
      const columns = (result.columns || []).map((c) => c.name);
      const execMs = Math.round(
        result.timeTakenMs ?? performance.now() - start,
      );

      setTabs((prev) =>
        prev.map((t) => {
          if (t.id !== tabId) return t;
          return {
            ...t,
            queryResults: {
              data: result.data || [],
              columns,
              executionTime: `${execMs}ms`,
            },
          };
        }),
      );
    } catch (e) {
      const errorMessage = e instanceof Error ? e.message : String(e);
      console.error("execute_query failed:", errorMessage);
      setTabs((prev) =>
        prev.map((t) => {
          if (t.id !== tabId) return t;
          return {
            ...t,
            queryResults: {
              data: [],
              columns: [],
              executionTime: "0ms",
              error: errorMessage,
            },
          };
        }),
      );
    }
  };

  const handleTableSelect = async (
    connection: string,
    database: string,
    table: string,
    connectionId: number,
    driver: string,
  ) => {
    const tabId = `${connection}-${database}-${table}`;
    const existingTab = tabs.find((t) => t.id === tabId);
    if (existingTab) {
      setActiveTab(tabId);
      return;
    }
    try {
      const schema = driver === "mysql" ? database : "public";

      const resp = await api.tableData.get({
        id: connectionId,
        schema,
        table,
        page: 1,
        limit: 50,
      });
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
        total: resp.total,
        page: resp.page,
        pageSize: resp.limit,
        executionTimeMs: resp.executionTimeMs,
        connectionId,
        driver,
      };
      setTabs([...tabs, newTab]);
      setActiveTab(tabId);
    } catch (e) {
      console.error("get_table_data failed", e instanceof Error ? e.message : String(e));
    }
  };

  const handleOpenTableDDL = (ctx: {
    connectionId: number;
    database: string;
    schema: string;
    table: string;
  }) => {
    const tabId = `ddl-${ctx.connectionId}-${ctx.database}-${ctx.schema}-${ctx.table}`;
    const existingTab = tabs.find((t) => t.id === tabId);
    if (existingTab) {
      setActiveTab(tabId);
      return;
    }

    const newTab: TabItem = {
      id: tabId,
      type: "ddl",
      title: `DDL: ${ctx.table}`,
      connectionId: ctx.connectionId,
      database: ctx.database,
      schema: ctx.schema,
      tableName: ctx.table,
    };
    setTabs((prev) => [...prev, newTab]);
    setActiveTab(tabId);
  };

  const handlePageChange = async (tabId: string, page: number) => {
    const tab = tabs.find((t) => t.id === tabId);
    if (!tab || !tab.connectionId || !tab.driver || !tab.tableName) return;

    try {
      const schema = tab.driver === "mysql" ? tab.database : "public";
      const resp = await api.tableData.get({
        id: tab.connectionId,
        schema: schema || "public",
        table: tab.tableName,
        page,
        limit: tab.pageSize || 50,
        filter: tab.filter,
        sortColumn: tab.sortColumn,
        sortDirection: tab.sortDirection,
        orderBy: tab.orderBy,
      });

      setTabs((prev) =>
        prev.map((t) => {
          if (t.id !== tabId) return t;
          return {
            ...t,
            data: resp.data,
            total: resp.total,
            page: resp.page,
            executionTimeMs: resp.executionTimeMs,
          };
        }),
      );
    } catch (e) {
      console.error("handlePageChange failed", e instanceof Error ? e.message : String(e));
    }
  };

  const handleSortChange = async (tabId: string, column: string, direction: "asc" | "desc") => {
    const tab = tabs.find((t) => t.id === tabId);
    if (!tab || !tab.connectionId || !tab.driver || !tab.tableName) return;

    // Optimistically update sort state
    setTabs((prev) =>
      prev.map((t) => {
        if (t.id !== tabId) return t;
        return { ...t, sortColumn: column, sortDirection: direction };
      }),
    );

    try {
      const schema = tab.driver === "mysql" ? tab.database : "public";
      const resp = await api.tableData.get({
        id: tab.connectionId,
        schema: schema || "public",
        table: tab.tableName,
        page: 1, // Reset to first page on sort change
        limit: tab.pageSize || 50,
        filter: tab.filter,
        sortColumn: column,
        sortDirection: direction,
        orderBy: tab.orderBy,
      });

      setTabs((prev) =>
        prev.map((t) => {
          if (t.id !== tabId) return t;
          return {
            ...t,
            data: resp.data,
            total: resp.total,
            page: resp.page,
            executionTimeMs: resp.executionTimeMs,
            sortColumn: column,
            sortDirection: direction,
          };
        }),
      );
    } catch (e) {
      console.error("handleSortChange failed", e instanceof Error ? e.message : String(e));
    }
  };

  const handleFilterChange = async (tabId: string, filter: string, orderBy: string) => {
    const tab = tabs.find((t) => t.id === tabId);
    if (!tab || !tab.connectionId || !tab.driver || !tab.tableName) return;

    // Optimistically update filter/orderBy state
    setTabs((prev) =>
      prev.map((t) => {
        if (t.id !== tabId) return t;
        return { ...t, filter, orderBy };
      }),
    );

    try {
      const schema = tab.driver === "mysql" ? tab.database : "public";
      const resp = await api.tableData.get({
        id: tab.connectionId,
        schema: schema || "public",
        table: tab.tableName,
        page: 1, // Reset to first page on filter change
        limit: tab.pageSize || 50,
        filter: filter || undefined,
        sortColumn: tab.sortColumn,
        sortDirection: tab.sortDirection,
        orderBy: orderBy || undefined,
      });

      const columns = resp.data.length > 0 ? Object.keys(resp.data[0]) : tab.columns;
      setTabs((prev) =>
        prev.map((t) => {
          if (t.id !== tabId) return t;
          return {
            ...t,
            data: resp.data,
            columns,
            total: resp.total,
            page: resp.page,
            executionTimeMs: resp.executionTimeMs,
            filter,
            orderBy,
          };
        }),
      );
    } catch (e) {
      console.error("handleFilterChange failed", e instanceof Error ? e.message : String(e));
    }
  };

  const handleCloseTab = (tabId: string) => {
    const newTabs = tabs.filter((t) => t.id !== tabId);
    setTabs(newTabs);

    if (activeTab === tabId) {
      setActiveTab(newTabs[newTabs.length - 1]?.id || "");
    }
  };

  const handleCloseOtherTabs = (tabId: string) => {
    setTabs((prev) => prev.filter((t) => t.id === tabId));
    setActiveTab(tabId);
  };

  // Global Keyboard Shortcuts
  useEffect(() => {
    const handleGlobalKeyDown = (e: KeyboardEvent) => {
      // Check for Mod key (Cmd on Mac, Ctrl on Windows)
      const isMod = e.metaKey || e.ctrlKey;
      if (!isMod) return;

      switch (e.key.toLowerCase()) {
        case "w":
          e.preventDefault();
          if (activeTab) {
            handleCloseTab(activeTab);
          }
          break;
        case "n":
          e.preventDefault();
          // Find current active tab to get context for new query
          const currentTab = tabs.find((t) => t.id === activeTab);
          if (
            currentTab &&
            currentTab.connectionId &&
            currentTab.database &&
            currentTab.driver
          ) {
            handleCreateQuery(
              currentTab.connectionId,
              currentTab.database,
              currentTab.driver
            );
          }
          break;
        case "\\": // Backslash for AI toggle
          e.preventDefault();
          setAiVisible((v) => !v);
          break;
        case ",": // Comma for settings
          e.preventDefault();
          setOpenSettings(true);
          break;
      }
    };

    window.addEventListener("keydown", handleGlobalKeyDown);
    return () => {
      window.removeEventListener("keydown", handleGlobalKeyDown);
    };
  }, [activeTab, tabs, aiVisible]);

  return (
    <div className="h-screen w-screen flex flex-col bg-muted/30">
      {/* Header */}
      <header className="h-12 bg-background border-b border-border flex items-center justify-between px-2 shadow-sm">
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

          <Button variant="ghost" size="sm" className="h-8 w-8 p-0" onClick={() => setOpenSettings(true)}>
            <Settings className="w-4 h-4" />
          </Button>
          <Button
            variant={aiVisible ? "default" : "ghost"}
            size="sm"
            className="h-8 w-8 p-0"
            onClick={() => setAiVisible((v) => !v)}
            title={aiVisible ? "Hide AI Panel" : "Show AI Panel"}
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
              <DropdownMenuLabel>My Account</DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuItem>
                <User className="w-4 h-4 mr-2" />
                Profile
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={() => setOpenSettings(true)}>
                <Settings className="w-4 h-4 mr-2" />
                Settings
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem>
                <LogOut className="w-4 h-4 mr-2" />
                Logout
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
            <Sidebar
              onTableSelect={handleTableSelect}
              onConnect={() => { }}
              onCreateQuery={handleCreateQuery}
              onSelectSavedQuery={handleOpenSavedQuery}
            />
          </ResizablePanel>

          <ResizableHandle />

          {/* Main Panel - SQL Editor & Results */}
          <ResizablePanel defaultSize={60} minSize={40}>
            {tabs.length === 0 ? (
              <div className="h-full flex items-center justify-center text-muted-foreground">
                <div className="text-center">
                  <FileCode className="w-12 h-12 mx-auto mb-2 opacity-50" />
                  <p>Select a table or create a new query from the sidebar</p>
                </div>
              </div>
            ) : (
              <Tabs
                value={activeTab}
                onValueChange={setActiveTab}
                className="h-full flex flex-col"
              >
                <div className="bg-muted/30">
                  <TabsList className="h-9 w-full justify-start gap-0 bg-transparent border-none border-b-0 p-0 overflow-x-auto">
                    {tabs.map((tab) => (
                      <ContextMenu key={tab.id}>
                        <ContextMenuTrigger asChild>
                          {/* Wrapper avoids data-state conflict: ContextMenu and Tabs both set it; only the trigger must get Tabs' data-state=active for the indicator bar */}
                          <span className="contents">
                            <TabsTrigger
                              value={tab.id}
                              className={TAB_TRIGGER_CLASS}
                              onMouseDown={(e) => {
                                if (e.button === 1) {
                                  e.preventDefault();
                                  handleCloseTab(tab.id);
                                }
                              }}
                            >
                              {tab.type === "table" ? (
                                <Table className="w-4 h-4 text-primary" />
                              ) : (
                                <FileCode className="w-4 h-4 text-primary" />
                              )}
                              <span className="truncate max-w-[120px]">
                                {tab.title}
                              </span>
                              <button
                                type="button"
                                aria-label={`Close ${tab.title}`}
                                className="absolute right-1 top-1/2 -translate-y-1/2 opacity-0 group-hover:opacity-100 p-1 hover:bg-accent rounded-sm cursor-pointer transition-opacity"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  handleCloseTab(tab.id);
                                }}
                              >
                                <X className="w-3 h-3 text-muted-foreground" />
                              </button>
                            </TabsTrigger>
                          </span>
                        </ContextMenuTrigger>
                        <ContextMenuContent>
                          <ContextMenuItem onClick={() => handleCloseTab(tab.id)}>
                            Close Tab
                          </ContextMenuItem>
                          <ContextMenuItem onClick={() => handleCloseOtherTabs(tab.id)}>
                            Close Other Tabs
                          </ContextMenuItem>
                        </ContextMenuContent>
                      </ContextMenu>
                    ))}
                  </TabsList>
                </div>

                <div className="flex-1 overflow-hidden">
                  {tabs.map((tab) => (
                    <TabsContent
                      key={tab.id}
                      value={tab.id}
                      className="h-full m-0"
                    >
                      {tab.type === "editor" ? (
                        <SqlEditor
                          databaseName={tab.database}
                          onExecute={(sql) => handleExecuteQuery(tab.id, sql)}
                          onCancel={() =>
                            api.query.cancel(tab.id, `q-${tab.connectionId}`)
                          }
                          queryResults={tab.queryResults}
                          value={tab.sqlContent}
                          onChange={(sql) => handleSqlChange(tab.id, sql)}
                          connectionId={tab.connectionId}
                          driver={tab.driver}
                          schemaOverview={tab.schemaOverview}
                          savedQueryId={tab.savedQueryId}
                          initialName={tab.title.startsWith("Query (") ? "" : tab.title}
                          onSaveSuccess={(savedQuery) => {
                            setTabs((prev) =>
                              prev.map((t) => {
                                if (t.id === tab.id) {
                                  return {
                                    ...t,
                                    savedQueryId: savedQuery.id,
                                    title: savedQuery.name,
                                  };
                                }
                                return t;
                              })
                            );
                          }}
                        />
                      ) : tab.type === "table" ? (
                        <TableView
                          data={tab.data}
                          columns={tab.columns}
                          total={tab.total}
                          page={tab.page}
                          pageSize={tab.pageSize}
                          executionTimeMs={tab.executionTimeMs}
                          onPageChange={(p) => handlePageChange(tab.id, p)}
                          sortColumn={tab.sortColumn}
                          sortDirection={tab.sortDirection}
                          onSortChange={(col, dir) =>
                            handleSortChange(tab.id, col, dir)
                          }
                          filter={tab.filter}
                          orderBy={tab.orderBy}
                          onFilterChange={(f, ob) =>
                            handleFilterChange(tab.id, f, ob)
                          }
                          onOpenDDL={handleOpenTableDDL}
                          onDataRefresh={() => handlePageChange(tab.id, tab.page || 1)}
                          tableContext={
                            tab.connectionId && tab.database && tab.tableName && tab.driver
                              ? {
                                connectionId: tab.connectionId,
                                database: tab.database,
                                schema:
                                  tab.driver === "mysql"
                                    ? tab.database
                                    : "public",
                                table: tab.tableName,
                                driver: tab.driver,
                              }
                              : undefined
                          }
                        />
                      ) : tab.connectionId &&
                        tab.database &&
                        tab.schema &&
                        tab.tableName ? (
                        <TableMetadataView
                          connectionId={tab.connectionId}
                          database={tab.database}
                          schema={tab.schema}
                          table={tab.tableName}
                        />
                      ) : null}
                    </TabsContent>
                  ))}
                </div>
              </Tabs>
            )}
          </ResizablePanel>

          <ResizableHandle />

          {/* Right Sidebar - AI Assistant */}
          {aiVisible && (
            <ResizablePanel defaultSize={20} minSize={15} maxSize={30}>
              <AISidebar />
            </ResizablePanel>
          )}
        </ResizablePanelGroup>
      </div>
      <SettingsDialog open={openSettings} onOpenChange={setOpenSettings} />
    </div>
  );
}
