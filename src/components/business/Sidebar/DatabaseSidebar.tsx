import { useEffect, useMemo, useState } from "react";
import {
  Database,
  Server,
  ChevronRight,
  ChevronDown,
  Table,
  Key,
  Plus,
  RefreshCw,
  Play,
  Loader2,
  Edit3,
  Plug,
  Trash2,
  FileCode,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api } from "@/services/api";
import type { ConnectionForm, Driver } from "@/services/api";

interface Column {
  name: string;
  type: string;
  isPrimaryKey?: boolean;
}

interface TableInfo {
  name: string;
  columns: Column[];
}

interface DatabaseInfo {
  name: string;
  tables: TableInfo[];
}

interface Connection {
  id: string;
  name: string;
  type: "postgresql" | "mysql" | "mongodb" | "sqlite";
  host: string;
  port: string;
  username: string;
  databases: DatabaseInfo[];
  isConnected: boolean;
}

const DEFAULT_UUID = "dbpaw-default";

interface TreeNodeProps {
  level: number;
  children: React.ReactNode;
  icon: React.ReactNode;
  label: string;
  isExpanded?: boolean;
  onToggle?: () => void;
  onDoubleClick?: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  actions?: React.ReactNode;
}

const TreeNode = ({
  level,
  children,
  icon,
  label,
  isExpanded,
  onToggle,
  onDoubleClick,
  onContextMenu,
  actions,
}: TreeNodeProps) => {
  const hasChildren = children !== null && children !== undefined;

  return (
    <div>
      <div
        className="flex items-center gap-1 px-2 py-1 hover:bg-accent cursor-pointer group select-none"
        style={{ paddingLeft: `${level * 12 + 8}px` }}
        onClick={onToggle}
        onDoubleClick={onDoubleClick}
        onContextMenu={onContextMenu}
      >
        {hasChildren && (
          <span className="text-muted-foreground">
            {isExpanded ? (
              <ChevronDown className="w-4 h-4" />
            ) : (
              <ChevronRight className="w-4 h-4" />
            )}
          </span>
        )}
        {!hasChildren && <span className="w-4" />}
        <span className="text-muted-foreground">{icon}</span>
        <span className="flex-1 text-sm truncate">{label}</span>
        {actions && (
          <span className="opacity-0 group-hover:opacity-100">{actions}</span>
        )}
      </div>
      {isExpanded && children}
    </div>
  );
};

interface DatabaseSidebarProps {
  onTableSelect?: (
    connection: string,
    database: string,
    table: string,
    connectionId: number,
    driver: string,
  ) => void;
  onConnect?: (form: ConnectionForm) => void;
  onCreateQuery?: (
    connectionId: number,
    databaseName: string,
    driver: string,
  ) => void;
}

