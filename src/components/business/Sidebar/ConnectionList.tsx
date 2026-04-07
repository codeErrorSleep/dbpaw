import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { save, open } from "@tauri-apps/plugin-dialog";
import { readTextFile } from "@tauri-apps/plugin-fs";
import {
  Database,
  Table,
  Key,
  Copy,
  Edit3,
  Plus,
  RefreshCw,
  Play,
  Loader2,
  Trash2,
  FileCode,
  Search,
  Download,
  FolderOpen,
  Upload,
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
import { Textarea } from "@/components/ui/textarea";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Checkbox } from "@/components/ui/checkbox";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { api, getImportDriverCapability, isTauri } from "@/services/api";
import type {
  ConnectionForm,
  CreateDatabasePayload,
  Driver,
  SavedQuery,
} from "@/services/api";
import {
  DRIVER_REGISTRY,
  getConnectionIcon,
  getDefaultPort,
  isFileBasedDriver,
  supportsSSLCA,
  isMysqlFamilyDriver,
  supportsCreateDatabase,
  supportsSchemaBrowsing,
} from "@/lib/driver-registry";
import { toast } from "sonner";
import { TreeNode } from "./connection-list/TreeNode";
import {
  getExportDefaultName,
  getExportFilter,
  renderConnectionStatusIndicator,
  sanitizeConnectionErrorMessage,
} from "./connection-list/helpers";
import { useTranslation } from "react-i18next";
import { normalizeConnectionFormInput } from "@/lib/connection-form/rules";
import { validateConnectionFormInput } from "@/lib/connection-form/validate";

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

interface SchemaInfo {
  name: string;
  tables: TableInfo[];
}

interface DatabaseInfo {
  name: string;
  schemas: SchemaInfo[];
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
  sslMode?: "require" | "verify_ca";
  sslCaCert?: string;
  filePath?: string;
  sshEnabled?: boolean;
  sshHost?: string;
  sshPort?: number;
  sshUsername?: string;
  sshPassword?: string;
  sshKeyPath?: string;
  databases: DatabaseInfo[];
  isConnected: boolean;
  connectState: "idle" | "connecting" | "success" | "error";
  connectError?: string;
}

interface CreateDatabaseForm {
  name: string;
  ifNotExists: boolean;
  charset: string;
  collation: string;
  encoding: string;
  lcCollate: string;
  lcCtype: string;
}

type SelectedTableNode = {
  key: string;
  connectionId: number;
  database: string;
  table: string;
  schema: string;
};

const defaultForm: ConnectionForm = {
  driver: "postgres",
  name: "",
  host: "",
  port: 5432,
  database: "",
  schema: "",
  username: "",
  password: "",
  ssl: false,
  sslMode: "require",
  sslCaCert: "",
  sshEnabled: false,
  sshPort: undefined,
  sshUsername: "",
};


const defaultCreateDatabaseForm: CreateDatabaseForm = {
  name: "",
  ifNotExists: true,
  charset: "",
  collation: "",
  encoding: "",
  lcCollate: "",
  lcCtype: "",
};

const createDbNoneOption = "__none__";
const mysqlCharsetOptions = ["utf8mb4", "utf8", "latin1"];
const mysqlCollationOptions = [
  "utf8mb4_general_ci",
  "utf8mb4_unicode_ci",
  "utf8_general_ci",
  "latin1_swedish_ci",
];
const postgresEncodingOptions = ["UTF8", "LATIN1", "SQL_ASCII"];
const postgresLocaleOptions = [
  "en_US.UTF-8",
  "C",
  "C.UTF-8",
  "zh_CN.UTF-8",
  "ja_JP.UTF-8",
];
const mssqlCollationOptions = [
  "SQL_Latin1_General_CP1_CI_AS",
  "Latin1_General_100_CI_AS_SC",
  "Chinese_PRC_CI_AS",
  "Japanese_CI_AS",
];
interface ConnectionListProps {
  onTableSelect?: (
    connection: string,
    database: string,
    table: string,
    connectionId: number,
    driver: string,
    schema?: string,
  ) => void;
  onConnect?: (form: ConnectionForm) => void;
  onCreateQuery?: (
    connectionId: number,
    databaseName: string,
    driver: string,
  ) => void;
  onExportTable?: (
    ctx: {
      connectionId: number;
      database: string;
      schema: string;
      table: string;
      driver: string;
    },
    format: "csv" | "json" | "sql",
    filePath: string,
  ) => void;
  activeTableTarget?: {
    connectionId: number;
    database: string;
    table: string;
    schema?: string;
  };
  sidebarRevealRequest?: {
    id: number;
    connectionId: number;
    database: string;
    table: string;
    schema?: string;
  };
  onSelectSavedQuery?: (query: SavedQuery) => void;
  lastUpdated?: number;
  showSavedQueriesInTree?: boolean;
}

