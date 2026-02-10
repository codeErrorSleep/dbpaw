import { invoke as tauriInvoke } from "@tauri-apps/api/core";

// Helper to check if running in Tauri
export const isTauri = () => {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
};

// Safe invoke wrapper
const invoke = async <T>(cmd: string, args?: any): Promise<T> => {
  if (!isTauri()) {
    console.warn(`[Mock] invoke ${cmd}`, args);
    throw new Error("Tauri API not available. Please run in Tauri window.");
  }
  return tauriInvoke(cmd, args);
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

export type Driver = "postgres" | "sqlite" | "mysql";
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

export interface TableSchema {
  schema: string;
  name: string;
  columns: ColumnSchema[];
}

export interface SchemaOverview {
  tables: TableSchema[];
}

export const api = {
  query: {
    execute: (id: number, query: string, database?: string) =>
      invoke<QueryResult>("execute_query", { id, query, database }),
    cancel: (uuid: string, queryId: string) =>
      invoke<boolean>("cancel_query", { uuid, queryId }),
    executeByConn: (form: ConnectionForm, sql: string) =>
      invoke<QueryResult>("execute_by_conn", { form, sql }),
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
      schema: string;
      table: string;
      page: number;
      limit: number;
      filter?: string;
      sortColumn?: string;
      sortDirection?: "asc" | "desc";
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
  connections: {
    list: () => invoke<any[]>("get_connections"),
    create: (form: ConnectionForm) => invoke<any>("create_connection", { form }),
    testEphemeral: (form: ConnectionForm) =>
      invoke<TestConnectionResult>("test_connection_ephemeral", { form }),
  },
};
