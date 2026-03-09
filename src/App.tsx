import {
  lazy,
  MouseEvent,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Sidebar } from "@/components/business/Sidebar/Sidebar";
import { SaveQueryDialog } from "@/components/business/Editor/SaveQueryDialog";
import { TableView } from "@/components/business/DataGrid/TableView";
import { TableMetadataView } from "@/components/business/Metadata/TableMetadataView";
import { SqlExecutionLogsDropdown } from "@/components/business/SqlLogs/SqlExecutionLogsDialog";
import { FileCode, Table, X, Settings, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { api, isTauri, SchemaOverview, SavedQuery } from "@/services/api";
import { toast } from "sonner";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { UpdaterChecker } from "@/components/updater-checker";
import { isModKey, shouldIgnoreGlobalShortcut } from "@/lib/keyboard";
import {
  DndContext,
  closestCenter,
  PointerSensor,
  useSensor,
  useSensors,
  DragEndEvent,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  horizontalListSortingStrategy,
} from "@dnd-kit/sortable";
import { SortableTab } from "@/components/ui/sortable-tab";
import { useTranslation } from "react-i18next";
import { applyQueryCompletionToTab } from "@/lib/queryExecutionState";
import {
  normalizeDatabaseOptions,
  resolvePreferredDatabase,
} from "@/lib/sqlEditorDatabase";

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
  lastSavedSql?: string;
  isDirty?: boolean;
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
  activeQueryId?: string;
  schemaOverview?: SchemaOverview;
  savedQueryId?: number;
  savedQueryDescription?: string;
  availableDatabases?: string[];
}

type TableRefreshOverrides = {
  page?: number;
  limit?: number;
  filter?: string;
  orderBy?: string;
};

type ActiveTableTarget = {
  connectionId: number;
  database: string;
  table: string;
  schema?: string;
};

const DEFAULT_SQL = "";

const TAB_TRIGGER_CLASS =
  "gap-2 group relative pr-8 bg-transparent data-[state=active]:bg-background border-b-2 border-b-transparent data-[state=active]:border-b-primary rounded-none h-9 hover:bg-muted/50 border-r border-r-border/40 last:border-r-0 shrink-0";

const SqlEditor = lazy(async () => {
  const mod = await import("@/components/business/Editor/SqlEditor");
  return { default: mod.SqlEditor };
});

const AISidebar = lazy(async () => {
  const mod = await import("@/components/business/Sidebar/AISidebar");
  return { default: mod.AISidebar };
});

const SettingsDialog = lazy(async () => {
  const mod = await import("@/components/settings/SettingsDialog");
  return { default: mod.SettingsDialog };
});

function LazyPanelFallback({
  label,
  className = "h-full",
}: {
  label: string;
  className?: string;
}) {
  return (
    <div className={`${className} flex items-center justify-center text-sm text-muted-foreground`}>
      {label}
    </div>
  );
}

