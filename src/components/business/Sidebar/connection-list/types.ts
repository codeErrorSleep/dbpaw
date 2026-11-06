import type { ReactNode } from "react";
import type {
  Driver,
  NetworkProxy,
  RedisConnectionMode,
  RoutineType,
} from "@/services/api";
import type { DatabaseGroupConfig } from "@/lib/tree-adapters/types";

export interface Column {
  name: string;
  type: string;
  isPrimaryKey?: boolean;
  nullable?: boolean;
}

export interface TableInfo {
  name: string;
  schema: string;
  columns: Column[];
  isSystem?: boolean;
  indexStatus?: string | null;
  type?: string;
}

export interface RoutineInfo {
  name: string;
  schema: string;
  type: RoutineType;
}

export interface SchemaInfo {
  name: string;
  tables: TableInfo[];
  procedures: RoutineInfo[];
  functions: RoutineInfo[];
}

export interface DatabaseInfo {
  name: string;
  schemas: SchemaInfo[];
  tables: TableInfo[];
  routines: RoutineInfo[];
  redisCursor?: string;
  redisIsPartial?: boolean;
  redisRequiresPattern?: boolean;
  redisKeyCount?: number;
}

export type DatabaseExportFormat = "sql_dml" | "sql_ddl" | "sql_full";
export type TableExportFormat = "csv" | "json" | "sql_dml" | "sql_ddl" | "sql_full";

export interface Connection {
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
  proxy?: NetworkProxy;
  mode?: RedisConnectionMode;
  seedNodes?: string[];
  sentinels?: string[];
  connectTimeoutMs?: number;
  serviceName?: string;
  sentinelPassword?: string;
  authMode?: "none" | "basic" | "api_key";
  apiKeyId?: string;
  apiKeySecret?: string;
  apiKeyEncoded?: string;
  cloudId?: string;
  authSource?: string;
  databases: DatabaseInfo[];
  isConnected: boolean;
  connectState: "idle" | "connecting" | "success" | "error";
  connectError?: string;
}

export interface CreateDatabaseForm {
  name: string;
  ifNotExists: boolean;
  charset: string;
  collation: string;
  encoding: string;
  lcCollate: string;
  lcCtype: string;
}

export type SelectedTableNode = {
  key: string;
  connectionId: number;
  database: string;
  table: string;
  schema: string;
};

export interface DatasourceTreeAdapter {
  supportsSchemaNode: boolean;
  isDatabaseExpandable: boolean;
  listDatabases: () => Promise<string[]>;
  loadDatabaseChildren: (databaseName: string) => Promise<TableInfo[]>;
  shouldSkipTableColumns: boolean;
  getItemIcon: () => ReactNode;
  onItemActivate: (database: DatabaseInfo, table: TableInfo) => void;
  getDatabaseRowActions: (database: DatabaseInfo) => ReactNode | undefined;
  onDatabaseDoubleClick?: (database: DatabaseInfo) => void;
  renderDatabaseFooter: (database: DatabaseInfo, level: number) => ReactNode;
  renderTableContextMenu: (
    database: DatabaseInfo,
    table: TableInfo,
  ) => ReactNode;
  renderDatabaseContextMenu?: (databaseName: string) => ReactNode;
  databaseGroups?: DatabaseGroupConfig[];
}
