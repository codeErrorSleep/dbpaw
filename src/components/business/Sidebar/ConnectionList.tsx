import { useEffect, useMemo, useState, type FormEvent } from "react";
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
  Search,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
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
import { Checkbox } from "@/components/ui/checkbox";
import { api } from "@/services/api";
import type { ConnectionForm, Driver } from "@/services/api";

interface Column {
  name: string;
  type: string;
  isPrimaryKey?: boolean;
  nullable?: boolean;
}

interface TableInfo {
  name: string;
  schema: string;
  columns: Column[];
}

interface DatabaseInfo {
  name: string;
  tables: TableInfo[];
}

interface Connection {
  id: string;
  name: string;
  type: Driver;
  host: string;
  port: string;
  database?: string;
  username: string;
  ssl?: boolean;
  filePath?: string;
  sshEnabled?: boolean;
  sshHost?: string;
  sshPort?: number;
  sshUsername?: string;
  sshPassword?: string;
  sshKeyPath?: string;
  databases: DatabaseInfo[];
  isConnected: boolean;
}

const defaultForm: ConnectionForm = {
  driver: "postgres",
  name: "My Database",
  host: "localhost",
  port: 5432,
  database: "",
  schema: "public",
  username: "",
  password: "",
  ssl: false,
  sshEnabled: false,
  sshPort: 22,
  sshUsername: "root",
};

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

