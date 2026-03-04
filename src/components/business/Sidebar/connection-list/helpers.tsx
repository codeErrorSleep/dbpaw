import type { ReactNode } from "react";
import { Database, Server, CircleDot, CheckCircle2, XCircle, Loader2 } from "lucide-react";
import { siMysql, siPostgresql, siSqlite, siClickhouse, type SimpleIcon } from "simple-icons";
import type { Driver } from "@/services/api";

export interface ConnectionStatusLike {
  connectState: "idle" | "connecting" | "success" | "error";
  connectError?: string;
}

const renderSimpleIcon = (icon: SimpleIcon) => (
  <svg
    viewBox="0 0 24 24"
    width="16"
    height="16"
    aria-hidden="true"
    className="shrink-0"
    role="img"
  >
    <path d={icon.path} fill="currentColor" />
  </svg>
);

export const getConnectionIcon = (driver: Driver | string): ReactNode => {
  const normalized = String(driver || "")
    .trim()
    .toLowerCase();

  switch (normalized) {
    case "postgres":
    case "postgresql":
    case "pgsql":
      return renderSimpleIcon(siPostgresql);
    case "mysql":
    case "mariadb":
      return renderSimpleIcon(siMysql);
    case "sqlite":
    case "sqlite3":
      return renderSimpleIcon(siSqlite);
    case "clickhouse":
      return renderSimpleIcon(siClickhouse);
    case "mssql":
      return <Database className="w-4 h-4" />;
    default:
      return <Server className="w-4 h-4" />;
  }
};

export const sanitizeConnectionErrorMessage = (message: string) =>
  message.replace(/^(?:\s*\[[^\]]+\])+\s*/g, "").trim();

export const getExportDefaultName = (
  tableName: string,
  format: "csv" | "json" | "sql",
) => {
  const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
  return `${tableName}_${timestamp}.${format}`;
};

export const getExportFilter = (format: "csv" | "json" | "sql") => {
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
