import { CircleDot, CheckCircle2, XCircle, Loader2 } from "lucide-react";
import type { ConnectionForm } from "@/services/api";
import { buildConnectionFormDefaults } from "@/lib/connection-form/rules";
import type { Connection, TableInfo, RoutineInfo, SchemaInfo } from "./types";

export { getConnectionIcon } from "@/lib/driver-registry";

export interface ConnectionStatusLike {
  connectState: "idle" | "connecting" | "success" | "error";
  connectError?: string;
}

export const sanitizeConnectionErrorMessage = (message: string) =>
  message.replace(/^(?:\s*\[[^\]]+\])+\s*/g, "").trim();

export const getExportDefaultName = (
  tableName: string,
  format: "csv" | "json" | "sql_dml" | "sql_ddl" | "sql_full",
) => {
  const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
  const ext = format === "csv" ? "csv" : format === "json" ? "json" : "sql";
  return `${tableName}_${timestamp}.${ext}`;
};

export const getExportFilter = (
  format: "csv" | "json" | "sql_dml" | "sql_ddl" | "sql_full",
) => {
  if (format === "csv") {
    return [{ name: "CSV", extensions: ["csv"] }];
  }
  if (format === "json") {
    return [{ name: "JSON", extensions: ["json"] }];
  }
  return [{ name: "SQL", extensions: ["sql"] }];
};

export const getConnectionStatusLabel = (connection: ConnectionStatusLike) => {
  if (connection.connectState === "success") return "Connected";
  if (connection.connectState === "error") {
    if (connection.connectError) {
      return `Connection failed: ${connection.connectError}`;
    }
    return "Connection failed";
  }
  if (connection.connectState === "connecting") return "Connecting";
  return "Not connected";
};

type TFunction = (key: string, options?: Record<string, unknown>) => string;

export const getConnectionStatusLabelI18n = (
  connection: ConnectionStatusLike,
  t: TFunction,
) => {
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

export interface ConnectionLike {
  id: string;
  databases: unknown[];
  connectState: "idle" | "connecting" | "success" | "error";
  isConnected: boolean;
}

export const mergeConnections = <T extends ConnectionLike>(
  newConnections: T[],
  previousConnections: T[],
): T[] =>
  newConnections.map((newConn) => {
    const existing = previousConnections.find((p) => p.id === newConn.id);
    return existing
      ? {
          ...newConn,
          databases: existing.databases,
          connectState: existing.connectState,
          isConnected: existing.isConnected,
        }
      : newConn;
  });

export const renderConnectionStatusIndicator = (
  connection: ConnectionStatusLike,
) => {
  if (connection.connectState === "success") {
    return (
      <CheckCircle2 className="h-3.5 w-3.5 text-green-500" aria-hidden="true" />
    );
  }
  if (connection.connectState === "error") {
    return <XCircle className="h-3.5 w-3.5 text-red-500" aria-hidden="true" />;
  }
  if (connection.connectState === "connecting") {
    return (
      <Loader2
        className="h-3.5 w-3.5 text-muted-foreground animate-spin"
        aria-hidden="true"
      />
    );
  }
  return (
    <CircleDot
      className="h-3.5 w-3.5 text-muted-foreground/60"
      aria-hidden="true"
    />
  );
};

export const buildFormFromConnection = (
  connection: Pick<
    Connection,
    | "type"
    | "name"
    | "host"
    | "port"
    | "database"
    | "username"
    | "ssl"
    | "sslMode"
    | "sslCaCert"
    | "filePath"
    | "sshEnabled"
    | "sshHost"
    | "sshPort"
    | "sshUsername"
    | "sshKeyPath"
    | "proxy"
    | "mode"
    | "seedNodes"
    | "sentinels"
    | "connectTimeoutMs"
    | "serviceName"
    | "sentinelPassword"
    | "authMode"
    | "apiKeyId"
    | "apiKeySecret"
    | "apiKeyEncoded"
    | "cloudId"
    | "authSource"
  >,
  overrides: Partial<ConnectionForm> = {},
): ConnectionForm =>
  buildConnectionFormDefaults(connection.type, {
    name: connection.name,
    host: connection.host || "",
    port: Number(connection.port) || undefined,
    database: connection.database || "",
    schema: connection.type === "postgres" ? "public" : "",
    username: connection.username || "",
    password: "",
    ssl: connection.ssl || false,
    sslMode: connection.sslMode || "require",
    sslCaCert: connection.sslCaCert || "",
    filePath: connection.filePath || "",
    sshEnabled: connection.sshEnabled || false,
    sshHost: connection.sshHost || "",
    sshPort: connection.sshPort || undefined,
    sshUsername: connection.sshUsername || "",
    sshPassword: "",
    sshKeyPath: connection.sshKeyPath || "",
    proxy: connection.proxy ? { ...connection.proxy, password: "" } : undefined,
    mode: connection.mode,
    seedNodes: connection.seedNodes || [],
    sentinels: connection.sentinels || [],
    connectTimeoutMs: connection.connectTimeoutMs,
    serviceName: connection.serviceName || "",
    sentinelPassword: "",
    authMode: connection.authMode || "none",
    apiKeyId: connection.apiKeyId || "",
    apiKeySecret: "",
    apiKeyEncoded: "",
    cloudId: connection.cloudId || "",
    authSource: connection.authSource || "",
    ...overrides,
  });

export function groupSqlObjectsBySchema(
  tables: TableInfo[],
  routines: RoutineInfo[],
): SchemaInfo[] {
  const groupedTables = tables.reduce<Record<string, TableInfo[]>>(
    (acc, table) => {
      const schemaName = (table.schema || "").trim() || "public";
      const current = acc[schemaName] || [];
      current.push(table);
      acc[schemaName] = current;
      return acc;
    },
    {},
  );
  const groupedRoutines = routines.reduce<Record<string, RoutineInfo[]>>(
    (acc, routine) => {
      const schemaName = (routine.schema || "").trim() || "dbo";
      const current = acc[schemaName] || [];
      current.push(routine);
      acc[schemaName] = current;
      return acc;
    },
    {},
  );
  const schemaNames = Array.from(
    new Set([...Object.keys(groupedTables), ...Object.keys(groupedRoutines)]),
  ).sort((a, b) => a.localeCompare(b));

  return schemaNames.map((name) => {
    const schemaTables = groupedTables[name] || [];
    const schemaRoutines = groupedRoutines[name] || [];
    return {
      name,
      tables: [...schemaTables].sort((a, b) => a.name.localeCompare(b.name)),
      procedures: schemaRoutines
        .filter((routine) => routine.type === "procedure")
        .sort((a, b) => a.name.localeCompare(b.name)),
      functions: schemaRoutines
        .filter((routine) => routine.type === "function")
        .sort((a, b) => a.name.localeCompare(b.name)),
    };
  });
}