export function DatabaseSidebar({
  onTableSelect,
  onConnect,
  onCreateQuery,
}: DatabaseSidebarProps) {
  const [connections, setConnections] = useState<Connection[]>([]);
  const [expandedConnections, setExpandedConnections] = useState<Set<string>>(
    new Set(["1"]),
  );
  const [expandedDatabases, setExpandedDatabases] = useState<Set<string>>(
    new Set(),
  );
  const [expandedTables, setExpandedTables] = useState<Set<string>>(new Set());
  const [contextMenu, setContextMenu] = useState<{
    visible: boolean;
    x: number;
    y: number;
    connectionId: string | null;
    databaseName?: string | null;
    type: "connection" | "database";
  }>({ visible: false, x: 0, y: 0, connectionId: null, type: "connection" });
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [testMsg, setTestMsg] = useState<{
    ok: boolean;
    text: string;
    latency?: number;
  } | null>(null);
  const [validationMsg, setValidationMsg] = useState<string | null>(null);
  const [form, setForm] = useState<ConnectionForm>({
    driver: "postgres",
    name: "My Database",
    host: "localhost",
    port: 5432,
    database: "",
    schema: "public",
    username: "",
    password: "",
    ssl: false,
  });
  const isSqlite = form.driver === "sqlite";
  const requiredOk = useMemo(() => {
    if (isSqlite) return !!form.filePath;
    // Database 不再必填，允许连接服务器后列出所有库
    return !!form.host && !!form.port && !!form.username && !!form.password;
  }, [form, isSqlite]);

  useEffect(() => {
    fetchConnections();
  }, []);

  const fetchConnections = async () => {
    try {
      const conns = await api.connections.list();
      setConnections(
        conns.map((c) => ({
          id: String(c.id),
          name: c.name || "Unknown",
          type: c.dbType,
          host: c.host,
          port: String(c.port),
          username: c.username,
          isConnected: false,
          databases: [],
        })),
      );
      setExpandedConnections(new Set());
      setExpandedDatabases(new Set());
    } catch (e) {
      console.error("listConnections failed", e);
    }
  };

  const toggleConnection = (id: string) => {
    const newExpanded = new Set(expandedConnections);
    if (newExpanded.has(id)) {
      newExpanded.delete(id);
    } else {
      newExpanded.add(id);
      const conn = connections.find((c) => c.id === id);
      if (conn && !conn.isConnected) {
        fetchAndSetDatabases(id);
      }
    }
    setExpandedConnections(newExpanded);
  };

  const fetchAndSetDatabases = async (connectionId: string) => {
    try {
      const dbNames = await api.metadata.listDatabasesById(
        Number(connectionId),
      );
      setConnections((prev) =>
        prev.map((conn) => {
          if (conn.id !== connectionId) return conn;
          return {
            ...conn,
            isConnected: true,
            databases: dbNames.map((name) => ({
              name,
              tables: [],
            })),
          };
        }),
      );
    } catch (e) {
      console.error("listDatabasesById failed", e);
    }
  };

  const fetchAndSetTables = async (
    connectionId: string,
    databaseName: string,
  ) => {
    try {
      // 使用 listTables 通过 ID 获取表列表，传入当前选中的 database
      // 对于 Postgres，databaseName 通常对应 database 字段，schema 可能是 public 或其他
      // 这里简化处理：将 databaseName 传给 database 参数
      const tables = await api.metadata.listTables(
        Number(connectionId),
        databaseName,
      );
      setConnections((prev) =>
        prev.map((conn) => {
          if (conn.id !== connectionId) return conn;
          return {
            ...conn,
            databases: conn.databases.map((db) => {
              if (db.name !== databaseName) return db;
              if (db.tables.length > 0) return db;
              return {
                ...db,
                tables: tables.map((t) => ({ name: t.name, columns: [] })),
              };
            }),
          };
        }),
      );
    } catch (e) {
      console.error("listTables failed", e);
    }
  };

  const toggleDatabase = (key: string) => {
    const newExpanded = new Set(expandedDatabases);
    if (newExpanded.has(key)) {
      newExpanded.delete(key);
    } else {
      newExpanded.add(key);
      // 展开时，尝试加载表（如果未加载）
      // key 格式为 "connectionId-dbName"
      const [connId, ...dbNameParts] = key.split("-");
      const dbName = dbNameParts.join("-");
      // 找到对应的 connection 和 database
      const conn = connections.find((c) => c.id === connId);
      if (conn) {
        const db = conn.databases.find((d) => d.name === dbName);
        if (db && db.tables.length === 0) {
          fetchAndSetTables(connId, dbName);
        }
      }
    }
    setExpandedDatabases(newExpanded);
  };

  const toggleTable = (key: string) => {
    const newExpanded = new Set(expandedTables);
    if (newExpanded.has(key)) {
      newExpanded.delete(key);
    } else {
      newExpanded.add(key);
    }
    setExpandedTables(newExpanded);
  };

  const fetchAndSetTableColumns = async (
    connectionId: string,
    databaseName: string,
    tableName: string,
  ) => {
    try {
      const structure = await api.metadata.getTableStructure(
        Number(connectionId),
        databaseName, // Use databaseName as schema for now (or whatever logic was intended)
        tableName,
      );
      setConnections((prev) =>
        prev.map((conn) => {
          if (conn.id !== connectionId) return conn;
          return {
            ...conn,
            databases: conn.databases.map((db) => {
              if (db.name !== databaseName) return db;
              return {
                ...db,
                tables: db.tables.map((t) => {
                  if (t.name !== tableName) return t;
                  return {
                    ...t,
                    columns: (structure.columns || []).map((c) => ({
                      name: c.name,
                      type: c.type,
                      isPrimaryKey: false,
                    })),
                  };
                }),
              };
            }),
          };
        }),
      );
    } catch (e) {
      console.error("getTableStructure failed", e);
    }
  };
  const handleTableClick = (
    connection: Connection,
    database: DatabaseInfo,
    table: TableInfo,
  ) => {
    if (onTableSelect) {
      onTableSelect(
        connection.name,
        database.name,
        table.name,
        Number(connection.id),
        connection.type,
      );
    }
  };

  const handleConnect = async () => {
    try {
      if (!requiredOk) return;
      const test = await api.connections.testEphemeral(form);
      if (!test.success) return;

      let dbInfos: DatabaseInfo[] = [];

      // 如果未指定 Database，则列出所有数据库
      if (!form.database) {
        const dbs = await api.metadata.listDatabases(form);
        dbInfos = dbs.map((dbName) => ({
          name: dbName,
          tables: [], // 懒加载：初始时不加载表
        }));
      } else {
        // 如果指定了 Database，直接加载该库的表
        const tables = await api.metadata.listTablesByConn(form);
        const dbName =
          form.driver === "mysql" ? form.database : form.schema || "public";
        dbInfos = [
          {
            name: dbName,
            tables: tables.map((t) => ({ name: t.name, columns: [] })),
          },
        ];
      }

      setConnections((prev) =>
        prev.map((conn) => ({
          ...conn,
          isConnected: true,
          databases: dbInfos,
        })),
      );
      setExpandedConnections(new Set(["1"]));
      // 如果只有一个库，默认展开
      if (dbInfos.length === 1) {
        setExpandedDatabases(new Set(["1-" + dbInfos[0].name]));
      } else {
        setExpandedDatabases(new Set());
      }
      setIsDialogOpen(false);
      if (onConnect) onConnect(form);
    } catch (e) {
      console.error("connect failed", e);
    }
  };

  return (
    <div className="h-full flex flex-col bg-background border-r border-border">
      <div className="p-3 border-b border-border flex items-center justify-between">
        <h2 className="font-semibold text-sm">Connections</h2>
        <div className="flex gap-1">
          <Button
            variant="ghost"
            size="sm"
            className="h-7 w-7 p-0"
            onClick={fetchConnections}
          >
            <RefreshCw className="w-4 h-4" />
          </Button>
          <Dialog open={isDialogOpen} onOpenChange={setIsDialogOpen}>
            <DialogTrigger asChild>
              <Button variant="ghost" size="sm" className="h-7 w-7 p-0">
                <Plus className="w-4 h-4" />
              </Button>
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>New Database Connection</DialogTitle>
              </DialogHeader>
              <div className="grid gap-4 py-4">
                <div className="grid gap-2">
                  <Label htmlFor="name">Connection Name</Label>
                  <Input
                    id="name"
                    placeholder="My Database"
                    value={form.name || ""}
                    onChange={(e) =>
                      setForm((f) => ({ ...f, name: e.target.value }))
                    }
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="type">Database Type</Label>
                  <Select
                    value={form.driver}
                    onValueChange={(v: Driver) =>
                      setForm((f) => ({
                        ...f,
                        driver: v,
                        port:
                          v === "postgres"
                            ? 5432
                            : v === "mysql"
                              ? 3306
                              : f.port,
                      }))
                    }
                  >
                    <SelectTrigger id="type">
                      <SelectValue placeholder="Select database type" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="postgres">PostgreSQL</SelectItem>
                      <SelectItem value="mysql">MySQL</SelectItem>
                      <SelectItem value="sqlite">SQLite</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                {!isSqlite && (
                  <>
                    <div className="grid grid-cols-2 gap-2">
                      <div className="grid gap-2">
                        <Label htmlFor="host">
                          Host <span className="text-red-600">*</span>
                        </Label>
                        <Input
                          id="host"
                          placeholder="localhost"
                          value={form.host || ""}
                          onChange={(e) =>
                            setForm((f) => ({ ...f, host: e.target.value }))
                          }
                        />
                      </div>
                      <div className="grid gap-2">
                        <Label htmlFor="port">
                          Port <span className="text-red-600">*</span>
                        </Label>
                        <Input
                          id="port"
                          placeholder={
                            form.driver === "postgres" ? "5432" : "3306"
                          }
                          value={String(form.port || "")}
                          onChange={(e) =>
                            setForm((f) => ({
                              ...f,
                              port: Number(e.target.value) || undefined,
                            }))
                          }
                        />
                      </div>
                    </div>
                    <div className="grid grid-cols-2 gap-2">
                      <div className="grid gap-2">
                        <Label htmlFor="username">
                          Username <span className="text-red-600">*</span>
                        </Label>
                        <Input
                          id="username"
                          placeholder="postgres"
                          value={form.username || ""}
                          onChange={(e) =>
                            setForm((f) => ({ ...f, username: e.target.value }))
                          }
                        />
                      </div>
                      <div className="grid gap-2">
                        <Label htmlFor="password">
                          Password <span className="text-red-600">*</span>
                        </Label>
                        <Input
                          id="password"
                          type="password"
                          value={form.password || ""}
                          onChange={(e) =>
                            setForm((f) => ({ ...f, password: e.target.value }))
                          }
                        />
                      </div>
                    </div>
                    <div className="grid grid-cols-2 gap-2">
                      <div className="grid gap-2">
                        <Label htmlFor="database">Database</Label>
                        <Input
                          id="database"
                          placeholder="mydb"
                          value={form.database || ""}
                          onChange={(e) =>
                            setForm((f) => ({ ...f, database: e.target.value }))
                          }
                        />
                      </div>
                      <div className="grid gap-2">
                        <Label htmlFor="schema">Schema</Label>
                        <Input
                          id="schema"
                          placeholder="public"
                          value={form.schema || ""}
                          onChange={(e) =>
                            setForm((f) => ({ ...f, schema: e.target.value }))
                          }
                        />
                      </div>
                    </div>
                  </>
                )}
                {isSqlite && (
                  <div className="grid gap-2">
                    <Label htmlFor="filePath">
                      SQLite File Path <span className="text-red-600">*</span>
                    </Label>
                    <Input
                      id="filePath"
                      placeholder="/path/to/db.sqlite"
                      value={form.filePath || ""}
                      onChange={(e) =>
                        setForm((f) => ({ ...f, filePath: e.target.value }))
                      }
                    />
                  </div>
                )}
              </div>
              <div className="flex justify-end gap-2">
                <Button
                  variant="outline"
                  onClick={() => setIsDialogOpen(false)}
                >
                  Cancel
                </Button>
                <Button
                  variant="outline"
                  onClick={async () => {
                    try {
                      setValidationMsg(null);
                      setIsTesting(true);
                      setTestMsg(null);
                      const res = await api.connections.testEphemeral(form);
                      setTestMsg({
                        ok: res.success,
                        text: res.message,
                        latency: res.latencyMs,
                      });
                    } catch (e: any) {
                      setTestMsg({ ok: false, text: String(e?.message || e) });
                    } finally {
                      setIsTesting(false);
                    }
                  }}
                  disabled={isTesting}
                >
                  {isTesting ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      Testing…
                    </>
                  ) : (
                    "Test"
                  )}
                </Button>
                <Button
                  onClick={async () => {
                    if (!requiredOk) {
                      setValidationMsg(
                        "请填写必填项：Host、Port、Username、Password、Database",
                      );
                      return;
                    }
                    setValidationMsg(null);
                    setIsConnecting(true);
                    try {
                      const res = await api.connections.create(form);
                      setConnections((prev) => [
                        {
                          id: String(res.id),
                          name: res.name || "Unknown",
                          type: res.dbType as any,
                          host: res.host,
                          port: String(res.port),
                          username: res.username,
                          isConnected: false,
                          databases: [],
                        },
                        ...prev,
                      ]);
                      setIsDialogOpen(false);
                      if (onConnect) onConnect(form);
                    } catch (e: any) {
                      setValidationMsg(String(e?.message || e));
                    } finally {
                      setIsConnecting(false);
                    }
                  }}
                  disabled={isConnecting || !requiredOk}
                >
                  {isConnecting ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      Connecting…
                    </>
                  ) : (
                    "Connect"
                  )}
                </Button>
              </div>
              {validationMsg && (
                <div className="mt-3">
                  <Alert variant="destructive">
                    <AlertTitle>校验失败</AlertTitle>
                    <AlertDescription>{validationMsg}</AlertDescription>
                  </Alert>
                </div>
              )}
              {testMsg && (
                <div className="mt-3">
                  <Alert variant={testMsg.ok ? "default" : "destructive"}>
                    <AlertTitle>
                      {testMsg.ok ? "连接测试成功" : "连接测试失败"}
                    </AlertTitle>
                    <AlertDescription>
                      {testMsg.text}
                      {testMsg.latency ? `（${testMsg.latency}ms）` : ""}
                    </AlertDescription>
                  </Alert>
                </div>
              )}
            </DialogContent>
          </Dialog>
        </div>
      </div>

      <div
        className="flex-1 overflow-auto"
        onClick={() => setContextMenu((prev) => ({ ...prev, visible: false }))}
      >
        {connections.map((connection) => (
          <TreeNode
            key={connection.id}
            level={0}
            icon={<Server className="w-4 h-4" />}
            label={`${connection.name} (${connection.type})`}
            isExpanded={expandedConnections.has(connection.id)}
            onToggle={() => toggleConnection(connection.id)}
            onContextMenu={(e) => {
              e.preventDefault();
              e.stopPropagation();
              setContextMenu({
                visible: true,
                x: e.clientX,
                y: e.clientY,
                connectionId: connection.id,
                type: "connection",
              });
            }}
          >
            {connection.isConnected ? (
              <>
                {connection.databases.map((database) => {
                  const dbKey = `${connection.id}-${database.name}`;
                  return (
                    <TreeNode
                      key={dbKey}
                      level={1}
                      icon={<Database className="w-4 h-4" />}
                      label={database.name}
                      isExpanded={expandedDatabases.has(dbKey)}
                      onToggle={() => toggleDatabase(dbKey)}
                      onContextMenu={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        setContextMenu({
                          visible: true,
                          x: e.clientX,
                          y: e.clientY,
                          connectionId: connection.id,
                          databaseName: database.name,
                          type: "database",
                        });
                      }}
                    >
                      {database.tables.map((table) => {
                        const tableKey = `${dbKey}-${table.name}`;
                        return (
                          <TreeNode
                            key={tableKey}
                            level={2}
                            icon={<Table className="w-4 h-4" />}
                            label={table.name}
                            isExpanded={expandedTables.has(tableKey)}
                            onToggle={() => {
                              // toggleTable(tableKey); // 禁用表展开/折叠
                              // handleTableClick(connection, database, table); // 单击不再触发打开
                              // 不再加载列信息
                              /* fetchAndSetTableColumns(
                                connection.id,
                                database.name,
                                table.name,
                              ); */
                            }}
                            onDoubleClick={() => {
                              handleTableClick(connection, database, table);
                            }}
                            actions={
                              <div onClick={(e) => e.stopPropagation()}>
                                <Button
                                  variant="ghost"
                                  size="sm"
                                  className="h-6 w-6 p-0"
                                  onClick={() =>
                                    handleTableClick(
                                      connection,
                                      database,
                                      table,
                                    )
                                  }
                                >
                                  <Play className="w-3 h-3" />
                                </Button>
                              </div>
                            }
                          >
                            /*{" "}
                            {table.columns.map((column) => (
                              <div
                                key={column.name}
                                className="flex items-center gap-1 px-2 py-1 hover:bg-gray-50 text-xs"
                                style={{ paddingLeft: `${3 * 12 + 8}px` }}
                              >
                                <span className="w-4" />
                                {column.isPrimaryKey && (
                                  <Key className="w-3 h-3 text-yellow-600" />
                                )}
                                {!column.isPrimaryKey && (
                                  <span className="w-3" />
                                )}
                                <span className="flex-1 truncate text-gray-700">
                                  {column.name}
                                </span>
                                <span className="text-gray-500 text-xs">
                                  {column.type}
                                </span>
                              </div>
                            ))}{" "}
                            */
                          </TreeNode>
                        );
                      })}
                    </TreeNode>
                  );
                })}
              </>
            ) : (
              <div
                className="px-2 py-1 text-xs text-gray-500"
                style={{ paddingLeft: "32px" }}
              >
                Not connected
              </div>
            )}
          </TreeNode>
        ))}
      </div>

      {contextMenu.visible && (
        <div
          className="fixed z-50 min-w-[140px] bg-popover border border-border rounded-md shadow-lg py-1"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          {contextMenu.type === "connection" ? (
            <>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={() => {
                  console.log("编辑连接", contextMenu.connectionId);
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <Edit3 className="w-4 h-4" />
                编辑
              </button>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={() => {
                  console.log("重新连接", contextMenu.connectionId);
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <Plug className="w-4 h-4" />
                重新连接
              </button>
              <div className="h-px bg-border my-1" />
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent text-destructive flex items-center gap-2"
                onClick={() => {
                  console.log("删除连接", contextMenu.connectionId);
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <Trash2 className="w-4 h-4" />
                删除
              </button>
            </>
          ) : (
            <button
              className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
              onClick={() => {
                if (
                  onCreateQuery &&
                  contextMenu.connectionId &&
                  contextMenu.databaseName
                ) {
                  const conn = connections.find(
                    (c) => c.id === contextMenu.connectionId,
                  );
                  const driver = conn ? conn.type : "postgres";

                  onCreateQuery(
                    Number(contextMenu.connectionId),
                    contextMenu.databaseName,
                    driver,
                  );
                }
                setContextMenu((prev) => ({ ...prev, visible: false }));
              }}
            >
              <FileCode className="w-4 h-4" />
              新建查询
            </button>
          )}
        </div>
      )}
    </div>
  );
}