export function ConnectionList({
  onTableSelect,
  onConnect,
  onCreateQuery,
  onExportTable,
  activeTableTarget,
  sidebarRevealRequest,
  onSelectSavedQuery,
  lastUpdated,
  showSavedQueriesInTree = false,
}: ConnectionListProps) {
  const { t } = useTranslation();
  const tableNodeRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const handledRevealRequestIdRef = useRef<number | null>(null);
  const [connections, setConnections] = useState<Connection[]>([]);
  const [expandedConnections, setExpandedConnections] = useState<Set<string>>(
    new Set(["1"]),
  );
  const [expandedDatabases, setExpandedDatabases] = useState<Set<string>>(
    new Set(),
  );
  const [expandedDatabaseGroups, setExpandedDatabaseGroups] = useState<
    Set<string>
  >(new Set());
  const [expandedQueryGroups, setExpandedQueryGroups] = useState<Set<string>>(
    new Set(),
  );
  const [expandedSchemas, setExpandedSchemas] = useState<Set<string>>(
    new Set(),
  );
  const [expandedTables, setExpandedTables] = useState<Set<string>>(new Set());
  const [selectedTableNode, setSelectedTableNode] =
    useState<SelectedTableNode | null>(null);
  const selectedTableKey = selectedTableNode?.key ?? null;
  const [autoScrollRequest, setAutoScrollRequest] = useState<{
    key: string;
    id: number;
  } | null>(null);
  const [contextMenu, setContextMenu] = useState<{
    visible: boolean;
    x: number;
    y: number;
    connectionId: string | null;
    databaseName?: string | null;
    schemaName?: string | null;
    type: "connection" | "database" | "schema";
  }>({ visible: false, x: 0, y: 0, connectionId: null, type: "connection" });
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [dialogMode, setDialogMode] = useState<"create" | "edit">("create");
  const [editingConnectionId, setEditingConnectionId] = useState<string | null>(
    null,
  );
  const [loadingDatabaseKeys, setLoadingDatabaseKeys] = useState<Set<string>>(
    new Set(),
  );
  const [loadingTableKeys, setLoadingTableKeys] = useState<Set<string>>(
    new Set(),
  );
  const loadingSpinner = (
    <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
  );
  const [isTesting, setIsTesting] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [isSavingEdit, setIsSavingEdit] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isCreatingDatabase, setIsCreatingDatabase] = useState(false);
  const [isImportingSql, setIsImportingSql] = useState(false);
  const [deleteTargetConnectionId, setDeleteTargetConnectionId] = useState<
    string | null
  >(null);
  const [createDbConnectionId, setCreateDbConnectionId] = useState<
    string | null
  >(null);
  const [isCreateDbDialogOpen, setIsCreateDbDialogOpen] = useState(false);
  const [showCreateDbAdvanced, setShowCreateDbAdvanced] = useState(false);
  const [createDbValidationMsg, setCreateDbValidationMsg] = useState<
    string | null
  >(null);
  const [createDbForm, setCreateDbForm] = useState<CreateDatabaseForm>(
    defaultCreateDatabaseForm,
  );
  const [testMsg, setTestMsg] = useState<{
    ok: boolean;
    text: string;
    latency?: number;
  } | null>(null);
  const [validationMsg, setValidationMsg] = useState<string | null>(null);
  const [form, setForm] = useState<ConnectionForm>(defaultForm);
  const [searchTerm, setSearchTerm] = useState("");
  const [savedQueriesByConnection, setSavedQueriesByConnection] = useState<
    Record<string, SavedQuery[]>
  >({});
  const [pendingImport, setPendingImport] = useState<{
    connectionId: string;
    databaseName: string;
    driver: Driver;
    filePath: string;
  } | null>(null);
  const [isImportConfirmOpen, setIsImportConfirmOpen] = useState(false);

  const supportsCreateDatabaseForDriver = (driver: Driver) =>
    supportsCreateDatabase(driver);
  const supportsSchemaNodeForDriver = (driver: Driver) =>
    supportsSchemaBrowsing(driver);
  const getSchemaNodeKey = (databaseKey: string, schema: string) =>
    `${databaseKey}::${schema}`;
  const getTableNodeKey = (
    connectionId: string,
    databaseName: string,
    schemaName: string,
    tableName: string,
  ) => `${connectionId}-${databaseName}-${schemaName}-${tableName}`;

  const createDbTargetConnection = useMemo(
    () => connections.find((conn) => conn.id === createDbConnectionId) || null,
    [connections, createDbConnectionId],
  );
  const createDbTargetDriver = createDbTargetConnection?.type;
  const isMySqlFamilyCreateDb =
    createDbTargetDriver === "mysql" ||
    createDbTargetDriver === "mariadb" ||
    createDbTargetDriver === "tidb";
  const isPostgresCreateDb = createDbTargetDriver === "postgres";
  const isMssqlCreateDb = createDbTargetDriver === "mssql";

  const getConnectionStatusLabel = (connection: Connection) => {
    if (connection.connectState === "success") {
      return t("connection.status.connected");
    }
    if (connection.connectState === "error") {
      if (connection.connectError) {
        return t("connection.status.failedWithReason", {
          error: connection.connectError,
        });
      }
      return t("connection.status.failed");
    }
    if (connection.connectState === "connecting") {
      return t("connection.status.connecting");
    }
    return t("connection.status.idle");
  };

  const filteredConnections = useMemo(() => {
    if (!searchTerm) return connections;
    const lowerTerm = searchTerm.toLowerCase();
    return connections
      .map((conn) => {
        const filteredDbs = conn.databases
          .map((db) => {
            const filteredSchemas = db.schemas
              .map((schema) => {
                const filteredTables = schema.tables.filter((t) =>
                  t.name.toLowerCase().includes(lowerTerm),
                );
                if (filteredTables.length > 0) {
                  return { ...schema, tables: filteredTables };
                }
                return null;
              })
              .filter(Boolean) as SchemaInfo[];
            const filteredTables = db.tables.filter((t) =>
              t.name.toLowerCase().includes(lowerTerm),
            );
            if (filteredSchemas.length > 0 || filteredTables.length > 0) {
              return {
                ...db,
                schemas: filteredSchemas,
                tables: filteredTables,
              };
            }
            return null;
          })
          .filter(Boolean) as DatabaseInfo[];

        const hasMatchingQuery =
          showSavedQueriesInTree &&
          (savedQueriesByConnection[conn.id] || []).some((query) =>
            query.name.toLowerCase().includes(lowerTerm),
          );

        if (filteredDbs.length > 0 || hasMatchingQuery) {
          return { ...conn, databases: filteredDbs };
        }
        return null;
      })
      .filter(Boolean) as Connection[];
  }, [
    connections,
    savedQueriesByConnection,
    searchTerm,
    showSavedQueriesInTree,
  ]);

  useEffect(() => {
    if (searchTerm) {
      setExpandedConnections((prev) => {
        const next = new Set(prev);
        filteredConnections.forEach((conn) => {
          next.add(conn.id);
        });
        return next;
      });
      setExpandedDatabases((prev) => {
        const next = new Set(prev);
        filteredConnections.forEach((conn) => {
          conn.databases.forEach((db) => {
            next.add(`${conn.id}-${db.name}`);
          });
        });
        return next;
      });
      setExpandedSchemas((prev) => {
        const next = new Set(prev);
        filteredConnections.forEach((conn) => {
          conn.databases.forEach((db) => {
            const databaseKey = `${conn.id}-${db.name}`;
            db.schemas.forEach((schema) => {
              next.add(getSchemaNodeKey(databaseKey, schema.name));
            });
          });
        });
        return next;
      });
      if (showSavedQueriesInTree) {
        setExpandedDatabaseGroups((prev) => {
          const next = new Set(prev);
          filteredConnections.forEach((conn) => {
            next.add(`${conn.id}::databases`);
          });
          return next;
        });
        setExpandedQueryGroups((prev) => {
          const next = new Set(prev);
          filteredConnections.forEach((conn) => {
            next.add(`${conn.id}::queries`);
          });
          return next;
        });
      }
    }
  }, [searchTerm, filteredConnections, showSavedQueriesInTree]);

  const isFileBased = isFileBasedDriver(form.driver);
  const supportsSslCa = supportsSSLCA(form.driver);
  const isPasswordRequiredOnCreate = useMemo(
    // MySQL-compatible engines (including TiDB and MariaDB) can be configured without password.
    () => !isMysqlFamilyDriver(form.driver),
    [form.driver],
  );
  const normalizedForm = useMemo(
    () => normalizeConnectionFormInput(form),
    [form],
  );
  const validationIssues = useMemo(
    () =>
      validateConnectionFormInput(
        normalizedForm,
        dialogMode === "edit" ? "edit" : "create",
      ),
    [normalizedForm, dialogMode],
  );
  const requiredOk = useMemo(() => {
    return validationIssues.length === 0;
  }, [validationIssues]);

  const validateSslSettings = () => {
    if (!form.ssl || !supportsSslCa) {
      return null;
    }
    if (form.sslMode === "verify_ca" && !(form.sslCaCert || "").trim()) {
      return t("connection.dialog.sslValidation.caRequired");
    }
    return null;
  };

  const getFirstValidationMessage = () => {
    if (validationIssues.length === 0) {
      return null;
    }
    const issue = validationIssues[0];
    return t(issue.key);
  };

  const pickSingleFile = async (params: {
    title: string;
    filters?: { name: string; extensions: string[] }[];
  }) => {
    if (!isTauri()) {
      toast.info(t("connection.toast.fileBrowserDesktopOnly"));
      return null;
    }
    try {
      const selected = await open({
        title: params.title,
        multiple: false,
        filters: params.filters,
      });
      if (selected && typeof selected === "string") {
        return selected;
      }
      return null;
    } catch (e) {
      toast.error(t("connection.toast.openFileDialogFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
      return null;
    }
  };

  const handlePickSslCaCertFile = async () => {
    const selectedPath = await pickSingleFile({
      title: t("connection.dialog.sslCaFileDialogTitle"),
      filters: [
        {
          name: t("connection.dialog.fileFilterCert"),
          extensions: ["pem", "crt", "cer"],
        },
        { name: t("connection.dialog.fileFilterAll"), extensions: ["*"] },
      ],
    });
    if (!selectedPath) return;
    try {
      const content = await readTextFile(selectedPath);
      setForm((f) => ({ ...f, sslCaCert: content }));
    } catch (e) {
      toast.error(t("connection.toast.readFileFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const handlePickSshKeyFile = async () => {
    const selectedPath = await pickSingleFile({
      title: t("connection.dialog.sshKeyFileDialogTitle"),
      // SSH private keys are often extensionless (for example ~/.ssh/id_rsa),
      // so filtering by extension can hide valid keys in the native picker.
    });
    if (!selectedPath) return;
    setForm((f) => ({ ...f, sshKeyPath: selectedPath }));
  };

  useEffect(() => {
    fetchConnections();
  }, []);

  useEffect(() => {
    if (!showSavedQueriesInTree) return;
    void fetchSavedQueriesByConnection();
  }, [showSavedQueriesInTree, lastUpdated]);

  const fetchConnections = async () => {
    try {
      const conns = await api.connections.list();
      setConnections(
        conns.map((c) => ({
          id: String(c.id),
          name: c.name || t("common.unknown"),
          type: (c.dbType as Driver) || "postgres",
          host: c.host || "",
          port: String(c.port || ""),
          database: c.database || "",
          username: c.username || "",
          ssl: c.ssl || false,
          sslMode: c.sslMode || "require",
          sslCaCert: c.sslCaCert || "",
          filePath: c.filePath || "",
          sshEnabled: c.sshEnabled || false,
          sshHost: c.sshHost || "",
          sshPort: c.sshPort || 22,
          sshUsername: c.sshUsername || "root",
          sshPassword: c.sshPassword || "",
          sshKeyPath: c.sshKeyPath || "",
          isConnected: false,
          connectState: "idle",
          connectError: undefined,
          databases: [],
        })),
      );
      setExpandedConnections(new Set());
      setExpandedDatabases(new Set());
      setExpandedDatabaseGroups(new Set());
    } catch (e) {
      console.error(
        "listConnections failed",
        e instanceof Error ? e.message : String(e),
      );
    }
  };

  const fetchSavedQueriesByConnection = async () => {
    try {
      const queries = await api.queries.list();
      const grouped: Record<string, SavedQuery[]> = {};
      queries.forEach((query) => {
        if (!query.connectionId) return;
        const key = String(query.connectionId);
        if (!grouped[key]) grouped[key] = [];
        grouped[key].push(query);
      });
      Object.values(grouped).forEach((items) =>
        items.sort((a, b) => a.name.localeCompare(b.name)),
      );
      setSavedQueriesByConnection(grouped);
    } catch (e) {
      console.error(
        "Failed to fetch saved queries for tree",
        e instanceof Error ? e.message : String(e),
      );
    }
  };

  const toggleConnection = (id: string) => {
    const connection = connections.find((conn) => conn.id === id);
    if (!connection) return;
    if (connection.connectState !== "success" && !showSavedQueriesInTree)
      return;

    const newExpanded = new Set(expandedConnections);
    if (newExpanded.has(id)) {
      newExpanded.delete(id);
    } else {
      newExpanded.add(id);
    }
    setExpandedConnections(newExpanded);
  };

  const fetchAndSetDatabases = async (
    connectionId: string,
  ): Promise<boolean> => {
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
            connectState: "success",
            connectError: undefined,
            databases: dbNames.map((name) => ({
              name,
              schemas: [],
              tables: [],
            })),
          };
        }),
      );
      return true;
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      const sanitizedMessage = sanitizeConnectionErrorMessage(message);
      console.error("listDatabasesById failed", message);
      setConnections((prev) =>
        prev.map((conn) => {
          if (conn.id !== connectionId) return conn;
          return {
            ...conn,
            isConnected: false,
            connectState: "error",
            connectError: sanitizedMessage || message,
            databases: [],
          };
        }),
      );
      toast.error(t("connection.toast.loadDatabasesFailed"), {
        description: sanitizedMessage || message,
      });
      return false;
    }
  };

  const connectConnection = async (
    connectionId: string,
    options?: { resetTree?: boolean },
  ) => {
    const target = connections.find((conn) => conn.id === connectionId);
    if (!target || target.connectState === "connecting") return;

    if (options?.resetTree) {
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
      setExpandedSchemas((prev) => {
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
    }

    setConnections((prev) =>
      prev.map((conn) => {
        if (conn.id !== connectionId) return conn;
        return {
          ...conn,
          isConnected: false,
          connectState: "connecting",
          connectError: undefined,
          databases: options?.resetTree ? [] : conn.databases,
        };
      }),
    );

    const ok = await fetchAndSetDatabases(connectionId);
    if (ok) {
      setExpandedConnections((prev) => {
        const next = new Set(prev);
        next.add(connectionId);
        return next;
      });
      return;
    }

    setExpandedConnections((prev) => {
      const next = new Set(prev);
      next.delete(connectionId);
      return next;
    });
  };

  const fetchAndSetTables = async (
    connectionId: string,
    databaseName: string,
    options?: { force?: boolean },
  ): Promise<TableInfo[]> => {
    try {
      const tables = await api.metadata.listTables(
        Number(connectionId),
        databaseName,
      );
      const nextTables: TableInfo[] = tables.map((t) => ({
        name: t.name,
        schema: t.schema,
        columns: [],
      }));
      setConnections((prev) =>
        prev.map((conn) => {
          if (conn.id !== connectionId) return conn;
          const supportsSchemaNode = supportsSchemaNodeForDriver(conn.type);
          return {
            ...conn,
            databases: conn.databases.map((db) => {
              if (db.name !== databaseName) return db;
              if (
                !options?.force &&
                (supportsSchemaNode
                  ? db.schemas.length > 0
                  : db.tables.length > 0)
              ) {
                return db;
              }
              if (!supportsSchemaNode) {
                return {
                  ...db,
                  schemas: [],
                  tables: nextTables,
                };
              }
              const grouped = nextTables.reduce<Record<string, TableInfo[]>>(
                (acc, table) => {
                  const schemaName = (table.schema || "").trim() || "public";
                  const current = acc[schemaName] || [];
                  current.push(table);
                  acc[schemaName] = current;
                  return acc;
                },
                {},
              );
              const schemas = Object.entries(grouped)
                .sort(([a], [b]) => a.localeCompare(b))
                .map(([name, schemaTables]) => ({
                  name,
                  tables: [...schemaTables].sort((a, b) =>
                    a.name.localeCompare(b.name),
                  ),
                }));
              return {
                ...db,
                schemas,
                tables: [],
              };
            }),
          };
        }),
      );
      return nextTables;
    } catch (e) {
      console.error(
        "listTables failed",
        e instanceof Error ? e.message : String(e),
      );
      return [];
    }
  };

  // Sync UI state (expansion, selection) and load data if needed.
  useEffect(() => {
    if (!activeTableTarget) {
      setSelectedTableNode(null);
      return;
    }

    const connectionId = String(activeTableTarget.connectionId);
    const databaseName = activeTableTarget.database;
    const tableName = activeTableTarget.table;
    const schemaName = activeTableTarget.schema || "";
    const dbKey = `${connectionId}-${databaseName}`;
    let cancelled = false;

    setExpandedConnections((prev) => {
      const next = new Set(prev);
      next.add(connectionId);
      return next;
    });
    setExpandedDatabases((prev) => {
      const next = new Set(prev);
      next.add(dbKey);
      return next;
    });

    const ensureDatabaseTablesLoaded = async () => {
      const targetConnection = connections.find(
        (conn) => conn.id === connectionId,
      );
      const targetDatabase = targetConnection?.databases.find(
        (db) => db.name === databaseName,
      );
      if (!targetDatabase) return;

      const supportsSchemaNode = supportsSchemaNodeForDriver(
        targetConnection?.type || "postgres",
      );
      const hasLoadedTables = supportsSchemaNode
        ? targetDatabase.schemas.length > 0
        : targetDatabase.tables.length > 0;
      let availableTables = supportsSchemaNode
        ? targetDatabase.schemas.flatMap((schema) => schema.tables)
        : targetDatabase.tables;
      if (!hasLoadedTables) {
        availableTables = await fetchAndSetTables(connectionId, databaseName);
      }
      if (cancelled) return;
      const resolvedSchema =
        schemaName ||
        availableTables.find((table) => table.name === tableName)?.schema ||
        "";
      if (supportsSchemaNode && resolvedSchema) {
        setExpandedSchemas((prev) => {
          const next = new Set(prev);
          next.add(getSchemaNodeKey(dbKey, resolvedSchema));
          return next;
        });
      }
      const resolvedTableKey = getTableNodeKey(
        connectionId,
        databaseName,
        resolvedSchema,
        tableName,
      );
      setSelectedTableNode({
        key: resolvedTableKey,
        connectionId: activeTableTarget.connectionId,
        database: databaseName,
        table: tableName,
        schema: resolvedSchema,
      });
    };

    void ensureDatabaseTablesLoaded();
    return () => {
      cancelled = true;
    };
  }, [activeTableTarget, connections]);

  useEffect(() => {
    if (!sidebarRevealRequest || !activeTableTarget || !selectedTableNode)
      return;
    if (handledRevealRequestIdRef.current === sidebarRevealRequest.id) return;
    if (
      sidebarRevealRequest.connectionId !== activeTableTarget.connectionId ||
      sidebarRevealRequest.database !== activeTableTarget.database ||
      sidebarRevealRequest.table !== activeTableTarget.table
    ) {
      return;
    }
    if (
      selectedTableNode.connectionId !== sidebarRevealRequest.connectionId ||
      selectedTableNode.database !== sidebarRevealRequest.database ||
      selectedTableNode.table !== sidebarRevealRequest.table
    ) {
      return;
    }
    if (
      sidebarRevealRequest.schema &&
      sidebarRevealRequest.schema !== selectedTableNode.schema
    ) {
      return;
    }

    handledRevealRequestIdRef.current = sidebarRevealRequest.id;
    setAutoScrollRequest({
      key: selectedTableNode.key,
      id: sidebarRevealRequest.id,
    });
  }, [activeTableTarget, selectedTableNode, sidebarRevealRequest]);

  useEffect(() => {
    if (!autoScrollRequest) return;
    let cancelled = false;
    let retriesLeft = 12;
    let frame1 = 0;
    let frame2 = 0;

    const run = () => {
      frame1 = requestAnimationFrame(() => {
        frame2 = requestAnimationFrame(() => {
          if (cancelled) return;
          const target = tableNodeRefs.current[autoScrollRequest.key];
          if (target) {
            target.scrollIntoView({
              block: "center",
              inline: "nearest",
              behavior: "auto",
            });
            setAutoScrollRequest((prev) =>
              prev?.id === autoScrollRequest.id ? null : prev,
            );
            return;
          }

          retriesLeft -= 1;
          if (retriesLeft > 0) {
            run();
            return;
          }

          setAutoScrollRequest((prev) =>
            prev?.id === autoScrollRequest.id ? null : prev,
          );
        });
      });
    };

    run();

    return () => {
      cancelled = true;
      if (frame1) cancelAnimationFrame(frame1);
      if (frame2) cancelAnimationFrame(frame2);
    };
  }, [autoScrollRequest]);

  const handleRefreshDatabaseTables = async (
    connectionId: string,
    databaseName: string,
  ) => {
    const databaseKey = `${connectionId}-${databaseName}`;
    const tableKeyPrefix = `${databaseKey}-`;
    const schemaKeyPrefix = `${databaseKey}::`;
    setExpandedSchemas((prev) => {
      const next = new Set(
        [...prev].filter((key) => !key.startsWith(schemaKeyPrefix)),
      );
      return next;
    });
    setExpandedTables((prev) => {
      const next = new Set(
        [...prev].filter((key) => !key.startsWith(tableKeyPrefix)),
      );
      return next;
    });

    await fetchAndSetTables(connectionId, databaseName, { force: true });
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
      // Find the corresponding connection and database
      const conn = connections.find((c) => c.id === connId);
      if (conn) {
        const db = conn.databases.find((d) => d.name === dbName);
        if (
          db &&
          (supportsSchemaNodeForDriver(conn.type)
            ? db.schemas.length === 0
            : db.tables.length === 0)
        ) {
          setLoadingDatabaseKeys((prev) => new Set(prev).add(key));
          fetchAndSetTables(connId, dbName).finally(() => {
            setLoadingDatabaseKeys((prev) => {
              const next = new Set(prev);
              next.delete(key);
              return next;
            });
          });
        }
      }
    }
    setExpandedDatabases(newExpanded);
  };

  const toggleQueryGroup = (key: string) => {
    setExpandedQueryGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };

  const toggleDatabaseGroup = (key: string) => {
    setExpandedDatabaseGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };

  const toggleSchema = (schemaKey: string) => {
    setExpandedSchemas((prev) => {
      const next = new Set(prev);
      if (next.has(schemaKey)) {
        next.delete(schemaKey);
      } else {
        next.add(schemaKey);
      }
      return next;
    });
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
                schemas: db.schemas.map((schemaNode) => ({
                  ...schemaNode,
                  tables: schemaNode.tables.map((t) => {
                    if (t.name !== tableName || t.schema !== schema) return t;
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
                })),
                tables: db.tables.map((t) => {
                  if (t.name !== tableName || t.schema !== schema) return t;
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
      console.error(
        "getTableMetadata failed",
        e instanceof Error ? e.message : String(e),
      );
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
        setLoadingTableKeys((prev) => new Set(prev).add(tableKey));
        fetchAndSetTableColumns(
          connectionId,
          databaseName,
          table.schema,
          table.name,
        ).finally(() => {
          setLoadingTableKeys((prev) => {
            const next = new Set(prev);
            next.delete(tableKey);
            return next;
          });
        });
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
        table.schema,
      );
    }
  };

  const handleCreateQueryFromContext = (
    connectionId: string | null | undefined,
    databaseName?: string | null,
  ) => {
    if (!onCreateQuery || !connectionId) return;
    const connection = connections.find((c) => c.id === connectionId);
    if (!connection) return;

    const explicitDatabaseName = (databaseName || "").trim();
    const fallbackDatabaseName =
      (connection.database || "").trim() ||
      connection.databases.find((db) => db.name.trim().length > 0)?.name ||
      (connection.type === "sqlite" || connection.type === "duckdb"
        ? "main"
        : "");
    const resolvedDatabaseName = explicitDatabaseName || fallbackDatabaseName;

    if (!resolvedDatabaseName) {
      toast.error(t("connection.toast.newQueryNoDatabase"));
      return;
    }

    onCreateQuery(Number(connectionId), resolvedDatabaseName, connection.type);
  };

  const openCreateDatabaseDialog = (connectionId: string) => {
    const connection = connections.find((conn) => conn.id === connectionId);
    if (!connection || !supportsCreateDatabaseForDriver(connection.type)) {
      return;
    }
    setCreateDbConnectionId(connectionId);
    setCreateDbValidationMsg(null);
    setShowCreateDbAdvanced(false);
    setCreateDbForm(defaultCreateDatabaseForm);
    setIsCreateDbDialogOpen(true);
  };

  const clearConnectionTreeCache = (connectionId: string) => {
    setConnections((prev) =>
      prev.map((conn) =>
        conn.id === connectionId ? { ...conn, databases: [] } : conn,
      ),
    );
    setExpandedDatabases(
      (prev) =>
        new Set([...prev].filter((key) => !key.startsWith(`${connectionId}-`))),
    );
    setExpandedSchemas(
      (prev) =>
        new Set([...prev].filter((key) => !key.startsWith(`${connectionId}-`))),
    );
    setExpandedTables(
      (prev) =>
        new Set([...prev].filter((key) => !key.startsWith(`${connectionId}-`))),
    );
  };

  const handleCreateDatabase = async () => {
    const connection = createDbTargetConnection;
    if (!connection || !supportsCreateDatabaseForDriver(connection.type))
      return;

    const name = createDbForm.name.trim();
    if (!name) {
      setCreateDbValidationMsg(
        t("connection.createDbDialog.validation.requiredName"),
      );
      return;
    }

    const payload: CreateDatabasePayload = {
      name,
      ifNotExists: createDbForm.ifNotExists,
    };
    if (isMySqlFamilyCreateDb) {
      if (createDbForm.charset.trim())
        payload.charset = createDbForm.charset.trim();
      if (createDbForm.collation.trim()) {
        payload.collation = createDbForm.collation.trim();
      }
    } else if (isPostgresCreateDb) {
      if (createDbForm.encoding.trim())
        payload.encoding = createDbForm.encoding.trim();
      if (createDbForm.lcCollate.trim()) {
        payload.lcCollate = createDbForm.lcCollate.trim();
      }
      if (createDbForm.lcCtype.trim())
        payload.lcCtype = createDbForm.lcCtype.trim();
    } else if (isMssqlCreateDb) {
      if (createDbForm.collation.trim()) {
        payload.collation = createDbForm.collation.trim();
      }
    }

    setCreateDbValidationMsg(null);
    setIsCreatingDatabase(true);
    try {
      await api.connections.createDatabase(Number(connection.id), payload);
      toast.success(t("connection.toast.createDatabaseSuccess"), {
        description: name,
      });
      setIsCreateDbDialogOpen(false);
      clearConnectionTreeCache(connection.id);
      const loaded = await fetchAndSetDatabases(connection.id);
      if (loaded) {
        setExpandedConnections((prev) => {
          const next = new Set(prev);
          next.add(connection.id);
          return next;
        });
      }
    } catch (e) {
      toast.error(t("connection.toast.createDatabaseFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsCreatingDatabase(false);
    }
  };

  const handleTestConnection = async () => {
    try {
      setValidationMsg(null);
      const fieldValidationError = getFirstValidationMessage();
      if (fieldValidationError) {
        setValidationMsg(fieldValidationError);
        return;
      }
      const sslError = validateSslSettings();
      if (sslError) {
        setValidationMsg(sslError);
        return;
      }
      setIsTesting(true);
      setTestMsg(null);
      const res = await api.connections.testEphemeral(normalizedForm);
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
      setValidationMsg(getFirstValidationMessage());
      return;
    }
    setValidationMsg(null);
    const sslError = validateSslSettings();
    if (sslError) {
      setValidationMsg(sslError);
      return;
    }
    setIsConnecting(true);
    try {
      const res = await api.connections.create(normalizedForm);
      setConnections((prev) => [
        {
          id: String(res.id),
          name: res.name || t("common.unknown"),
          type: (res.dbType as Driver) || "postgres",
          host: res.host || "",
          port: String(res.port || ""),
          database: res.database || "",
          username: res.username || "",
          ssl: res.ssl || false,
          sslMode: res.sslMode || "require",
          sslCaCert: res.sslCaCert || "",
          filePath: res.filePath || "",
          sshEnabled: res.sshEnabled || false,
          sshHost: res.sshHost || "",
          sshPort: res.sshPort || 22,
          sshUsername: res.sshUsername || "root",
          sshPassword: "",
          sshKeyPath: res.sshKeyPath || "",
          isConnected: false,
          connectState: "idle",
          connectError: undefined,
          databases: [],
        },
        ...prev,
      ]);
      setIsDialogOpen(false);
      setForm(defaultForm);
      if (onConnect) onConnect(normalizedForm);
    } catch (e: any) {
      setValidationMsg(String(e?.message || e));
    } finally {
      setIsConnecting(false);
    }
  };

  const handleSaveEdit = async () => {
    if (!editingConnectionId) return;
    if (!requiredOk) {
      setValidationMsg(getFirstValidationMessage());
      return;
    }

    setValidationMsg(null);
    const sslError = validateSslSettings();
    if (sslError) {
      setValidationMsg(sslError);
      return;
    }
    setIsSavingEdit(true);
    try {
      await api.connections.update(Number(editingConnectionId), normalizedForm);
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
      sslMode: conn.sslMode || "require",
      sslCaCert: conn.sslCaCert || "",
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
    await connectConnection(connectionId, { resetTree: true });
  };

  const buildDuplicateConnectionName = (sourceName: string) => {
    const baseName = `${sourceName}-${t("connection.menu.copy")}`;
    let candidate = baseName;
    let counter = 2;
    while (connections.some((conn) => conn.name === candidate)) {
      candidate = `${baseName}-${counter}`;
      counter += 1;
    }
    return candidate;
  };

  const handleDuplicateConnection = async (connectionId: string) => {
    const source = connections.find((conn) => conn.id === connectionId);
    if (!source) return;

    const duplicateName = buildDuplicateConnectionName(
      source.name || t("common.unknown"),
    );
    const duplicateForm: ConnectionForm = {
      driver: source.type,
      name: duplicateName,
      host: source.host || "",
      port: Number(source.port) || undefined,
      database: source.database || "",
      schema: source.type === "postgres" ? "public" : "",
      username: source.username || "",
      password: "",
      ssl: source.ssl || false,
      sslMode: source.sslMode || "require",
      sslCaCert: source.sslCaCert || "",
      filePath: source.filePath || "",
      sshEnabled: source.sshEnabled || false,
      sshHost: source.sshHost || "",
      sshPort: source.sshPort || undefined,
      sshUsername: source.sshUsername || "",
      sshPassword: "",
      sshKeyPath: source.sshKeyPath || "",
    };

    try {
      const res = await api.connections.create(duplicateForm);
      setConnections((prev) => [
        {
          id: String(res.id),
          name: res.name || t("common.unknown"),
          type: (res.dbType as Driver) || "postgres",
          host: res.host || "",
          port: String(res.port || ""),
          database: res.database || "",
          username: res.username || "",
          ssl: res.ssl || false,
          sslMode: res.sslMode || "require",
          sslCaCert: res.sslCaCert || "",
          filePath: res.filePath || "",
          sshEnabled: res.sshEnabled || false,
          sshHost: res.sshHost || "",
          sshPort: res.sshPort || 22,
          sshUsername: res.sshUsername || "root",
          sshPassword: "",
          sshKeyPath: res.sshKeyPath || "",
          isConnected: false,
          connectState: "idle",
          connectError: undefined,
          databases: [],
        },
        ...prev,
      ]);
      toast.success(t("connection.toast.duplicateSuccess"));
    } catch (e) {
      toast.error(t("connection.toast.duplicateFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
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
      setExpandedSchemas((prev) => {
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
      console.error(
        "deleteConnection failed",
        e instanceof Error ? e.message : String(e),
      );
    } finally {
      setIsDeleting(false);
    }
  };

  const handleTableExport = async (
    connection: Connection,
    database: DatabaseInfo,
    table: TableInfo,
    format: "csv" | "json" | "sql",
  ) => {
    if (!onExportTable) return;
    if (!isTauri()) {
      toast.error(t("connection.toast.exportDesktopOnly"));
      return;
    }

    try {
      const selected = await save({
        title: t("connection.toast.saveExportFile"),
        defaultPath: getExportDefaultName(table.name, format),
        filters: getExportFilter(format),
      });
      if (!selected) return;
      const filePath = Array.isArray(selected) ? selected[0] : selected;
      if (!filePath) return;

      onExportTable(
        {
          connectionId: Number(connection.id),
          database: database.name,
          schema: table.schema,
          table: table.name,
          driver: connection.type,
        },
        format,
        filePath,
      );
    } catch (e) {
      toast.error(t("connection.toast.openSaveDialogFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const handleDatabaseImport = async (
    connectionId: string,
    databaseName: string,
  ) => {
    const connection = connections.find((conn) => conn.id === connectionId);
    if (!connection) return;

    const capability = getImportDriverCapability(connection.type);
    if (capability === "read_only_not_supported") {
      toast.error(t("connection.toast.importReadOnlyDriver"));
      return;
    }

    if (capability !== "supported") {
      toast.error(t("connection.toast.importUnsupportedDriver"));
      return;
    }

    if (!isTauri()) {
      toast.error(t("connection.toast.importDesktopOnly"));
      return;
    }

    const selectedPath = await pickSingleFile({
      title: t("connection.toast.selectImportSqlFile"),
      filters: [{ name: "SQL", extensions: ["sql"] }],
    });
    if (!selectedPath) return;

    setPendingImport({
      connectionId,
      databaseName,
      driver: connection.type,
      filePath: selectedPath,
    });
    setIsImportConfirmOpen(true);
  };

  const handleConfirmImport = async () => {
    if (!pendingImport) return;

    setIsImportingSql(true);
    try {
      const result = await api.transfer.importSqlFile({
        id: Number(pendingImport.connectionId),
        database: pendingImport.databaseName,
        filePath: pendingImport.filePath,
        driver: pendingImport.driver,
      });

      if (result.error || result.failedAt) {
        toast.error(t("connection.toast.importFailed"), {
          description: result.error || t("common.unknown"),
        });
      } else {
        toast.success(
          t("connection.toast.importSuccess", {
            count: result.successStatements,
          }),
          {
            description: pendingImport.filePath,
          },
        );
      }

      await handleRefreshDatabaseTables(
        pendingImport.connectionId,
        pendingImport.databaseName,
      );
    } catch (e) {
      toast.error(t("connection.toast.importFailed"), {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsImportingSql(false);
      setIsImportConfirmOpen(false);
      setPendingImport(null);
    }
  };

  const contextMenuConnection = contextMenu.connectionId
    ? connections.find((conn) => conn.id === contextMenu.connectionId)
    : null;
  const contextMenuDatabaseConnection = contextMenu.connectionId
    ? connections.find((conn) => conn.id === contextMenu.connectionId)
    : null;

  return (
    <div className="h-full flex flex-col bg-background border-r border-border">
      <div className="px-2 py-1 border-b border-border flex items-center justify-between h-8">
        <h2 className="font-semibold text-sm">{t("connection.title")}</h2>
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
                      ? t("connection.dialog.editTitle")
                      : t("connection.dialog.newTitle")}
                  </DialogTitle>
                </DialogHeader>
                <div className="grid gap-4 py-4">
                  <div className="grid gap-2">
                    <Label htmlFor="type">
                      {t("connection.dialog.fields.databaseType")}
                    </Label>
                    <Select
                      value={form.driver}
                      onValueChange={(v: Driver) =>
                        setForm((f) => ({
                          ...f,
                          driver: v,
                          port: getDefaultPort(v) ?? f.port,
                        }))
                      }
                    >
                      <SelectTrigger id="type">
                        <SelectValue
                          placeholder={t(
                            "connection.dialog.placeholders.selectDatabaseType",
                          )}
                        />
                      </SelectTrigger>
                      <SelectContent>
                        {DRIVER_REGISTRY.map((d) => (
                          <SelectItem key={d.id} value={d.id}>
                            {d.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="grid gap-2">
                    <Label htmlFor="name">
                      {t("connection.dialog.fields.connectionName")}
                    </Label>
                    <Input
                      id="name"
                      value={form.name || ""}
                      onChange={(e) =>
                        setForm((f) => ({ ...f, name: e.target.value }))
                      }
                    />
                  </div>
                  {!isFileBased && (
                    <>
                      <div className="grid grid-cols-2 gap-2">
                        <div className="grid gap-2">
                          <Label htmlFor="host">
                            {t("connection.dialog.fields.host")}{" "}
                            <span className="text-red-600">*</span>
                          </Label>
                          <Input
                            id="host"
                            value={form.host || ""}
                            onChange={(e) =>
                              setForm((f) => ({ ...f, host: e.target.value }))
                            }
                          />
                        </div>
                        <div className="grid gap-2">
                          <Label htmlFor="port">
                            {t("connection.dialog.fields.port")}{" "}
                            <span className="text-red-600">*</span>
                          </Label>
                          <Input
                            id="port"
                            placeholder={String(
                              getDefaultPort(form.driver) ?? "",
                            )}
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
                            {t("connection.dialog.fields.username")}{" "}
                            <span className="text-red-600">*</span>
                          </Label>
                          <Input
                            id="username"
                            value={form.username || ""}
                            onChange={(e) =>
                              setForm((f) => ({
                                ...f,
                                username: e.target.value,
                              }))
                            }
                          />
                        </div>
                        <div className="grid gap-2">
                          <Label htmlFor="password">
                            {t("connection.dialog.fields.password")}{" "}
                            {dialogMode === "create" &&
                              isPasswordRequiredOnCreate && (
                                <span className="text-red-600">*</span>
                              )}
                          </Label>
                          <Input
                            id="password"
                            type="password"
                            placeholder={
                              dialogMode === "edit"
                                ? t(
                                    "connection.dialog.placeholders.keepPassword",
                                  )
                                : undefined
                            }
                            value={form.password || ""}
                            onChange={(e) =>
                              setForm((f) => ({
                                ...f,
                                password: e.target.value,
                              }))
                            }
                          />
                        </div>
                      </div>
                      <div className="grid grid-cols-2 gap-2">
                        <div className="grid gap-2">
                          <Label htmlFor="database">
                            {t("connection.dialog.fields.database")}
                          </Label>
                          <Input
                            id="database"
                            value={form.database || ""}
                            onChange={(e) =>
                              setForm((f) => ({
                                ...f,
                                database: e.target.value,
                              }))
                            }
                          />
                        </div>
                        <div className="grid gap-2">
                          <Label htmlFor="schema">
                            {t("connection.dialog.fields.schema")}
                          </Label>
                          <Input
                            id="schema"
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
                        <Label htmlFor="ssl">
                          {t("connection.dialog.fields.ssl")}
                        </Label>
                      </div>
                      {form.ssl && supportsSslCa && (
                        <div className="border p-3 rounded-md space-y-3 bg-muted/20">
                          <div className="grid gap-2">
                            <Label htmlFor="sslMode">
                              {t("connection.dialog.fields.sslMode")}
                            </Label>
                            <Select
                              value={form.sslMode || "require"}
                              onValueChange={(v: "require" | "verify_ca") =>
                                setForm((f) => ({ ...f, sslMode: v }))
                              }
                            >
                              <SelectTrigger id="sslMode">
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent>
                                <SelectItem value="require">
                                  {t("connection.dialog.sslMode.require")}
                                </SelectItem>
                                <SelectItem value="verify_ca">
                                  {t("connection.dialog.sslMode.verifyCa")}
                                </SelectItem>
                              </SelectContent>
                            </Select>
                          </div>
                          {form.sslMode === "verify_ca" && (
                            <div className="grid gap-2">
                              <Label htmlFor="sslCaCert">
                                {t("connection.dialog.fields.sslCaCert")}{" "}
                                <span className="text-red-600">*</span>
                              </Label>
                              <div className="flex justify-end">
                                <Button
                                  type="button"
                                  variant="outline"
                                  size="sm"
                                  onClick={() => void handlePickSslCaCertFile()}
                                >
                                  <FolderOpen className="w-4 h-4 mr-2" />
                                  {t("connection.dialog.browse")}
                                </Button>
                              </div>
                              <Textarea
                                id="sslCaCert"
                                rows={5}
                                placeholder={t(
                                  "connection.dialog.placeholders.sslCaCert",
                                )}
                                value={form.sslCaCert || ""}
                                onChange={(e) =>
                                  setForm((f) => ({
                                    ...f,
                                    sslCaCert: e.target.value,
                                  }))
                                }
                              />
                            </div>
                          )}
                        </div>
                      )}

                      <div className="flex items-center space-x-2">
                        <Checkbox
                          id="ssh"
                          checked={form.sshEnabled}
                          onCheckedChange={(checked) =>
                            setForm((f) => ({
                              ...f,
                              sshEnabled: checked === true,
                            }))
                          }
                        />
                        <Label htmlFor="ssh">
                          {t("connection.dialog.fields.ssh")}
                        </Label>
                      </div>

                      {form.sshEnabled && (
                        <div className="border p-3 rounded-md space-y-3 bg-muted/20">
                          <div className="grid grid-cols-2 gap-2">
                            <div className="grid gap-2">
                              <Label htmlFor="sshHost">
                                {t("connection.dialog.fields.sshHost")}
                              </Label>
                              <Input
                                id="sshHost"
                                placeholder={t(
                                  "connection.dialog.placeholders.sshHost",
                                )}
                                value={form.sshHost || ""}
                                onChange={(e) =>
                                  setForm((f) => ({
                                    ...f,
                                    sshHost: e.target.value,
                                  }))
                                }
                              />
                            </div>
                            <div className="grid gap-2">
                              <Label htmlFor="sshPort">
                                {t("connection.dialog.fields.sshPort")}
                              </Label>
                              <Input
                                id="sshPort"
                                placeholder={t(
                                  "connection.dialog.placeholders.sshPort",
                                )}
                                value={String(form.sshPort || "")}
                                onChange={(e) =>
                                  setForm((f) => ({
                                    ...f,
                                    sshPort:
                                      Number(e.target.value) || undefined,
                                  }))
                                }
                              />
                            </div>
                          </div>
                          <div className="grid gap-2">
                            <Label htmlFor="sshUsername">
                              {t("connection.dialog.fields.sshUsername")}
                            </Label>
                            <Input
                              id="sshUsername"
                              placeholder={t(
                                "connection.dialog.placeholders.sshUsername",
                              )}
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
                            <Label htmlFor="sshPassword">
                              {t("connection.dialog.fields.sshPassword")}
                            </Label>
                            <Input
                              id="sshPassword"
                              type="password"
                              placeholder={t(
                                "connection.dialog.placeholders.sshPassword",
                              )}
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
                            <Label htmlFor="sshKeyPath">
                              {t("connection.dialog.fields.sshKeyPath")}
                            </Label>
                            <div className="flex gap-2">
                              <Input
                                id="sshKeyPath"
                                placeholder={t(
                                  "connection.dialog.placeholders.sshKeyPath",
                                )}
                                value={form.sshKeyPath || ""}
                                onChange={(e) =>
                                  setForm((f) => ({
                                    ...f,
                                    sshKeyPath: e.target.value,
                                  }))
                                }
                              />
                              <Button
                                type="button"
                                variant="outline"
                                onClick={() => void handlePickSshKeyFile()}
                              >
                                <FolderOpen className="w-4 h-4 mr-2" />
                                {t("connection.dialog.browse")}
                              </Button>
                            </div>
                          </div>
                        </div>
                      )}
                    </>
                  )}
                  {isFileBased && (
                    <div className="grid gap-2">
                      <Label htmlFor="filePath">
                        {form.driver === "duckdb"
                          ? t("connection.dialog.fields.duckdbFilePath")
                          : t("connection.dialog.fields.sqliteFilePath")}{" "}
                        <span className="text-red-600">*</span>
                      </Label>
                      <div className="flex gap-2">
                        <Input
                          id="filePath"
                          placeholder={
                            form.driver === "duckdb"
                              ? t("connection.dialog.placeholders.duckdbPath")
                              : t("connection.dialog.placeholders.sqlitePath")
                          }
                          value={form.filePath || ""}
                          onChange={(e) =>
                            setForm((f) => ({ ...f, filePath: e.target.value }))
                          }
                          className="flex-1"
                        />
                        <Button
                          type="button"
                          variant="outline"
                          onClick={async () => {
                            const selected = await pickSingleFile({
                              title:
                                form.driver === "duckdb"
                                  ? t("connection.dialog.fileDialogTitleDuckdb")
                                  : t("connection.dialog.fileDialogTitle"),
                              filters: [
                                {
                                  name:
                                    form.driver === "duckdb"
                                      ? t("connection.dialog.fileFilterDuckdb")
                                      : t("connection.dialog.fileFilterSqlite"),
                                  extensions:
                                    form.driver === "duckdb"
                                      ? ["duckdb", "db"]
                                      : ["sqlite", "db", "sqlite3", "db3"],
                                },
                                {
                                  name: t("connection.dialog.fileFilterAll"),
                                  extensions: ["*"],
                                },
                              ],
                            });
                            if (!selected) return;
                            setForm((f) => ({ ...f, filePath: selected }));
                          }}
                        >
                          <FolderOpen className="w-4 h-4 mr-2" />
                          {t("connection.dialog.browse")}
                        </Button>
                      </div>
                    </div>
                  )}
                </div>
                <div className="flex justify-end gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => setIsDialogOpen(false)}
                  >
                    {t("common.cancel")}
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
                        {t("connection.dialog.testing")}
                      </>
                    ) : (
                      t("connection.dialog.test")
                    )}
                  </Button>
                  <Button
                    type="submit"
                    disabled={
                      (dialogMode === "edit" ? isSavingEdit : isConnecting) ||
                      !requiredOk
                    }
                  >
                    {dialogMode === "edit" ? (
                      isSavingEdit ? (
                        <>
                          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                          {t("connection.dialog.saving")}
                        </>
                      ) : (
                        t("common.save")
                      )
                    ) : isConnecting ? (
                      <>
                        <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                        {t("connection.dialog.connecting")}
                      </>
                    ) : (
                      t("connection.dialog.connect")
                    )}
                  </Button>
                </div>
                {validationMsg && (
                  <div className="mt-3">
                    <Alert variant="destructive">
                      <AlertTitle>
                        {t("connection.dialog.validationFailed")}
                      </AlertTitle>
                      <AlertDescription>{validationMsg}</AlertDescription>
                    </Alert>
                  </div>
                )}
                {testMsg && (
                  <div className="mt-3">
                    <Alert variant={testMsg.ok ? "default" : "destructive"}>
                      <AlertTitle>
                        {testMsg.ok
                          ? t("connection.dialog.testSuccess")
                          : t("connection.dialog.testFailed")}
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
            placeholder={t("connection.searchTables")}
            value={searchTerm}
            onChange={(e) => {
              setSearchTerm(e.target.value);
            }}
            className="pl-8"
          />
        </div>
      </div>
      <div
        className="flex-1 overflow-auto"
        onClick={() => setContextMenu((prev) => ({ ...prev, visible: false }))}
      >
        {filteredConnections.map((connection) => {
          const queriesForConnection = (
            savedQueriesByConnection[connection.id] || []
          ).filter((query) =>
            query.name.toLowerCase().includes(searchTerm.toLowerCase()),
          );
          const visibleDatabases = connection.databases.filter(
            (database) =>
              !["information_schema", "performance_schema"].includes(
                database.name.toLowerCase(),
              ),
          );

          return (
            <TreeNode
              key={connection.id}
              level={0}
              icon={getConnectionIcon(connection.type)}
              label={connection.name}
              isExpanded={expandedConnections.has(connection.id)}
              toggleOnRowClick={
                showSavedQueriesInTree || connection.connectState === "success"
              }
              onToggle={() => toggleConnection(connection.id)}
              onDoubleClick={() => {
                void connectConnection(connection.id);
              }}
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
              leadingIndicator={
                <span
                  className="inline-flex items-center justify-center shrink-0"
                  role="status"
                  aria-label={getConnectionStatusLabel(connection)}
                  title={getConnectionStatusLabel(connection)}
                >
                  {renderConnectionStatusIndicator(connection)}
                </span>
              }
            >
              <>
                {showSavedQueriesInTree ? (
                  <TreeNode
                    level={1}
                    icon={<FileCode className="w-4 h-4" />}
                    label={t("connection.tree.queries")}
                    isExpanded={expandedQueryGroups.has(
                      `${connection.id}::queries`,
                    )}
                    onToggle={() =>
                      toggleQueryGroup(`${connection.id}::queries`)
                    }
                    forceShowToggle={queriesForConnection.length > 0}
                    canToggle={queriesForConnection.length > 0}
                  >
                    {queriesForConnection.map((query) => (
                      <TreeNode
                        key={`conn-query-${query.id}`}
                        level={2}
                        icon={<FileCode className="w-4 h-4" />}
                        label={query.name}
                        toggleOnRowClick={false}
                        canToggle={false}
                        onDoubleClick={() => onSelectSavedQuery?.(query)}
                        onContextMenu={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                        }}
                      >
                        {null}
                      </TreeNode>
                    ))}
                  </TreeNode>
                ) : null}

                {connection.connectState === "success" ? (
                  showSavedQueriesInTree ? (
                    <TreeNode
                      level={1}
                      icon={<Database className="w-4 h-4" />}
                      label={t("connection.tree.database")}
                      isExpanded={expandedDatabaseGroups.has(
                        `${connection.id}::databases`,
                      )}
                      onToggle={() =>
                        toggleDatabaseGroup(`${connection.id}::databases`)
                      }
                      forceShowToggle={visibleDatabases.length > 0}
                      canToggle={visibleDatabases.length > 0}
                    >
                      {visibleDatabases.map((database) => {
                        const databaseLevel = 2;
                        const dbKey = `${connection.id}-${database.name}`;
                        const supportsSchemaNode = supportsSchemaNodeForDriver(
                          connection.type,
                        );
                        const renderTableNode = (
                          table: TableInfo,
                          level: number,
                        ) => {
                          const tableKey = getTableNodeKey(
                            connection.id,
                            database.name,
                            table.schema,
                            table.name,
                          );
                          return (
                            <ContextMenu key={tableKey}>
                              <ContextMenuTrigger asChild>
                                <div
                                  ref={(el) => {
                                    tableNodeRefs.current[tableKey] = el;
                                  }}
                                >
                                  <TreeNode
                                    level={level}
                                    icon={<Table className="w-4 h-4" />}
                                    label={table.name}
                                    isSelected={selectedTableKey === tableKey}
                                    isExpanded={expandedTables.has(tableKey)}
                                    toggleOnRowClick={false}
                                    onToggle={() => {
                                      toggleTable(
                                        tableKey,
                                        connection.id,
                                        database.name,
                                        table,
                                      );
                                    }}
                                    onDoubleClick={() => {
                                      handleTableClick(
                                        connection,
                                        database,
                                        table,
                                      );
                                    }}
                                    statusIndicator={
                                      loadingTableKeys.has(tableKey)
                                        ? loadingSpinner
                                        : undefined
                                    }
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
                                        style={{
                                          paddingLeft: `${(level + 1) * 12 + 8}px`,
                                        }}
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
                                </div>
                              </ContextMenuTrigger>
                              <ContextMenuContent>
                                <ContextMenuItem
                                  onClick={() =>
                                    handleCreateQueryFromContext(
                                      connection.id,
                                      database.name,
                                    )
                                  }
                                >
                                  <FileCode className="w-4 h-4 mr-2" />
                                  {t("connection.menu.newQuery")}
                                </ContextMenuItem>
                                <ContextMenuItem
                                  onClick={() =>
                                    void handleTableExport(
                                      connection,
                                      database,
                                      table,
                                      "csv",
                                    )
                                  }
                                >
                                  <Download className="w-4 h-4 mr-2" />
                                  {t("connection.menu.exportCsv")}
                                </ContextMenuItem>
                                <ContextMenuItem
                                  onClick={() =>
                                    void handleTableExport(
                                      connection,
                                      database,
                                      table,
                                      "json",
                                    )
                                  }
                                >
                                  <Download className="w-4 h-4 mr-2" />
                                  {t("connection.menu.exportJson")}
                                </ContextMenuItem>
                                <ContextMenuItem
                                  onClick={() =>
                                    void handleTableExport(
                                      connection,
                                      database,
                                      table,
                                      "sql",
                                    )
                                  }
                                >
                                  <Download className="w-4 h-4 mr-2" />
                                  {t("connection.menu.exportSql")}
                                </ContextMenuItem>
                              </ContextMenuContent>
                            </ContextMenu>
                          );
                        };
                        return (
                          <TreeNode
                            key={dbKey}
                            level={databaseLevel}
                            icon={<Database className="w-4 h-4" />}
                            label={
                              (connection.type === "sqlite" ||
                                connection.type === "duckdb") &&
                              database.name === "main"
                                ? t(
                                    connection.type === "duckdb"
                                      ? "connection.duckdbMainLabel"
                                      : "connection.sqliteMainLabel",
                                  )
                                : database.name
                            }
                            isExpanded={expandedDatabases.has(dbKey)}
                            onToggle={() => toggleDatabase(dbKey)}
                            statusIndicator={
                              loadingDatabaseKeys.has(dbKey)
                                ? loadingSpinner
                                : undefined
                            }
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
                            {supportsSchemaNode
                              ? database.schemas.map((schemaNode) => {
                                  const schemaKey = getSchemaNodeKey(
                                    dbKey,
                                    schemaNode.name,
                                  );
                                  return (
                                    <TreeNode
                                      key={schemaKey}
                                      level={databaseLevel + 1}
                                      icon={<FolderOpen className="w-4 h-4" />}
                                      label={schemaNode.name}
                                      isExpanded={expandedSchemas.has(
                                        schemaKey,
                                      )}
                                      onToggle={() => toggleSchema(schemaKey)}
                                      onContextMenu={(e) => {
                                        e.preventDefault();
                                        e.stopPropagation();
                                        setContextMenu({
                                          visible: true,
                                          x: e.clientX,
                                          y: e.clientY,
                                          connectionId: connection.id,
                                          databaseName: database.name,
                                          schemaName: schemaNode.name,
                                          type: "schema",
                                        });
                                      }}
                                    >
                                      {schemaNode.tables.map((table) =>
                                        renderTableNode(
                                          table,
                                          databaseLevel + 2,
                                        ),
                                      )}
                                    </TreeNode>
                                  );
                                })
                              : database.tables.map((table) =>
                                  renderTableNode(table, databaseLevel + 1),
                                )}
                          </TreeNode>
                        );
                      })}
                    </TreeNode>
                  ) : (
                    visibleDatabases.map((database) => {
                      const databaseLevel = 1;
                      const dbKey = `${connection.id}-${database.name}`;
                      const supportsSchemaNode = supportsSchemaNodeForDriver(
                        connection.type,
                      );
                      const renderTableNode = (
                        table: TableInfo,
                        level: number,
                      ) => {
                        const tableKey = getTableNodeKey(
                          connection.id,
                          database.name,
                          table.schema,
                          table.name,
                        );
                        return (
                          <ContextMenu key={tableKey}>
                            <ContextMenuTrigger asChild>
                              <div
                                ref={(el) => {
                                  tableNodeRefs.current[tableKey] = el;
                                }}
                              >
                                <TreeNode
                                  level={level}
                                  icon={<Table className="w-4 h-4" />}
                                  label={table.name}
                                  isSelected={selectedTableKey === tableKey}
                                  isExpanded={expandedTables.has(tableKey)}
                                  toggleOnRowClick={false}
                                  onToggle={() => {
                                    toggleTable(
                                      tableKey,
                                      connection.id,
                                      database.name,
                                      table,
                                    );
                                  }}
                                  onDoubleClick={() => {
                                    handleTableClick(
                                      connection,
                                      database,
                                      table,
                                    );
                                  }}
                                  statusIndicator={
                                    loadingTableKeys.has(tableKey) ? (
                                      <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
                                    ) : undefined
                                  }
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
                                      style={{
                                        paddingLeft: `${(level + 1) * 12 + 8}px`,
                                      }}
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
                              </div>
                            </ContextMenuTrigger>
                            <ContextMenuContent>
                              <ContextMenuItem
                                onClick={() =>
                                  handleCreateQueryFromContext(
                                    connection.id,
                                    database.name,
                                  )
                                }
                              >
                                <FileCode className="w-4 h-4 mr-2" />
                                {t("connection.menu.newQuery")}
                              </ContextMenuItem>
                              <ContextMenuItem
                                onClick={() =>
                                  void handleTableExport(
                                    connection,
                                    database,
                                    table,
                                    "csv",
                                  )
                                }
                              >
                                <Download className="w-4 h-4 mr-2" />
                                {t("connection.menu.exportCsv")}
                              </ContextMenuItem>
                              <ContextMenuItem
                                onClick={() =>
                                  void handleTableExport(
                                    connection,
                                    database,
                                    table,
                                    "json",
                                  )
                                }
                              >
                                <Download className="w-4 h-4 mr-2" />
                                {t("connection.menu.exportJson")}
                              </ContextMenuItem>
                              <ContextMenuItem
                                onClick={() =>
                                  void handleTableExport(
                                    connection,
                                    database,
                                    table,
                                    "sql",
                                  )
                                }
                              >
                                <Download className="w-4 h-4 mr-2" />
                                {t("connection.menu.exportSql")}
                              </ContextMenuItem>
                            </ContextMenuContent>
                          </ContextMenu>
                        );
                      };
                      return (
                        <TreeNode
                          key={dbKey}
                          level={databaseLevel}
                          icon={<Database className="w-4 h-4" />}
                          label={
                            (connection.type === "sqlite" ||
                              connection.type === "duckdb") &&
                            database.name === "main"
                              ? t(
                                  connection.type === "duckdb"
                                    ? "connection.duckdbMainLabel"
                                    : "connection.sqliteMainLabel",
                                )
                              : database.name
                          }
                          isExpanded={expandedDatabases.has(dbKey)}
                          onToggle={() => toggleDatabase(dbKey)}
                          statusIndicator={
                            loadingDatabaseKeys.has(dbKey) ? (
                              <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
                            ) : undefined
                          }
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
                          {supportsSchemaNode
                            ? database.schemas.map((schemaNode) => {
                                const schemaKey = getSchemaNodeKey(
                                  dbKey,
                                  schemaNode.name,
                                );
                                return (
                                  <TreeNode
                                    key={schemaKey}
                                    level={databaseLevel + 1}
                                    icon={<FolderOpen className="w-4 h-4" />}
                                    label={schemaNode.name}
                                    isExpanded={expandedSchemas.has(schemaKey)}
                                    onToggle={() => toggleSchema(schemaKey)}
                                    onContextMenu={(e) => {
                                      e.preventDefault();
                                      e.stopPropagation();
                                      setContextMenu({
                                        visible: true,
                                        x: e.clientX,
                                        y: e.clientY,
                                        connectionId: connection.id,
                                        databaseName: database.name,
                                        schemaName: schemaNode.name,
                                        type: "schema",
                                      });
                                    }}
                                  >
                                    {schemaNode.tables.map((table) =>
                                      renderTableNode(table, databaseLevel + 2),
                                    )}
                                  </TreeNode>
                                );
                              })
                            : database.tables.map((table) =>
                                renderTableNode(table, databaseLevel + 1),
                              )}
                        </TreeNode>
                      );
                    })
                  )
                ) : null}
              </>
            </TreeNode>
          );
        })}
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
                  if (contextMenu.connectionId) {
                    openEditDialog(contextMenu.connectionId);
                  }
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <Edit3 className="w-4 h-4" />
                {t("connection.menu.edit")}
              </button>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={async () => {
                  if (contextMenu.connectionId) {
                    await handleDuplicateConnection(contextMenu.connectionId);
                  }
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <Copy className="w-4 h-4" />
                {t("connection.menu.copy")}
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
                <RefreshCw className="w-4 h-4" />
                {t("connection.menu.refresh")}
              </button>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={() => {
                  handleCreateQueryFromContext(contextMenu.connectionId);
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <FileCode className="w-4 h-4" />
                {t("connection.menu.newQuery")}
              </button>
              {contextMenuConnection &&
              supportsCreateDatabaseForDriver(contextMenuConnection.type) ? (
                <button
                  className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                  onClick={() => {
                    openCreateDatabaseDialog(contextMenuConnection.id);
                    setContextMenu((prev) => ({ ...prev, visible: false }));
                  }}
                >
                  <Plus className="w-4 h-4" />
                  {t("connection.menu.newDatabase")}
                </button>
              ) : null}
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
                {t("connection.menu.delete")}
              </button>
            </>
          ) : contextMenu.type === "database" ? (
            <>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={async () => {
                  if (contextMenu.connectionId && contextMenu.databaseName) {
                    await handleRefreshDatabaseTables(
                      contextMenu.connectionId,
                      contextMenu.databaseName,
                    );
                  }
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <RefreshCw className="w-4 h-4" />
                {t("connection.menu.refreshTables")}
              </button>
              {contextMenu.connectionId &&
              contextMenu.databaseName &&
              contextMenuDatabaseConnection &&
              getImportDriverCapability(contextMenuDatabaseConnection.type) !==
                "unsupported" ? (
                <button
                  className="w-full px-3 py-2 text-left text-sm hover:bg-accent disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-2"
                  disabled={
                    getImportDriverCapability(
                      contextMenuDatabaseConnection.type,
                    ) === "read_only_not_supported"
                  }
                  onClick={async () => {
                    await handleDatabaseImport(
                      contextMenu.connectionId!,
                      contextMenu.databaseName!,
                    );
                    setContextMenu((prev) => ({ ...prev, visible: false }));
                  }}
                >
                  <Upload className="w-4 h-4" />
                  {getImportDriverCapability(
                    contextMenuDatabaseConnection.type,
                  ) === "read_only_not_supported"
                    ? t("connection.menu.importSqlReadOnly")
                    : t("connection.menu.importSql")}
                </button>
              ) : null}
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={() => {
                  handleCreateQueryFromContext(
                    contextMenu.connectionId,
                    contextMenu.databaseName,
                  );
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <FileCode className="w-4 h-4" />
                {t("connection.menu.newQuery")}
              </button>
            </>
          ) : contextMenu.type === "schema" ? (
            <>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={async () => {
                  if (contextMenu.connectionId && contextMenu.databaseName) {
                    await handleRefreshDatabaseTables(
                      contextMenu.connectionId,
                      contextMenu.databaseName,
                    );
                  }
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <RefreshCw className="w-4 h-4" />
                {t("connection.menu.refreshTables")}
              </button>
              <button
                className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
                onClick={() => {
                  handleCreateQueryFromContext(
                    contextMenu.connectionId,
                    contextMenu.databaseName,
                  );
                  setContextMenu((prev) => ({ ...prev, visible: false }));
                }}
              >
                <FileCode className="w-4 h-4" />
                {t("connection.menu.newQuery")}
              </button>
            </>
          ) : null}
        </div>
      )}
      <Dialog
        open={isCreateDbDialogOpen}
        onOpenChange={(open) => {
          setIsCreateDbDialogOpen(open);
          if (!open) {
            setCreateDbValidationMsg(null);
            setCreateDbConnectionId(null);
            setShowCreateDbAdvanced(false);
            setCreateDbForm(defaultCreateDatabaseForm);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("connection.createDbDialog.title")}</DialogTitle>
          </DialogHeader>
          <div className="grid gap-4">
            <div className="grid gap-2">
              <Label htmlFor="create-db-name">
                {t("connection.createDbDialog.fields.name")}{" "}
                <span className="text-red-600">*</span>
              </Label>
              <Input
                id="create-db-name"
                value={createDbForm.name}
                onChange={(e) =>
                  setCreateDbForm((prev) => ({ ...prev, name: e.target.value }))
                }
                placeholder={t("connection.createDbDialog.placeholders.name")}
              />
            </div>
            <div className="flex items-center space-x-2">
              <Checkbox
                id="create-db-if-not-exists"
                checked={createDbForm.ifNotExists}
                onCheckedChange={(checked) =>
                  setCreateDbForm((prev) => ({
                    ...prev,
                    ifNotExists: checked === true,
                  }))
                }
              />
              <Label htmlFor="create-db-if-not-exists">
                {t("connection.createDbDialog.fields.ifNotExists")}
              </Label>
            </div>
            <div>
              <Button
                type="button"
                variant="ghost"
                className="h-8 px-0"
                onClick={() => setShowCreateDbAdvanced((prev) => !prev)}
              >
                {showCreateDbAdvanced
                  ? t("connection.createDbDialog.hideAdvanced")
                  : t("connection.createDbDialog.showAdvanced")}
              </Button>
            </div>
            {showCreateDbAdvanced && (
              <div className="border p-3 rounded-md space-y-3 bg-muted/20">
                {isMySqlFamilyCreateDb && (
                  <>
                    <div className="grid gap-2">
                      <Label htmlFor="create-db-charset">
                        {t("connection.createDbDialog.fields.charset")}
                      </Label>
                      <Select
                        value={createDbForm.charset || createDbNoneOption}
                        onValueChange={(v) =>
                          setCreateDbForm((prev) => ({
                            ...prev,
                            charset: v === createDbNoneOption ? "" : v,
                          }))
                        }
                      >
                        <SelectTrigger id="create-db-charset">
                          <SelectValue
                            placeholder={t(
                              "connection.createDbDialog.placeholders.charset",
                            )}
                          />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value={createDbNoneOption}>
                            {t("connection.createDbDialog.defaultOption")}
                          </SelectItem>
                          {mysqlCharsetOptions.map((opt) => (
                            <SelectItem key={opt} value={opt}>
                              {opt}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                    <div className="grid gap-2">
                      <Label htmlFor="create-db-collation">
                        {t("connection.createDbDialog.fields.collation")}
                      </Label>
                      <Select
                        value={createDbForm.collation || createDbNoneOption}
                        onValueChange={(v) =>
                          setCreateDbForm((prev) => ({
                            ...prev,
                            collation: v === createDbNoneOption ? "" : v,
                          }))
                        }
                      >
                        <SelectTrigger id="create-db-collation">
                          <SelectValue
                            placeholder={t(
                              "connection.createDbDialog.placeholders.collation",
                            )}
                          />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value={createDbNoneOption}>
                            {t("connection.createDbDialog.defaultOption")}
                          </SelectItem>
                          {mysqlCollationOptions.map((opt) => (
                            <SelectItem key={opt} value={opt}>
                              {opt}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                  </>
                )}
                {isPostgresCreateDb && (
                  <>
                    <div className="grid gap-2">
                      <Label htmlFor="create-db-encoding">
                        {t("connection.createDbDialog.fields.encoding")}
                      </Label>
                      <Select
                        value={createDbForm.encoding || createDbNoneOption}
                        onValueChange={(v) =>
                          setCreateDbForm((prev) => ({
                            ...prev,
                            encoding: v === createDbNoneOption ? "" : v,
                          }))
                        }
                      >
                        <SelectTrigger id="create-db-encoding">
                          <SelectValue
                            placeholder={t(
                              "connection.createDbDialog.placeholders.encoding",
                            )}
                          />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value={createDbNoneOption}>
                            {t("connection.createDbDialog.defaultOption")}
                          </SelectItem>
                          {postgresEncodingOptions.map((opt) => (
                            <SelectItem key={opt} value={opt}>
                              {opt}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                    <div className="grid gap-2">
                      <Label htmlFor="create-db-lc-collate">
                        {t("connection.createDbDialog.fields.lcCollate")}
                      </Label>
                      <Select
                        value={createDbForm.lcCollate || createDbNoneOption}
                        onValueChange={(v) =>
                          setCreateDbForm((prev) => ({
                            ...prev,
                            lcCollate: v === createDbNoneOption ? "" : v,
                          }))
                        }
                      >
                        <SelectTrigger id="create-db-lc-collate">
                          <SelectValue
                            placeholder={t(
                              "connection.createDbDialog.placeholders.lcCollate",
                            )}
                          />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value={createDbNoneOption}>
                            {t("connection.createDbDialog.defaultOption")}
                          </SelectItem>
                          {postgresLocaleOptions.map((opt) => (
                            <SelectItem key={opt} value={opt}>
                              {opt}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                    <div className="grid gap-2">
                      <Label htmlFor="create-db-lc-ctype">
                        {t("connection.createDbDialog.fields.lcCtype")}
                      </Label>
                      <Select
                        value={createDbForm.lcCtype || createDbNoneOption}
                        onValueChange={(v) =>
                          setCreateDbForm((prev) => ({
                            ...prev,
                            lcCtype: v === createDbNoneOption ? "" : v,
                          }))
                        }
                      >
                        <SelectTrigger id="create-db-lc-ctype">
                          <SelectValue
                            placeholder={t(
                              "connection.createDbDialog.placeholders.lcCtype",
                            )}
                          />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value={createDbNoneOption}>
                            {t("connection.createDbDialog.defaultOption")}
                          </SelectItem>
                          {postgresLocaleOptions.map((opt) => (
                            <SelectItem key={opt} value={opt}>
                              {opt}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                  </>
                )}
                {isMssqlCreateDb && (
                  <div className="grid gap-2">
                    <Label htmlFor="create-db-collation">
                      {t("connection.createDbDialog.fields.collation")}
                    </Label>
                    <Select
                      value={createDbForm.collation || createDbNoneOption}
                      onValueChange={(v) =>
                        setCreateDbForm((prev) => ({
                          ...prev,
                          collation: v === createDbNoneOption ? "" : v,
                        }))
                      }
                    >
                      <SelectTrigger id="create-db-collation">
                        <SelectValue
                          placeholder={t(
                            "connection.createDbDialog.placeholders.collation",
                          )}
                        />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value={createDbNoneOption}>
                          {t("connection.createDbDialog.defaultOption")}
                        </SelectItem>
                        {mssqlCollationOptions.map((opt) => (
                          <SelectItem key={opt} value={opt}>
                            {opt}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                )}
              </div>
            )}
            {createDbValidationMsg && (
              <Alert variant="destructive">
                <AlertTitle>
                  {t("connection.dialog.validationFailed")}
                </AlertTitle>
                <AlertDescription>{createDbValidationMsg}</AlertDescription>
              </Alert>
            )}
            <div className="flex justify-end gap-2">
              <Button
                type="button"
                variant="outline"
                disabled={isCreatingDatabase}
                onClick={() => setIsCreateDbDialogOpen(false)}
              >
                {t("common.cancel")}
              </Button>
              <Button
                type="button"
                disabled={isCreatingDatabase}
                onClick={() => void handleCreateDatabase()}
              >
                {isCreatingDatabase ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    {t("connection.createDbDialog.creating")}
                  </>
                ) : (
                  t("connection.createDbDialog.confirm")
                )}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
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
            <AlertDialogTitle>
              {t("connection.deleteDialog.title")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("connection.deleteDialog.description")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={isDeleting}>
              {t("common.cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              disabled={isDeleting || !deleteTargetConnectionId}
              onClick={async (e) => {
                e.preventDefault();
                if (!deleteTargetConnectionId) return;
                await handleDeleteConnection(deleteTargetConnectionId);
              }}
            >
              {isDeleting
                ? t("connection.deleteDialog.deleting")
                : t("common.delete")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <AlertDialog
        open={isImportConfirmOpen}
        onOpenChange={(open) => {
          setIsImportConfirmOpen(open);
          if (!open && !isImportingSql) {
            setPendingImport(null);
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("connection.importDialog.title")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("connection.importDialog.description", {
                database: pendingImport?.databaseName || "",
              })}
            </AlertDialogDescription>
            <div className="text-xs text-muted-foreground font-mono break-all mt-2">
              {pendingImport?.filePath || ""}
            </div>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={isImportingSql}>
              {t("common.cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              disabled={isImportingSql || !pendingImport}
              onClick={async (e) => {
                e.preventDefault();
                await handleConfirmImport();
              }}
            >
              {isImportingSql ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  {t("connection.importDialog.importing")}
                </>
              ) : (
                t("connection.importDialog.confirm")
              )}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
