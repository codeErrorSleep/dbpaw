import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { invokeMock } from "./mocks";

// Helper to check if running in Tauri
export const isTauri = () => {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
};

// Helper to check if Mock mode is enabled
const useMockMode = () => {
  return import.meta.env.VITE_USE_MOCK === "true";
};

// Safe invoke wrapper
const invoke = async <T>(cmd: string, args?: any): Promise<T> => {
  // If running in Tauri, use real Tauri invoke
  if (isTauri()) {
    return tauriInvoke(cmd, args);
  }

  // If not in Tauri, check if Mock mode is enabled
  if (useMockMode()) {
    return invokeMock<T>(cmd, args);
  }

  // If not in Tauri and Mock mode is disabled, throw error
  console.warn(`[API] invoke ${cmd}`, args);
  throw new Error(
    "Tauri API not available. Please run 'bun tauri dev' or enable Mock mode with 'VITE_USE_MOCK=true'.",
  );
};

export interface QueryColumn {
  name: string;
  type: string;
}

export interface QueryResult {
  data: any[];
  rowCount: number;
  columns: QueryColumn[];
  timeTakenMs: number;
  success: boolean;
  error?: string;
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

export type Driver = "postgres" | "sqlite" | "mysql" | "clickhouse" | "mssql";
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
  filePath?: string;
  sshEnabled?: boolean;
  sshHost?: string;
  sshPort?: number;
  sshUsername?: string;
  sshPassword?: string;
  sshKeyPath?: string;
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

export interface TableMetadata {
  columns: ColumnInfo[];
  indexes: IndexInfo[];
  foreignKeys: ForeignKeyInfo[];
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

export interface AIProviderConfig {
  id: number;
  name: string;
  providerType: AIProviderType;
  baseUrl: string;
  model: string;
  hasApiKey: boolean;
  isDefault: boolean;
  enabled: boolean;
  extraJson?: string | null;
  createdAt: string;
  updatedAt: string;
}

export type AIProviderType = string;

export interface AIProviderForm {
  name: string;
  providerType?: AIProviderType;
  baseUrl: string;
  model: string;
  apiKey?: string;
  isDefault?: boolean;
  enabled?: boolean;
  extraJson?: string;
}

export interface AIUsage {
  promptTokens?: number | null;
  completionTokens?: number | null;
  totalTokens?: number | null;
}

export interface AIConversation {
  id: number;
  title: string;
  scenario: string;
  connectionId?: number | null;
  database?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface AIMessage {
  id: number;
  conversationId: number;
  role: "system" | "developer" | "user" | "assistant" | "tool" | string;
  content: string;
  promptVersion?: string | null;
  model?: string | null;
  tokenIn?: number | null;
  tokenOut?: number | null;
  latencyMs?: number | null;
  createdAt: string;
}

export interface AIConversationDetail {
  conversation: AIConversation;
  messages: AIMessage[];
}

export interface AITableSummary {
  schema: string;
  name: string;
  columns: { name: string; type: string; nullable?: boolean }[];
}

export interface AISchemaOverview {
  tables: AITableSummary[];
}

export interface AISelectedTableRef {
  schema: string;
  name: string;
}

export interface AIChatRequest {
  requestId: string;
  providerId?: number;
  conversationId?: number;
  scenario: "sql_generate" | "sql_optimize" | "sql_explain" | string;
  input: string;
  title?: string;
  connectionId?: number;
  database?: string;
  schemaOverview?: AISchemaOverview;
  selectedTables?: AISelectedTableRef[];
}

export interface AIChatResponse {
  conversationId: number;
  userMessageId: number;
  assistantMessageId: number;
}

export type TransferFormat = "csv" | "json" | "sql";
export type ExportScope =
  | "current_page"
  | "filtered"
  | "full_table"
  | "query_result";

export interface ExportResult {
  filePath: string;
  rowCount: number;
}

export const api = {
  query: {
    execute: (
      id: number,
      query: string,
      database?: string,
      source?: SqlExecutionSource,
    ) => invoke<QueryResult>("execute_query", { id, query, database, source }),
    cancel: (uuid: string, queryId: string) =>
      invoke<boolean>("cancel_query", { uuid, queryId }),
    executeByConn: (form: ConnectionForm, sql: string) =>
      invoke<QueryResult>("execute_by_conn", { form, sql }),
  },
  sqlLogs: {
    list: (limit = 100) =>
      invoke<SqlExecutionLog[]>("list_sql_execution_logs", { limit }),
  },
  metadata: {
    listTables: (id: number, database?: string, schema?: string) =>
      invoke<{ schema: string; name: string; type: string }[]>("list_tables", {
        id,
        database,
        schema,
      }),
    getTableStructure: (id: number, schema: string, table: string) =>
      invoke<{ columns: { name: string; type: string; nullable: boolean }[] }>(
        "get_table_structure",
        { id, schema, table },
      ),
    getTableDDL: (
      id: number,
      database: string | undefined,
      schema: string,
      table: string,
    ) => invoke<string>("get_table_ddl", { id, database, schema, table }),
    getTableMetadata: (
      id: number,
      database: string | undefined,
      schema: string,
      table: string,
    ) =>
      invoke<TableMetadata>("get_table_metadata", {
        id,
        database,
        schema,
        table,
      }),
    listTablesByConn: (form: ConnectionForm) =>
      invoke<{ schema: string; name: string; type: string }[]>(
        "list_tables_by_conn",
        { form },
      ),
    listDatabases: (form: ConnectionForm) =>
      invoke<string[]>("list_databases", { form }),
    listDatabasesById: (id: number) =>
      invoke<string[]>("list_databases_by_id", { id }),
    getSchemaOverview: (id: number, database?: string, schema?: string) =>
      invoke<SchemaOverview>("get_schema_overview", { id, database, schema }),
  },
  tableData: {
    get: (params: {
      id: number;
      database?: string;
      schema: string;
      table: string;
      page: number;
      limit: number;
      filter?: string;
      sortColumn?: string;
      sortDirection?: "asc" | "desc";
      orderBy?: string;
    }) =>
      invoke<{
        data: any[];
        total: number;
        page: number;
        limit: number;
        executionTimeMs: number;
      }>("get_table_data", params),
    getByConn: (
      form: ConnectionForm,
      schema: string,
      table: string,
      page: number,
      limit: number,
    ) =>
      invoke<{
        data: any[];
        total: number;
        page: number;
        limit: number;
        executionTimeMs: number;
      }>("get_table_data_by_conn", { form, schema, table, page, limit }),
  },
  transfer: {
    exportTable: (params: {
      id: number;
      database?: string;
      schema: string;
      table: string;
      driver: string;
      format: TransferFormat;
      scope: Exclude<ExportScope, "query_result">;
      filter?: string;
      orderBy?: string;
      sortColumn?: string;
      sortDirection?: "asc" | "desc";
      page?: number;
      limit?: number;
      filePath?: string;
      chunkSize?: number;
    }) => invoke<ExportResult>("export_table_data", params),
    exportQueryResult: (params: {
      id: number;
      database?: string;
      sql: string;
      driver: string;
      format: TransferFormat;
      filePath?: string;
    }) => invoke<ExportResult>("export_query_result", params),
  },
  connections: {
    list: () => invoke<any[]>("get_connections"),
    create: (form: ConnectionForm) =>
      invoke<any>("create_connection", { form }),
    update: (id: number, form: ConnectionForm) =>
      invoke<any>("update_connection", { id, form }),
    delete: (id: number) => invoke<void>("delete_connection", { id }),
    testEphemeral: (form: ConnectionForm) =>
      invoke<TestConnectionResult>("test_connection_ephemeral", { form }),
    listSqliteIssues: () =>
      invoke<SqliteConnectionIssue[]>("list_sqlite_issues"),
  },
  queries: {
    list: () => invoke<SavedQuery[]>("get_saved_queries"),
    create: (data: {
      name: string;
      query: string;
      description?: string;
      connectionId?: number;
      database?: string;
    }) => invoke<SavedQuery>("save_query", data),
    update: (
      id: number,
      data: {
        name: string;
        query: string;
        description?: string;
        connectionId?: number;
        database?: string;
      },
    ) => invoke<SavedQuery>("update_saved_query", { id, ...data }),
    delete: (id: number) => invoke<void>("delete_saved_query", { id }),
  },
  ai: {
    providers: {
      list: () => invoke<AIProviderConfig[]>("ai_list_providers"),
      create: (config: AIProviderForm) =>
        invoke<AIProviderConfig>("ai_create_provider", { config }),
      update: (id: number, config: AIProviderForm) =>
        invoke<AIProviderConfig>("ai_update_provider", { id, config }),
      delete: (id: number) => invoke<void>("ai_delete_provider", { id }),
      setDefault: (id: number) =>
        invoke<void>("ai_set_default_provider", { id }),
      clearApiKey: (providerType: string) =>
        invoke<void>("ai_clear_provider_api_key", { provider_type: providerType }),
    },
    chat: {
      start: (request: AIChatRequest) =>
        invoke<AIChatResponse>("ai_chat_start", { request }),
      continue: (request: AIChatRequest) =>
        invoke<AIChatResponse>("ai_chat_continue", { request }),
    },
    conversations: {
      list: (filters?: { connectionId?: number; database?: string }) =>
        invoke<AIConversation[]>("ai_list_conversations", {
          connectionId: filters?.connectionId,
          database: filters?.database,
        }),
      get: (conversationId: number) =>
        invoke<AIConversationDetail>("ai_get_conversation", {
          conversationId,
        }),
      delete: (conversationId: number) =>
        invoke<void>("ai_delete_conversation", { conversationId }),
    },
  },
};
