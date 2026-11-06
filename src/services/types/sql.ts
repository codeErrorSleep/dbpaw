import type { RedisConnectionMode } from "./redis";
import type { Driver } from "@/lib/driver-registry";

export type { Driver, ImportDriverCapability } from "@/lib/driver-registry";

export interface QueryColumn {
  name: string;
  type: string;
}

export interface SingleResultSet {
  data: any[];
  rowCount: number;
  columns: QueryColumn[];
  index: number;
  statement: string;
}

export interface QueryResult {
  data: any[];
  rowCount: number;
  columns: QueryColumn[];
  timeTakenMs: number;
  success: boolean;
  error?: string;
  resultSets?: SingleResultSet[];
}

export interface QueryExecutionMetadata {
  queryId: string;
  originalSql: string;
  executedSql: string;
  defaultLimitApplied: boolean;
  defaultLimit?: number;
}

export interface QueryExecutionResult extends QueryResult {
  execution: QueryExecutionMetadata;
}

export type SqlRisk = "read" | "write" | "ddl" | "transaction" | "unknown";

export type SqlRiskReason =
  | "write_statement"
  | "missing_where"
  | "schema_change"
  | "transaction_control"
  | "unknown_statement"
  | "multiple_statements";

export interface SqlRiskAnalysis {
  risk: SqlRisk;
  requiresConfirmation: boolean;
  statementCount: number;
  reasons: SqlRiskReason[];
}

export type SqlExecutionSource =
  | "sql_editor"
  | "table_view_save"
  | "execute_by_conn"
  | "unknown";

export interface SqlExecutionLog {
  id: number;
  sql: string;
  source?: string | null;
  connectionId?: number | null;
  database?: string | null;
  success: boolean;
  error?: string | null;
  executedAt: string;
}

export interface NetworkProxy {
  enabled: boolean;
  kind: "socks5" | "http";
  host: string;
  port: number;
  username: string;
  password: string;
}

export interface ConnectionForm {
  driver: Driver;
  name?: string;
  host?: string;
  port?: number;
  database?: string;
  schema?: string;
  username?: string;
  password?: string;
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
  authMode?: string;
  apiKeyId?: string;
  apiKeySecret?: string;
  apiKeyEncoded?: string;
  cloudId?: string;
  authSource?: string;
}

export interface SavedConnection {
  id: number;
  uuid: string;
  name: string;
  dbType: string;
  host: string;
  port: number;
  database: string;
  username: string;
  ssl: boolean;
  sslMode?: "require" | "verify_ca";
  sslCaCert?: string | null;
  filePath?: string | null;
  sshEnabled: boolean;
  sshHost?: string | null;
  sshPort?: number | null;
  sshUsername?: string | null;
  sshPassword?: string | null;
  sshKeyPath?: string | null;
  proxy?: NetworkProxy | null;
  mode?: RedisConnectionMode | null;
  seedNodes?: string[] | null;
  sentinels?: string[] | null;
  connectTimeoutMs?: number | null;
  serviceName?: string | null;
  sentinelPassword?: string | null;
  authMode?: "none" | "basic" | "api_key" | null;
  apiKeyId?: string | null;
  apiKeySecret?: string | null;
  apiKeyEncoded?: string | null;
  cloudId?: string | null;
  authSource?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ImportResult {
  imported: SavedConnection[];
  skipped: number;
}

export interface CreateDatabasePayload {
  name: string;
  ifNotExists?: boolean;
  charset?: string;
  collation?: string;
  encoding?: string;
  lcCollate?: string;
  lcCtype?: string;
}

export interface TestConnectionResult {
  success: boolean;
  message: string;
  latencyMs?: number;
}

export interface ColumnSchema {
  name: string;
  type: string;
}

export interface ColumnInfo {
  name: string;
  type: string;
  nullable: boolean;
  defaultValue?: string | null;
  primaryKey: boolean;
  comment?: string | null;
  defaultConstraintName?: string | null;
}

export interface IndexInfo {
  name: string;
  unique: boolean;
  indexType?: string | null;
  columns: string[];
}

export interface ForeignKeyInfo {
  name: string;
  column: string;
  referencedSchema?: string | null;
  referencedTable: string;
  referencedColumn: string;
  onUpdate?: string | null;
  onDelete?: string | null;
}

export interface ClickHouseTableExtra {
  engine: string;
  partitionKey?: string | null;
  sortingKey?: string | null;
  primaryKeyExpr?: string | null;
  samplingKey?: string | null;
  ttlExpr?: string | null;
  createTableQuery?: string | null;
}

export interface CassandraTableExtra {
  partitionKey: string[];
  clusteringColumns: string[];
  compactionStrategy: string;
  bloomFilterFpChance: number;
  caching: any;
  gcGraceSeconds: number;
  defaultTimeToLive: number;
}

export type SpecialTypeCategory = "bitmap" | "geo" | "hyperloglog";

export interface SpecialTypeSummary {
  columnName: string;
  category: SpecialTypeCategory;
  typeName: string;
  declaredLength?: string | null;
  memoryUsageBytes?: number | null;
  memoryUsageDisplay?: string | null;
  rawType: string;
  notes?: string | null;
}

export interface TableMetadata {
  columns: ColumnInfo[];
  indexes: IndexInfo[];
  foreignKeys: ForeignKeyInfo[];
  clickhouseExtra?: ClickHouseTableExtra | null;
  cassandraExtra?: CassandraTableExtra | null;
  specialTypeSummaries: SpecialTypeSummary[];
}

export interface SchemaForeignKey {
  name: string;
  sourceTable: string;
  sourceSchema?: string | null;
  sourceColumn: string;
  targetTable: string;
  targetSchema?: string | null;
  targetColumn: string;
  onUpdate?: string | null;
  onDelete?: string | null;
}

export type RoutineType = "procedure" | "function";

export interface RoutineInfo {
  schema: string;
  name: string;
  type: RoutineType;
}

export interface EventInfo {
  schema: string;
  name: string;
  status: string;
  eventType: string;
  executeAt: string | null;
  intervalValue: string | null;
  lastExecuted: string | null;
  definition: string | null;
}

export interface SequenceInfo {
  schema: string;
  name: string;
  dataType: string;
  startValue: string | null;
  increment: string | null;
}

export interface TypeInfo {
  schema: string;
  name: string;
  category: string;
}

export interface SynonymInfo {
  schema: string;
  name: string;
  baseObjectType: string;
}

export interface PackageInfo {
  schema: string;
  name: string;
  objectType: string;
}

export interface TableSchema {
  schema: string;
  name: string;
  columns: ColumnSchema[];
}

export interface SchemaOverview {
  tables: TableSchema[];
}

export interface SavedQuery {
  id: number;
  name: string;
  query: string;
  description?: string | null;
  connectionId?: number | null;
  database?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface SqliteConnectionIssue {
  id: number;
  connectionId: number;
  connectionName: string;
  filePath: string;
  issueType:
    | "locked"
    | "corrupted"
    | "permission_denied"
    | "not_found"
    | string;
  description: string;
  detectedAt: string;
  resolvedAt?: string | null;
}