interface ConnectionListProps {
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

export function ConnectionList({
  onTableSelect,
  onConnect,
  onCreateQuery,
}: ConnectionListProps) {
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
  const [dialogMode, setDialogMode] = useState<"create" | "edit">("create");
  const [editingConnectionId, setEditingConnectionId] = useState<string | null>(null);
  const [isTesting, setIsTesting] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [isSavingEdit, setIsSavingEdit] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [deleteTargetConnectionId, setDeleteTargetConnectionId] = useState<string | null>(null);
  const [testMsg, setTestMsg] = useState<{
    ok: boolean;
    text: string;
    latency?: number;
  } | null>(null);
  const [validationMsg, setValidationMsg] = useState<string | null>(null);
  const [form, setForm] = useState<ConnectionForm>(defaultForm);
  const [searchTerm, setSearchTerm] = useState("");

  const filteredConnections = useMemo(() => {
    if (!searchTerm) return connections;
    const lowerTerm = searchTerm.toLowerCase();
    return connections
      .map((conn) => {
        const filteredDbs = conn.databases
          .map((db) => {
            const filteredTables = db.tables.filter((t) =>
              t.name.toLowerCase().includes(lowerTerm),
            );
            if (filteredTables.length > 0) {
              return { ...db, tables: filteredTables };
            }
            return null;
          })
          .filter(Boolean) as DatabaseInfo[];

        if (filteredDbs.length > 0) {
          return { ...conn, databases: filteredDbs };
        }
        return null;
      })
      .filter(Boolean) as Connection[];
  }, [connections, searchTerm]);

  useEffect(() => {
    if (searchTerm) {
      const newExpandedConns = new Set(expandedConnections);
      const newExpandedDbs = new Set(expandedDatabases);
      filteredConnections.forEach((conn) => {
        newExpandedConns.add(conn.id);
        conn.databases.forEach((db) => {
          newExpandedDbs.add(`${conn.id}-${db.name}`);
        });
      });
      setExpandedConnections(newExpandedConns);
      setExpandedDatabases(newExpandedDbs);
    }
  }, [searchTerm, filteredConnections]);

  const isSqlite = form.driver === "sqlite";
  const requiredOk = useMemo(() => {
    if (isSqlite) return !!form.filePath;
    const hasBasic = !!form.host && !!form.port && !!form.username;
    if (dialogMode === "edit") return hasBasic;
    return hasBasic && !!form.password;
  }, [form, isSqlite, dialogMode]);

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
          type: (c.dbType as Driver) || "postgres",
          host: c.host || "",
          port: String(c.port || ""),
          database: c.database || "",
          username: c.username || "",
          ssl: c.ssl || false,
          filePath: c.filePath || "",
          sshEnabled: c.sshEnabled || false,
          sshHost: c.sshHost || "",
          sshPort: c.sshPort || 22,
          sshUsername: c.sshUsername || "root",
          sshPassword: c.sshPassword || "",
          sshKeyPath: c.sshKeyPath || "",
          isConnected: false,
          databases: [],
        })),
      );
      setExpandedConnections(new Set());
      setExpandedDatabases(new Set());
    } catch (e) {
      console.error("listConnections failed", e instanceof Error ? e.message : String(e));
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
      console.error("listDatabasesById failed", e instanceof Error ? e.message : String(e));
    }
  };

  const fetchAndSetTables = async (
    connectionId: string,
    databaseName: string,
  ) => {
    try {
      // Use listTables to get table list by ID, passing the currently selected database
      // For Postgres, databaseName usually corresponds to database field, schema might be public or others
      // Simplified handling: pass databaseName to database parameter
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
                tables: tables.map((t) => ({ name: t.name, schema: t.schema, columns: [] })),
              };
            }),
          };
        }),
      );
    } catch (e) {
      console.error("listTables failed", e instanceof Error ? e.message : String(e));
    }
  };

  const toggleDatabase = (key: string) => {
    const newExpanded = new Set(expandedDatabases);
    if (newExpanded.has(key)) {
      newExpanded.delete(key);
    } else {
      newExpanded.add(key);
      // When expanding, try to load tables (if not loaded)
      // Key format is "connectionId-dbName"
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
  const fetchAndSetTableColumns = async (
    connectionId: string,
    databaseName: string,
    schema: string,
    tableName: string,
  ) => {
    try {
      const metadata = await api.metadata.getTableMetadata(
        Number(connectionId),
        databaseName,
        schema,
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
                  if (t.columns.length > 0) return t;
                  return {
                    ...t,
                    columns: metadata.columns.map((c) => ({
                      name: c.name,
                      type: c.type,
                      isPrimaryKey: c.primaryKey,
                      nullable: c.nullable,
                    })),
                  };
                }),
              };
            }),
          };
        }),
      );
    } catch (e) {
      console.error("getTableMetadata failed", e instanceof Error ? e.message : String(e));
    }
  };

  const toggleTable = (
    tableKey: string,
    connectionId: string,
    databaseName: string,
    table: TableInfo,
  ) => {
    const newExpanded = new Set(expandedTables);
    if (newExpanded.has(tableKey)) {
      newExpanded.delete(tableKey);
    } else {
      newExpanded.add(tableKey);
      // Load column info on first expand
      if (table.columns.length === 0) {
        fetchAndSetTableColumns(connectionId, databaseName, table.schema, table.name);
      }
    }
    setExpandedTables(newExpanded);
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

  const handleTestConnection = async () => {
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
  };

  const handleConnect = async () => {
    if (!requiredOk) {
      const requiredFields = isSqlite
        ? "File path"
        : "Host, Port, Username, Password";
      setValidationMsg(`Please fill in required fields: ${requiredFields}`);
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
          type: (res.dbType as Driver) || "postgres",
          host: res.host || "",
          port: String(res.port || ""),
          database: res.database || "",
          username: res.username || "",
          ssl: res.ssl || false,
          filePath: res.filePath || "",
          sshEnabled: res.sshEnabled || false,
          sshHost: res.sshHost || "",
          sshPort: res.sshPort || 22,
          sshUsername: res.sshUsername || "root",
          sshPassword: "",
          sshKeyPath: res.sshKeyPath || "",
          isConnected: false,
          databases: [],
        },
        ...prev,
      ]);
      setIsDialogOpen(false);
      setForm(defaultForm);
      if (onConnect) onConnect(form);
    } catch (e: any) {
      setValidationMsg(String(e?.message || e));
    } finally {
      setIsConnecting(false);
    }
  };

  const handleSaveEdit = async () => {
    if (!editingConnectionId) return;
    if (!requiredOk) {
      const requiredFields = isSqlite
        ? "File path"
        : "Host, Port, Username";
      setValidationMsg(`Please fill in required fields: ${requiredFields}`);
      return;
    }

    setValidationMsg(null);
    setIsSavingEdit(true);
    try {
      await api.connections.update(Number(editingConnectionId), form);
      await fetchConnections();
      setIsDialogOpen(false);
      setDialogMode("create");
      setEditingConnectionId(null);
      setForm(defaultForm);
    } catch (e: any) {
      setValidationMsg(String(e?.message || e));
    } finally {
      setIsSavingEdit(false);
    }
  };

  const handleDialogSubmit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (dialogMode === "edit") {
      void handleSaveEdit();
      return;
    }
    void handleConnect();
  };

  const openCreateDialog = () => {
    setDialogMode("create");
    setEditingConnectionId(null);
    setValidationMsg(null);
    setTestMsg(null);
    setForm(defaultForm);
    setIsDialogOpen(true);
  };

  const openEditDialog = (connectionId: string) => {
    const conn = connections.find((c) => c.id === connectionId);
    if (!conn) return;

    setDialogMode("edit");
    setEditingConnectionId(connectionId);
    setValidationMsg(null);
    setTestMsg(null);
    setForm({
      driver: conn.type,
      name: conn.name,
      host: conn.host || "",
      port: Number(conn.port) || undefined,
      database: conn.database || "",
      schema: conn.type === "postgres" ? "public" : "",
      username: conn.username || "",
      password: "",
      ssl: conn.ssl || false,
      filePath: conn.filePath || "",
      sshEnabled: conn.sshEnabled || false,
      sshHost: conn.sshHost || "",
      sshPort: conn.sshPort || 22,
      sshUsername: conn.sshUsername || "root",
      sshPassword: "",
      sshKeyPath: conn.sshKeyPath || "",
    });
    setIsDialogOpen(true);
  };

  const handleReconnect = async (connectionId: string) => {
    setConnections((prev) =>
      prev.map((conn) => {
        if (conn.id !== connectionId) return conn;
        return { ...conn, isConnected: false, databases: [] };
      }),
    );
    setExpandedDatabases((prev) => {
      const next = new Set(
        [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
      );
      return next;
    });
    setExpandedTables((prev) => {
      const next = new Set(
        [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
      );
      return next;
    });
    await fetchAndSetDatabases(connectionId);
  };

  const handleDeleteConnection = async (connectionId: string) => {
    setIsDeleting(true);
    try {
      await api.connections.delete(Number(connectionId));
      setConnections((prev) => prev.filter((conn) => conn.id !== connectionId));
      setExpandedConnections((prev) => {
        const next = new Set(prev);
        next.delete(connectionId);
        return next;
      });
      setExpandedDatabases((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
      setExpandedTables((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
      setDeleteTargetConnectionId(null);
    } catch (e) {
      console.error("deleteConnection failed", e instanceof Error ? e.message : String(e));
    } finally {
      setIsDeleting(false);
    }
  };

  return (
    <div className="h-full flex flex-col bg-background border-r border-border">
      <div className="px-2 py-1 border-b border-border flex items-center justify-between h-8">
        <h2 className="font-semibold text-sm">Connections</h2>
        <div className="flex gap-1">
          <Button
            variant="ghost"
            size="sm"
            className="h-6 w-6 p-0"
            onClick={fetchConnections}
          >
            <RefreshCw className="w-3.5 h-3.5" />
          </Button>
          <Dialog
            open={isDialogOpen}
            onOpenChange={(open) => {
              setIsDialogOpen(open);
              if (!open) {
                setValidationMsg(null);
                setTestMsg(null);
              }
            }}
          >
            <DialogTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                className="h-6 w-6 p-0"
                onClick={openCreateDialog}
              >
                <Plus className="w-3.5 h-3.5" />
              </Button>
            </DialogTrigger>
            <DialogContent>
              <form onSubmit={handleDialogSubmit}>
                <DialogHeader>
                  <DialogTitle>
                    {dialogMode === "edit"
                      ? "Edit Database Connection"
                      : "New Database Connection"}
                  </DialogTitle>
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
                          Password{" "}
                          {dialogMode === "create" && (
                            <span className="text-red-600">*</span>
                          )}
                        </Label>
                        <Input
                          id="password"
                          type="password"
                          placeholder={
                            dialogMode === "edit"
                              ? "Leave empty to keep current password"
                              : undefined
                          }
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
                    <div className="flex items-center space-x-2">
                      <Checkbox
                        id="ssl"
                        checked={form.ssl}
                        onCheckedChange={(checked) =>
                          setForm((f) => ({ ...f, ssl: checked === true }))
                        }
                      />
                      <Label htmlFor="ssl">SSL</Label>
                    </div>

                    <div className="flex items-center space-x-2">
                      <Checkbox
                        id="ssh"
                        checked={form.sshEnabled}
                        onCheckedChange={(checked) =>
                          setForm((f) => ({ ...f, sshEnabled: checked === true }))
                        }
                      />
                      <Label htmlFor="ssh">SSH</Label>
                    </div>

                    {form.sshEnabled && (
                      <div className="border p-3 rounded-md space-y-3 bg-muted/20">
                        <div className="grid grid-cols-2 gap-2">
                          <div className="grid gap-2">
                            <Label htmlFor="sshHost">SSH Host</Label>
                            <Input
                              id="sshHost"
                              placeholder="ssh.example.com"
                              value={form.sshHost || ""}
                              onChange={(e) =>
                                setForm((f) => ({ ...f, sshHost: e.target.value }))
                              }
                            />
                          </div>
                          <div className="grid gap-2">
                            <Label htmlFor="sshPort">SSH Port</Label>
                            <Input
                              id="sshPort"
                              placeholder="22"
                              value={String(form.sshPort || "")}
                              onChange={(e) =>
                                setForm((f) => ({
                                  ...f,
                                  sshPort: Number(e.target.value) || undefined,
                                }))
                              }
                            />
                          </div>
                        </div>
                        <div className="grid gap-2">
                          <Label htmlFor="sshUsername">SSH Username</Label>
                          <Input
                            id="sshUsername"
                            placeholder="root"
                            value={form.sshUsername || ""}
                            onChange={(e) =>
                              setForm((f) => ({
                                ...f,
                                sshUsername: e.target.value,
                              }))
                            }
                          />
                        </div>
                        <div className="grid gap-2">
                          <Label htmlFor="sshPassword">SSH Password</Label>
                          <Input
                            id="sshPassword"
                            type="password"
                            placeholder="Optional if using key"
                            value={form.sshPassword || ""}
                            onChange={(e) =>
                              setForm((f) => ({
                                ...f,
                                sshPassword: e.target.value,
                              }))
                            }
                          />
                        </div>
                        <div className="grid gap-2">
                          <Label htmlFor="sshKeyPath">SSH Key Path</Label>
                          <Input
                            id="sshKeyPath"
                            placeholder="/path/to/private_key"
                            value={form.sshKeyPath || ""}
                            onChange={(e) =>
                              setForm((f) => ({
                                ...f,
                                sshKeyPath: e.target.value,
                              }))
                            }
                          />
                        </div>
                      </div>
                    )}
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
                    type="button"
                    variant="outline"
                    onClick={() => setIsDialogOpen(false)}
                  >
                    Cancel
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    onClick={handleTestConnection}
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
                    type="submit"
                    disabled={(dialogMode === "edit" ? isSavingEdit : isConnecting) || !requiredOk}
                  >
                  {dialogMode === "edit" ? (
                    isSavingEdit ? (
                      <>
                        <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                        Saving…
                      </>
                    ) : (
                      "Save"
                    )
                  ) : isConnecting ? (
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
                      <AlertTitle>Validation Failed</AlertTitle>
                      <AlertDescription>{validationMsg}</AlertDescription>
                    </Alert>
                  </div>
                )}
                {testMsg && (
                  <div className="mt-3">
                    <Alert variant={testMsg.ok ? "default" : "destructive"}>
                      <AlertTitle>
                        {testMsg.ok ? "Connection Test Successful" : "Connection Test Failed"}
                      </AlertTitle>
                      <AlertDescription>
                        {testMsg.text}
                        {testMsg.latency ? `(${testMsg.latency}ms)` : ""}
                      </AlertDescription>
                    </Alert>
                  </div>
                )}
              </form>
            </DialogContent>
          </Dialog>
        </div>
      </div>

      <div className="p-2 border-b border-border">
        <div className="relative">
          <Search className="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search tables..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="pl-8"
          />
        </div>
      </div>

      <div
        className="flex-1 overflow-auto"
        onClick={() => setContextMenu((prev) => ({ ...prev, visible: false }))}
      >
        {filteredConnections.map((connection) => (
          <TreeNode
            key={connection.id}
            level={0}
            icon={<Server className="w-4 h-4" />}
            label={connection.name}
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
                              toggleTable(tableKey, connection.id, database.name, table);
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
                            {table.columns.map((column) => (
                              <div
                                key={column.name}
                                className="flex items-center gap-1 px-2 py-1 hover:bg-accent text-xs"
                                style={{ paddingLeft: `${3 * 12 + 8}px` }}
                              >
                                <span className="w-4" />
                                {column.isPrimaryKey ? (
                                  <Key className="w-3 h-3 text-yellow-600 shrink-0" />
                                ) : (
                                  <span className="w-3 shrink-0" />
                                )}
                                <span className="flex-1 truncate text-foreground">
                                  {column.name}
                                </span>
                                <span className="text-muted-foreground text-xs shrink-0">
                                  {column.type}
                                </span>
                              </div>
                            ))}
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
                onClick={async () => {
                  if (contextMenu.connectionId) {
                    openEditDialog(contextMenu.connectionId);
                  }
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <Edit3 className="w-4 h-4" />
                Edit
              </button>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={async () => {
                  if (contextMenu.connectionId) {
                    await handleReconnect(contextMenu.connectionId);
                  }
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <Plug className="w-4 h-4" />
                Reconnect
              </button>
              <div className="h-px bg-border my-1" />
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent text-destructive flex items-center gap-2"
                onClick={() => {
                  if (contextMenu.connectionId) {
                    setDeleteTargetConnectionId(contextMenu.connectionId);
                  }
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <Trash2 className="w-4 h-4" />
                Delete
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
              New Query
            </button>
          )}
        </div>
      )}
      <AlertDialog
        open={!!deleteTargetConnectionId}
        onOpenChange={(open) => {
          if (!open) {
            setDeleteTargetConnectionId(null);
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete Connection</AlertDialogTitle>
            <AlertDialogDescription>
              This action cannot be undone. The selected connection configuration
              will be removed.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={isDeleting}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              disabled={isDeleting || !deleteTargetConnectionId}
              onClick={async (e) => {
                e.preventDefault();
                if (!deleteTargetConnectionId) return;
                await handleDeleteConnection(deleteTargetConnectionId);
              }}
            >
              {isDeleting ? "Deleting..." : "Delete"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