export default function App() {
  const { t } = useTranslation();
  const resolveTableScope = (driver: string, database?: string) => {
    const isDatabaseScoped =
      driver === "mysql" ||
      driver === "tidb" ||
      driver === "mariadb" ||
      driver === "clickhouse";
    return {
      schema: isDatabaseScoped
        ? database || ""
        : driver === "mssql"
          ? "dbo"
          : driver === "sqlite" || driver === "duckdb"
            ? "main"
            : "public",
      dbParam: isDatabaseScoped ? undefined : database,
    };
  };

  const fetchEditorDatabases = useCallback(
    async (connectionId: number, fallbackDatabase?: string) => {
      const databases = await api.metadata.listDatabasesById(connectionId);
      return normalizeDatabaseOptions(databases, fallbackDatabase);
    },
    [],
  );

  const fetchEditorSchemaOverview = useCallback(
    async (connectionId: number, database?: string) =>
      api.metadata.getSchemaOverview(connectionId, database),
    [],
  );

  const [tabs, setTabs] = useState<TabItem[]>([]);
  const [activeTab, setActiveTab] = useState<string>("");
  const [aiVisible, setAiVisible] = useState(false);
  const [openSettings, setOpenSettings] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const isDefaultQueryTitle = (title?: string) =>
    !!title && /^(Query \(|查询（|クエリ（)/.test(title);
  const [queriesLastUpdated, setQueriesLastUpdated] = useState(0);
  const [pendingCloseTabIds, setPendingCloseTabIds] = useState<string[]>([]);
  const [currentCloseTabId, setCurrentCloseTabId] = useState<string | null>(
    null,
  );
  const [isUnsavedConfirmOpen, setIsUnsavedConfirmOpen] = useState(false);
  const [isCloseSaveDialogOpen, setIsCloseSaveDialogOpen] = useState(false);
  const closeSaveCompletedRef = useRef(false);
  const unsavedConfirmActionRef = useRef<"save" | "discard" | null>(null);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8,
      },
    }),
  );

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;

    if (over && active.id !== over.id) {
      setTabs((items) => {
        const oldIndex = items.findIndex((item) => item.id === active.id);
        const newIndex = items.findIndex((item) => item.id === over.id);

        return arrayMove(items, oldIndex, newIndex);
      });
    }
  };

  const handleWindowDragStart = (event: MouseEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    const target = event.target as HTMLElement;
    if (target.closest('[data-no-drag="true"]')) return;
    getCurrentWindow()
      .startDragging()
      .catch(() => {
        // Keep attribute drag region as fallback.
      });
  };

  const renderWindowActions = () => (
    <>
      <Button
        variant="ghost"
        size="sm"
        className="h-7 w-7 p-0"
        onClick={() => setOpenSettings(true)}
        title={t("app.window.settingsTooltip")}
        aria-label={t("app.window.openSettings")}
      >
        <Settings className="w-4 h-4" />
      </Button>
      <SqlExecutionLogsDropdown />
      <Button
        variant={aiVisible ? "default" : "ghost"}
        size="sm"
        className="h-7 w-7 p-0"
        onClick={() => setAiVisible((v) => !v)}
        title={
          aiVisible
            ? t("app.window.hideAiPanel")
            : t("app.window.showAiPanel")
        }
        aria-label={aiVisible ? t("app.window.hideAiPanelAria") : t("app.window.showAiPanelAria")}
      >
        <Sparkles className="w-4 h-4" />
      </Button>
    </>
  );

  useEffect(() => {
    if (!isTauri()) return;

    // Remove global connections fetching as it's handled by Sidebar
    // api.connections.list().then(setConnections).catch(console.error);

    const unlistenChunk = listen("query.chunk", (_evt: any) => {
      // TODO: Handle streaming chunks for specific tabs if backend supports session/request ID
      // For now, simple execute returns full result, so this might not be needed for basic execution
      // If needed, we'd need to map evt to a specific tab
    });

    const unlistenProgress = listen("query.progress", () => {});
    const unlistenDone = listen("query.done", () => {});
    const unlistenSettings = listen("open-settings", () =>
      setOpenSettings(true),
    );

    return () => {
      unlistenChunk.then((f) => f());
      unlistenProgress.then((f) => f());
      unlistenDone.then((f) => f());
      unlistenSettings.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (!isTauri()) return;

    const appWindow = getCurrentWindow();
    let mounted = true;
    let unlistenResized: null | (() => void) = null;

    const syncFullscreenState = async () => {
      try {
        const fullscreen = await appWindow.isFullscreen();
        if (mounted) setIsFullscreen(fullscreen);
      } catch {
        // Ignore window state lookup failures in non-native contexts.
      }
    };

    void syncFullscreenState();
    appWindow
      .onResized(() => {
        void syncFullscreenState();
      })
      .then((unlisten) => {
        unlistenResized = unlisten;
      })
      .catch(() => {
        // Ignore event binding failures.
      });

    return () => {
      mounted = false;
      if (unlistenResized) unlistenResized();
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
      title: t("app.tab.queryTitle", { database: databaseName }),
      connectionId,
      database: databaseName,
      driver,
      availableDatabases: normalizeDatabaseOptions([databaseName], databaseName),
      sqlContent: DEFAULT_SQL,
      lastSavedSql: DEFAULT_SQL,
      isDirty: false,
      queryResults: null,
    };
    setTabs((prev) => [...prev, newTab]);
    setActiveTab(newTabId);

    Promise.allSettled([
      fetchEditorDatabases(connectionId, databaseName),
      fetchEditorSchemaOverview(connectionId, databaseName),
    ])
      .then(([availableDatabasesResult, schemaOverviewResult]) => {
        if (availableDatabasesResult.status === "rejected") {
          console.error(
            "Failed to load editor databases:",
            availableDatabasesResult.reason instanceof Error
              ? availableDatabasesResult.reason.message
              : String(availableDatabasesResult.reason),
          );
        }
        if (schemaOverviewResult.status === "rejected") {
          console.error(
            "Failed to load schema overview:",
            schemaOverviewResult.reason instanceof Error
              ? schemaOverviewResult.reason.message
              : String(schemaOverviewResult.reason),
          );
        }

        const availableDatabases =
          availableDatabasesResult.status === "fulfilled"
            ? availableDatabasesResult.value
            : normalizeDatabaseOptions([databaseName], databaseName);
        const schemaOverview =
          schemaOverviewResult.status === "fulfilled"
            ? schemaOverviewResult.value
            : undefined;

        setTabs((prev) =>
          prev.map((t) =>
            t.id === newTabId
              ? {
                  ...t,
                  database: resolvePreferredDatabase({
                    preferredDatabase: databaseName,
                    connectionDatabase: databaseName,
                    availableDatabases,
                  }),
                  availableDatabases,
                  schemaOverview,
                }
              : t,
          ),
        );
      });
  };

  const handleOpenSavedQuery = async (query: SavedQuery) => {
    const newTabId = `saved-query-${query.id}`;

    // Check if tab already exists
    const existingTab = tabs.find(
      (t) => t.id === newTabId || t.savedQueryId === query.id,
    );
    if (existingTab) {
      setActiveTab(existingTab.id);
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

          let availableDatabases = normalizeDatabaseOptions(
            database ? [database] : [],
            conn.database || database,
          );
          try {
            availableDatabases = await fetchEditorDatabases(
              connectionId,
              conn.database || database,
            );
          } catch (e) {
            console.error(
              "Failed to load editor databases for saved query",
              e instanceof Error ? e.message : String(e),
            );
          }
          database = resolvePreferredDatabase({
            preferredDatabase: query.database || undefined,
            connectionDatabase: conn.database || undefined,
            availableDatabases,
          });

          let schemaOverview: SchemaOverview | undefined;
          if (database) {
            try {
              schemaOverview = await fetchEditorSchemaOverview(
                connectionId,
                database,
              );
            } catch (e) {
              console.error(
                "Failed to load schema overview for saved query",
                e instanceof Error ? e.message : String(e),
              );
            }
          }

          const newTab: TabItem = {
            id: newTabId,
            type: "editor",
            title: query.name,
            connectionId,
            database,
            driver,
            availableDatabases,
            schemaOverview,
            sqlContent: query.query,
            lastSavedSql: query.query,
            isDirty: false,
            savedQueryId: query.id,
            savedQueryDescription: query.description || undefined,
            queryResults: null,
          };
          setTabs((prev) => [...prev, newTab]);
          setActiveTab(newTabId);
          return;
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
      availableDatabases: normalizeDatabaseOptions(
        database ? [database] : [],
        database,
      ),
      sqlContent: query.query,
      lastSavedSql: query.query,
      isDirty: false,
      savedQueryId: query.id,
      savedQueryDescription: query.description || undefined,
      queryResults: null,
    };
    setTabs((prev) => [...prev, newTab]);
    setActiveTab(newTabId);
  };

  const handleEditorDatabaseChange = useCallback(
    async (tabId: string, database: string) => {
      const tab = tabs.find((item) => item.id === tabId);
      if (!tab || tab.type !== "editor" || !tab.connectionId) return;

      setTabs((prev) =>
        prev.map((item) =>
          item.id === tabId
            ? {
                ...item,
                title: isDefaultQueryTitle(item.title)
                  ? t("app.tab.queryTitle", { database })
                  : item.title,
                database,
                queryResults: null,
                activeQueryId: undefined,
                schemaOverview: undefined,
              }
            : item,
        ),
      );

      try {
        const schemaOverview = await fetchEditorSchemaOverview(
          tab.connectionId,
          database,
        );
        setTabs((prev) =>
          prev.map((item) =>
            item.id === tabId ? { ...item, schemaOverview } : item,
          ),
        );
      } catch (e) {
        const errorMessage = e instanceof Error ? e.message : String(e);
        console.error("Failed to switch editor database", errorMessage);
        toast.error(t("app.error.loadSchemaOverview"), {
          description: errorMessage,
        });
      }
    },
    [fetchEditorSchemaOverview, t, tabs],
  );

  const handleSqlChange = (tabId: string, sql: string) => {
    setTabs((prev) =>
      prev.map((t) => {
        if (t.id !== tabId) return t;
        return {
          ...t,
          sqlContent: sql,
          isDirty: sql !== (t.lastSavedSql ?? ""),
        };
      }),
    );
  };

  const handleExecuteQuery = async (tabId: string, sql: string) => {
    const tab = tabs.find((t) => t.id === tabId);
    if (!tab || !tab.connectionId) {
      // TODO: Prompt user to select connection if missing
      alert(t("app.error.selectConnectionFirst"));
      return;
    }

    const start = performance.now();
    const queryId = `q-${tab.connectionId}-${Date.now()}-${Math.random()
      .toString(36)
      .slice(2, 8)}`;
    setTabs((prev) =>
      prev.map((t) => (t.id === tabId ? { ...t, activeQueryId: queryId } : t)),
    );
    try {
      const result = await api.query.execute(
        tab.connectionId,
        sql,
        tab.database,
        "sql_editor",
        queryId,
      );
      const columns = (result.columns || []).map((c) => c.name);
      const execMs = Math.round(
        result.timeTakenMs ?? performance.now() - start,
      );

      setTabs((prev) =>
        prev.map((t) =>
          applyQueryCompletionToTab(t, tabId, queryId, {
            data: result.data || [],
            columns,
            executionTime: `${execMs}ms`,
          }),
        ),
      );
    } catch (e) {
      const errorMessage = e instanceof Error ? e.message : String(e);
      console.error("execute_query failed:", errorMessage);
      setTabs((prev) =>
        prev.map((t) =>
          applyQueryCompletionToTab(t, tabId, queryId, {
            data: [],
            columns: [],
            executionTime: "0ms",
            error: errorMessage,
          }),
        ),
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
      const { schema, dbParam } = resolveTableScope(driver, database);

      const resp = await api.tableData.get({
        id: connectionId,
        database: dbParam,
        schema,
        table,
        page: 1,
        limit: 100,
      });
      let columns = resp.data.length > 0 ? Object.keys(resp.data[0]) : [];

      // If data is empty, fetch columns from metadata
      if (columns.length === 0) {
        try {
          const meta = await api.metadata.getTableMetadata(
            connectionId,
            database,
            schema,
            table,
          );
          if (meta && meta.columns) {
            columns = meta.columns.map((c) => c.name);
          }
        } catch (e) {
          console.warn("Failed to fetch metadata for empty table:", e);
        }
      }

      const newTab: TabItem = {
        id: tabId,
        type: "table",
        title: table,
        connection,
        database,
        schema,
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
      const errorMessage = e instanceof Error ? e.message : String(e);
      console.error("get_table_data failed", errorMessage);
      toast.error(t("app.error.loadTableData"), {
        description: errorMessage,
      });
    }
  };

  const handleExportTableFromTree = async (
    ctx: {
      connectionId: number;
      database: string;
      schema: string;
      table: string;
      driver: string;
    },
    format: "csv" | "json" | "sql",
    filePath: string,
  ) => {
    try {
      const result = await api.transfer.exportTable({
        id: ctx.connectionId,
        database: ctx.database,
        schema: ctx.schema,
        table: ctx.table,
        driver: ctx.driver,
        format,
        scope: "full_table",
        filePath,
      });
      toast.success(t("app.success.exportCompleted", { count: result.rowCount }), {
        description: result.filePath,
      });
    } catch (e) {
      toast.error(t("app.error.exportFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
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
      title: t("app.tab.ddlTitle", { table: ctx.table }),
      connectionId: ctx.connectionId,
      database: ctx.database,
      schema: ctx.schema,
      tableName: ctx.table,
    };
    setTabs((prev) => [...prev, newTab]);
    setActiveTab(tabId);
  };

  const handleTableRefresh = async (
    tabId: string,
    overrides?: TableRefreshOverrides,
  ) => {
    const tab = tabs.find((t) => t.id === tabId);
    if (!tab || !tab.connectionId || !tab.driver || !tab.tableName) return;

    const hasOwn = <K extends keyof TableRefreshOverrides>(key: K) =>
      !!overrides && Object.prototype.hasOwnProperty.call(overrides, key);

    const nextPage = overrides?.page ?? tab.page ?? 1;
    const nextLimit = overrides?.limit ?? tab.pageSize ?? 100;
    const nextFilter = hasOwn("filter") ? overrides?.filter : tab.filter;
    const nextOrderBy = hasOwn("orderBy") ? overrides?.orderBy : tab.orderBy;

    try {
      const { schema, dbParam } = resolveTableScope(tab.driver, tab.database);
      const resp = await api.tableData.get({
        id: tab.connectionId,
        database: dbParam,
        schema: schema || "public",
        table: tab.tableName,
        page: nextPage,
        limit: nextLimit,
        filter: nextFilter || undefined,
        sortColumn: tab.sortColumn,
        sortDirection: tab.sortDirection,
        orderBy: nextOrderBy || undefined,
      });

      setTabs((prev) =>
        prev.map((t) => {
          if (t.id !== tabId) return t;
          return {
            ...t,
            data: resp.data,
            total: resp.total,
            page: resp.page,
            pageSize: resp.limit,
            executionTimeMs: resp.executionTimeMs,
            filter: nextFilter,
            orderBy: nextOrderBy,
          };
        }),
      );
    } catch (e) {
      const errorMessage = e instanceof Error ? e.message : String(e);
      console.error("handleTableRefresh failed", errorMessage);
      toast.error(t("app.error.refreshTable"), {
        description: errorMessage,
      });
    }
  };

  const handlePageChange = async (tabId: string, page: number) => {
    const tab = tabs.find((t) => t.id === tabId);
    if (!tab || !tab.connectionId || !tab.driver || !tab.tableName) return;

    try {
      const { schema, dbParam } = resolveTableScope(tab.driver, tab.database);
      const resp = await api.tableData.get({
        id: tab.connectionId,
        database: dbParam,
        schema: schema || "public",
        table: tab.tableName,
        page,
        limit: tab.pageSize || 100,
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
      const errorMessage = e instanceof Error ? e.message : String(e);
      console.error("handlePageChange failed", errorMessage);
      toast.error(t("app.error.changePage"), {
        description: errorMessage,
      });
    }
  };

  const handlePageSizeChange = async (tabId: string, pageSize: number) => {
    const tab = tabs.find((t) => t.id === tabId);
    if (!tab || !tab.connectionId || !tab.driver || !tab.tableName) return;

    try {
      const { schema, dbParam } = resolveTableScope(tab.driver, tab.database);
      const resp = await api.tableData.get({
        id: tab.connectionId,
        database: dbParam,
        schema: schema || "public",
        table: tab.tableName,
        page: 1,
        limit: pageSize,
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
            pageSize: resp.limit,
            executionTimeMs: resp.executionTimeMs,
          };
        }),
      );
    } catch (e) {
      const errorMessage = e instanceof Error ? e.message : String(e);
      console.error("handlePageSizeChange failed", errorMessage);
      toast.error(t("app.error.changePageSize"), {
        description: errorMessage,
      });
    }
  };

  const handleSortChange = async (
    tabId: string,
    column: string,
    direction: "asc" | "desc",
  ) => {
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
      const { schema, dbParam } = resolveTableScope(tab.driver, tab.database);
      const resp = await api.tableData.get({
        id: tab.connectionId,
        database: dbParam,
        schema: schema || "public",
        table: tab.tableName,
        page: 1, // Reset to first page on sort change
        limit: tab.pageSize || 100,
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
      const errorMessage = e instanceof Error ? e.message : String(e);
      console.error("handleSortChange failed", errorMessage);
      toast.error(t("app.error.sortTable"), {
        description: errorMessage,
      });
    }
  };

  const handleFilterChange = async (
    tabId: string,
    filter: string,
    orderBy: string,
  ) => {
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
      const { schema, dbParam } = resolveTableScope(tab.driver, tab.database);
      const resp = await api.tableData.get({
        id: tab.connectionId,
        database: dbParam,
        schema: schema || "public",
        table: tab.tableName,
        page: 1, // Reset to first page on filter change
        limit: tab.pageSize || 100,
        filter: filter || undefined,
        sortColumn: tab.sortColumn,
        sortDirection: tab.sortDirection,
        orderBy: orderBy || undefined,
      });

      const columns =
        resp.data.length > 0 ? Object.keys(resp.data[0]) : tab.columns;
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
      const errorMessage = e instanceof Error ? e.message : String(e);
      console.error("handleFilterChange failed", errorMessage);
      toast.error(t("app.error.filterTable"), {
        description: errorMessage,
      });
    }
  };

  const resetCloseFlow = useCallback(() => {
    setPendingCloseTabIds([]);
    setCurrentCloseTabId(null);
    setIsUnsavedConfirmOpen(false);
    setIsCloseSaveDialogOpen(false);
    closeSaveCompletedRef.current = false;
    unsavedConfirmActionRef.current = null;
  }, []);

  const closeTabNow = useCallback((tabId: string) => {
    setTabs((prev) => {
      const newTabs = prev.filter((t) => t.id !== tabId);
      setActiveTab((currentActiveTab) => {
        if (currentActiveTab !== tabId) return currentActiveTab;
        return newTabs[newTabs.length - 1]?.id || "";
      });
      return newTabs;
    });
  }, []);

  const saveEditorTab = useCallback(
    async (tab: TabItem, name: string, description: string) => {
      if (tab.type !== "editor") return;

      try {
        const query = tab.sqlContent || "";
        const payload = {
          name,
          description,
          query,
          connectionId: tab.connectionId || undefined,
          database: tab.database,
        };

        const savedQuery = tab.savedQueryId
          ? await api.queries.update(tab.savedQueryId, payload)
          : await api.queries.create(payload);

        setQueriesLastUpdated(Date.now());
        setTabs((prev) =>
          prev.map((t) =>
            t.id === tab.id
              ? {
                  ...t,
                  savedQueryId: savedQuery.id,
                  title: savedQuery.name,
                  savedQueryDescription: savedQuery.description || undefined,
                  sqlContent: savedQuery.query,
                  lastSavedSql: savedQuery.query,
                  isDirty: false,
                }
              : t,
          ),
        );
      } catch (e) {
        toast.error(t("app.error.saveQuery"), {
          description: e instanceof Error ? e.message : String(e),
        });
        throw e;
      }
    },
    [],
  );

  const continueCloseFlow = useCallback(
    (queue: string[]) => {
      if (queue.length === 0) {
        resetCloseFlow();
        return;
      }

      const [nextTabId, ...rest] = queue;
      const nextTab = tabs.find((t) => t.id === nextTabId);
      if (!nextTab) {
        continueCloseFlow(rest);
        return;
      }

      if (nextTab.type === "editor" && nextTab.isDirty) {
        setPendingCloseTabIds(queue);
        setCurrentCloseTabId(nextTabId);
        setIsUnsavedConfirmOpen(true);
        setIsCloseSaveDialogOpen(false);
        return;
      }

      closeTabNow(nextTabId);
      continueCloseFlow(rest);
    },
    [tabs, closeTabNow, resetCloseFlow],
  );

  const requestCloseTabs = useCallback(
    (tabIds: string[]) => {
      const existingTabIds = tabIds.filter((id) =>
        tabs.some((t) => t.id === id),
      );
      if (existingTabIds.length === 0) return;
      continueCloseFlow(existingTabIds);
    },
    [tabs, continueCloseFlow],
  );

  const handleCloseTab = useCallback(
    (tabId: string) => {
      requestCloseTabs([tabId]);
    },
    [requestCloseTabs],
  );

  const handleCloseOtherTabs = useCallback(
    (tabId: string) => {
      requestCloseTabs(tabs.filter((t) => t.id !== tabId).map((t) => t.id));
      setActiveTab(tabId);
    },
    [requestCloseTabs, tabs],
  );

  const handleUnsavedCloseCancel = useCallback(() => {
    resetCloseFlow();
  }, [resetCloseFlow]);

  const handleUnsavedCloseWithoutSave = useCallback(() => {
    unsavedConfirmActionRef.current = "discard";
    if (!currentCloseTabId) {
      resetCloseFlow();
      return;
    }

    closeTabNow(currentCloseTabId);
    const currentIndex = pendingCloseTabIds.indexOf(currentCloseTabId);
    const rest =
      currentIndex >= 0
        ? pendingCloseTabIds.slice(currentIndex + 1)
        : pendingCloseTabIds.filter((id) => id !== currentCloseTabId);
    continueCloseFlow(rest);
  }, [
    closeTabNow,
    continueCloseFlow,
    currentCloseTabId,
    pendingCloseTabIds,
    resetCloseFlow,
  ]);

  const handleUnsavedCloseSave = useCallback(() => {
    unsavedConfirmActionRef.current = "save";
    setIsUnsavedConfirmOpen(false);
    setIsCloseSaveDialogOpen(true);
  }, []);

  const handleCloseSaveDialogOpenChange = useCallback(
    (open: boolean) => {
      setIsCloseSaveDialogOpen(open);
      if (open) return;
      if (closeSaveCompletedRef.current) {
        closeSaveCompletedRef.current = false;
        return;
      }
      resetCloseFlow();
    },
    [resetCloseFlow],
  );

  const handleCloseFlowSave = useCallback(
    async (name: string, description: string) => {
      if (!currentCloseTabId) {
        resetCloseFlow();
        return;
      }

      const currentTab = tabs.find((t) => t.id === currentCloseTabId);
      if (!currentTab || currentTab.type !== "editor") {
        closeSaveCompletedRef.current = true;
        const currentIndex = pendingCloseTabIds.indexOf(currentCloseTabId);
        const rest =
          currentIndex >= 0
            ? pendingCloseTabIds.slice(currentIndex + 1)
            : pendingCloseTabIds.filter((id) => id !== currentCloseTabId);
        continueCloseFlow(rest);
        return;
      }

      await saveEditorTab(currentTab, name, description);

      closeSaveCompletedRef.current = true;
      closeTabNow(currentCloseTabId);
      const currentIndex = pendingCloseTabIds.indexOf(currentCloseTabId);
      const rest =
        currentIndex >= 0
          ? pendingCloseTabIds.slice(currentIndex + 1)
          : pendingCloseTabIds.filter((id) => id !== currentCloseTabId);
      continueCloseFlow(rest);
    },
    [
      closeTabNow,
      continueCloseFlow,
      currentCloseTabId,
      pendingCloseTabIds,
      resetCloseFlow,
      saveEditorTab,
      tabs,
    ],
  );

  const handleCycleTabs = (direction: 1 | -1) => {
    if (tabs.length < 2) return;
    const currentIndex = tabs.findIndex((t) => t.id === activeTab);
    const startIndex = currentIndex >= 0 ? currentIndex : 0;
    const nextIndex = (startIndex + direction + tabs.length) % tabs.length;
    setActiveTab(tabs[nextIndex].id);
  };

  // Global Keyboard Shortcuts
  useEffect(() => {
    const handleGlobalKeyDown = (e: KeyboardEvent) => {
      if (!isModKey(e) || shouldIgnoreGlobalShortcut(e)) return;

      if (e.shiftKey && e.code === "BracketRight") {
        e.preventDefault();
        handleCycleTabs(1);
        return;
      }

      if (e.shiftKey && e.code === "BracketLeft") {
        e.preventDefault();
        handleCycleTabs(-1);
        return;
      }

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
              currentTab.driver,
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
  }, [activeTab, tabs]);

  const activeTabItem = tabs.find((t) => t.id === activeTab);
  const activeTableTarget = useMemo<ActiveTableTarget | undefined>(() => {
    if (!activeTabItem) return undefined;

    if (
      (activeTabItem.type === "table" || activeTabItem.type === "ddl") &&
      activeTabItem.connectionId &&
      activeTabItem.database &&
      activeTabItem.tableName
    ) {
      return {
        connectionId: activeTabItem.connectionId,
        database: activeTabItem.database,
        table: activeTabItem.tableName,
        schema: activeTabItem.schema,
      };
    }

    return undefined;
  }, [activeTabItem]);
  const tableTabTitleCounts = useMemo(() => {
    const counts = new Map<string, number>();
    tabs.forEach((tab) => {
      if (tab.type !== "table") return;
      counts.set(tab.title, (counts.get(tab.title) || 0) + 1);
    });
    return counts;
  }, [tabs]);
  const currentCloseTab = currentCloseTabId
    ? tabs.find((t) => t.id === currentCloseTabId)
    : undefined;

  return (
    <div className="h-screen w-screen flex flex-col bg-muted/30">
      {!isFullscreen && (
        <div
          data-tauri-drag-region
          className="relative h-9 bg-background border-b border-border flex items-center pl-20 pr-2 select-none cursor-grab active:cursor-grabbing"
          onMouseDown={handleWindowDragStart}
        >
          <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
            <span className="text-xs font-medium text-muted-foreground">
              DbPaw
            </span>
          </div>
          <div
            data-no-drag="true"
            className="ml-auto flex items-center gap-1 shrink-0"
          >
            {renderWindowActions()}
          </div>
        </div>
      )}

      {/* Main Content */}
      <div className="flex-1 overflow-hidden">
        <ResizablePanelGroup
          direction="horizontal"
          autoSaveId={aiVisible ? "main-layout-with-ai" : "main-layout"}
        >
          {/* Left Sidebar - Database Connections */}
          <ResizablePanel
            id="left-sidebar"
            order={1}
            defaultSize={20}
            minSize={15}
            maxSize={30}
          >
            <Sidebar
              onTableSelect={handleTableSelect}
              onConnect={() => {}}
              onCreateQuery={handleCreateQuery}
              onExportTable={handleExportTableFromTree}
              onSelectSavedQuery={handleOpenSavedQuery}
              lastUpdated={queriesLastUpdated}
              activeTableTarget={activeTableTarget}
            />
          </ResizablePanel>

          <ResizableHandle />

          {/* Main Panel - SQL Editor & Results */}
          <ResizablePanel
            id="main-panel"
            order={2}
            defaultSize={60}
            minSize={40}
          >
            <Tabs
              value={activeTab}
              onValueChange={setActiveTab}
              className="h-full flex flex-col"
            >
              <div className="bg-background border-b border-border flex items-center h-9">
                <div className="min-w-0 flex-1">
                  <TabsList className="h-9 min-w-0 w-full justify-start gap-0 bg-transparent border-none p-0 overflow-x-auto">
                    <DndContext
                      sensors={sensors}
                      collisionDetection={closestCenter}
                      onDragEnd={handleDragEnd}
                    >
                      <SortableContext
                        items={tabs.map((t) => t.id)}
                        strategy={horizontalListSortingStrategy}
                      >
                        {tabs.map((tab) => {
                          const title =
                            tab.type === "table" &&
                            (tableTabTitleCounts.get(tab.title) || 0) > 1 &&
                            tab.database
                              ? `${tab.database}.${tab.title}`
                              : tab.title;
                          return (
                            <SortableTab key={tab.id} id={tab.id}>
                            <ContextMenu>
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
                                    <span className="max-w-[120px] flex items-center">
                                      <span className="truncate">
                                        {title}
                                      </span>
                                      {tab.type === "editor" && tab.isDirty && (
                                        <span
                                          className="inline-block w-1.5 h-1.5 rounded-full bg-amber-500 ml-1 shrink-0"
                                          aria-label={t("app.tab.unsavedChanges")}
                                        />
                                      )}
                                    </span>
                                    <button
                                      type="button"
                                      aria-label={t("app.tab.closeAria", { title })}
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
                                <ContextMenuItem
                                  onClick={() => handleCloseTab(tab.id)}
                                >
                                  {t("app.tab.closeTab")}
                                </ContextMenuItem>
                                <ContextMenuItem
                                  onClick={() => handleCloseOtherTabs(tab.id)}
                                >
                                  {t("app.tab.closeOtherTabs")}
                                </ContextMenuItem>
                              </ContextMenuContent>
                            </ContextMenu>
                          </SortableTab>
                          );
                        })}
                      </SortableContext>
                    </DndContext>
                  </TabsList>
                </div>
                {isFullscreen && (
                  <div
                    data-no-drag="true"
                    className="flex items-center gap-1 shrink-0 pr-2"
                  >
                    {renderWindowActions()}
                  </div>
                )}
              </div>

              <div className="flex-1 overflow-hidden">
                {tabs.length === 0 ? (
                  <div className="h-full flex items-center justify-center text-muted-foreground">
                    <div className="text-center">
                      <FileCode className="w-12 h-12 mx-auto mb-2 opacity-50" />
                      <p>
                        {t("app.empty.hint")}
                      </p>
                    </div>
                  </div>
                ) : (
                  tabs.map((tab) => (
                    <TabsContent
                      key={tab.id}
                      value={tab.id}
                      className="h-full m-0"
                    >
                      {tab.type === "editor" ? (
                        <Suspense
                          fallback={
                            <LazyPanelFallback
                              label={t("common.loading")}
                            />
                          }
                        >
                          <SqlEditor
                            databaseName={tab.database}
                            availableDatabases={tab.availableDatabases}
                            onExecute={(sql) => handleExecuteQuery(tab.id, sql)}
                            onCancel={() =>
                              tab.connectionId && tab.activeQueryId
                                ? api.query.cancel(
                                    String(tab.connectionId),
                                    tab.activeQueryId,
                                  )
                                : Promise.resolve(false)
                            }
                            queryResults={tab.queryResults}
                            value={tab.sqlContent}
                            onChange={(sql) => handleSqlChange(tab.id, sql)}
                            onDatabaseChange={(database) =>
                              void handleEditorDatabaseChange(tab.id, database)
                            }
                            connectionId={tab.connectionId}
                            driver={tab.driver}
                            schemaOverview={tab.schemaOverview}
                            savedQueryId={tab.savedQueryId}
                            initialName={
                              isDefaultQueryTitle(tab.title) ? "" : tab.title
                            }
                            initialDescription={tab.savedQueryDescription}
                            onSaveSuccess={(savedQuery) => {
                              setQueriesLastUpdated(Date.now());
                              setTabs((prev) =>
                                prev.map((t) => {
                                  if (t.id === tab.id) {
                                    return {
                                      ...t,
                                      savedQueryId: savedQuery.id,
                                      title: savedQuery.name,
                                      savedQueryDescription:
                                        savedQuery.description || undefined,
                                      sqlContent: savedQuery.query,
                                      lastSavedSql: savedQuery.query,
                                      isDirty: false,
                                    };
                                  }
                                  return t;
                                }),
                              );
                            }}
                          />
                        </Suspense>
                      ) : tab.type === "table" ? (
                        <TableView
                          data={tab.data}
                          columns={tab.columns}
                          total={tab.total}
                          page={tab.page}
                          pageSize={tab.pageSize}
                          executionTimeMs={tab.executionTimeMs}
                          onPageChange={(p) => handlePageChange(tab.id, p)}
                          onPageSizeChange={(size) =>
                            handlePageSizeChange(tab.id, size)
                          }
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
                          onDataRefresh={(params) =>
                            handleTableRefresh(tab.id, params)
                          }
                          tableContext={
                            tab.connectionId &&
                            tab.database &&
                            tab.tableName &&
                            tab.driver
                              ? {
                                  connectionId: tab.connectionId,
                                  database: tab.database,
                                  schema:
                                    tab.driver === "mysql" ||
                                    tab.driver === "tidb" ||
                                    tab.driver === "mariadb" ||
                                    tab.driver === "clickhouse"
                                      ? tab.database
                                      : tab.driver === "mssql"
                                        ? "dbo"
                                        : tab.driver === "duckdb"
                                          ? "main"
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
                  ))
                )}
              </div>
            </Tabs>
          </ResizablePanel>

          <ResizableHandle />

          {/* Right Sidebar - AI Assistant */}
          {aiVisible && (
            <ResizablePanel
              id="ai-sidebar"
              order={3}
              defaultSize={20}
              minSize={20}
              maxSize={40}
            >
              <Suspense
                fallback={
                  <LazyPanelFallback
                    label={t("common.loading")}
                  />
                }
              >
                <AISidebar
                  connectionId={activeTabItem?.connectionId}
                  database={activeTabItem?.database}
                  schemaOverview={activeTabItem?.schemaOverview}
                />
              </Suspense>
            </ResizablePanel>
          )}
        </ResizablePanelGroup>
      </div>
      <AlertDialog
        open={isUnsavedConfirmOpen}
        onOpenChange={(open) => {
          if (!open && unsavedConfirmActionRef.current) {
            unsavedConfirmActionRef.current = null;
            setIsUnsavedConfirmOpen(false);
            return;
          }
          if (!open && isUnsavedConfirmOpen) {
            handleUnsavedCloseCancel();
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("app.dialog.unsavedTitle")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("app.dialog.unsavedDescription")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={handleUnsavedCloseCancel}>
              {t("common.cancel")}
            </AlertDialogCancel>
            <AlertDialogAction onClick={handleUnsavedCloseWithoutSave}>
              {t("app.dialog.dontSave")}
            </AlertDialogAction>
            <AlertDialogAction onClick={handleUnsavedCloseSave}>
              {t("common.save")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <SaveQueryDialog
        open={isCloseSaveDialogOpen}
        onOpenChange={handleCloseSaveDialogOpenChange}
        onSave={handleCloseFlowSave}
        initialName={
          currentCloseTab && !isDefaultQueryTitle(currentCloseTab.title)
            ? currentCloseTab.title
            : ""
        }
        initialDescription={currentCloseTab?.savedQueryDescription}
      />
      {openSettings && (
        <Suspense fallback={null}>
          <SettingsDialog open={openSettings} onOpenChange={setOpenSettings} />
        </Suspense>
      )}
      <UpdaterChecker />
    </div>
  );
}
