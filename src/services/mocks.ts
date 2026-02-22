import {
  QueryResult,
  ColumnSchema,
  TableMetadata,
  TableSchema,
  SchemaOverview,
  ConnectionForm,
  TestConnectionResult,
} from "./api";

/**
 * Mock 数据层 - 为所有 API 命令提供 Mock 实现
 * 在非 Tauri 环境下用于前端独立开发调试
 */

// ==================== Mock 数据 ====================

export const mockConnections: any[] = [
  {
    id: 1,
    name: "PostgreSQL Dev",
    driver: "postgres",
    host: "localhost",
    port: 5432,
    database: "testdb",
    username: "postgres",
    createdAt: new Date().toISOString(),
  },
  {
    id: 2,
    name: "SQLite Local",
    driver: "sqlite",
    filePath: "/path/to/database.db",
    createdAt: new Date().toISOString(),
  },
];

export const mockTables: { schema: string; name: string; type: string }[] = [
  { schema: "public", name: "users", type: "table" },
  { schema: "public", name: "posts", type: "table" },
  { schema: "public", name: "comments", type: "table" },
  { schema: "public", name: "tags", type: "table" },
];

export const mockTableStructure = {
  columns: [
    { name: "id", type: "integer", nullable: false },
    { name: "username", type: "varchar", nullable: false },
    { name: "email", type: "varchar", nullable: false },
    { name: "created_at", type: "timestamp", nullable: true },
    { name: "updated_at", type: "timestamp", nullable: true },
  ],
};

export const mockTableMetadata: TableMetadata = {
  columns: [
    {
      name: "id",
      type: "integer",
      nullable: false,
      primaryKey: true,
      comment: "User ID",
    },
    {
      name: "username",
      type: "varchar",
      nullable: false,
      primaryKey: false,
      comment: "Username",
    },
    {
      name: "email",
      type: "varchar",
      nullable: false,
      primaryKey: false,
      comment: "Email address",
    },
    {
      name: "password_hash",
      type: "varchar",
      nullable: false,
      primaryKey: false,
      comment: "Password hash",
    },
    {
      name: "created_at",
      type: "timestamp",
      nullable: true,
      defaultValue: "CURRENT_TIMESTAMP",
      primaryKey: false,
      comment: "Created timestamp",
    },
    {
      name: "updated_at",
      type: "timestamp",
      nullable: true,
      defaultValue: "CURRENT_TIMESTAMP",
      primaryKey: false,
      comment: "Updated timestamp",
    },
  ],
  indexes: [
    {
      name: "users_pkey",
      unique: true,
      indexType: "btree",
      columns: ["id"],
    },
    {
      name: "users_email_idx",
      unique: false,
      indexType: "btree",
      columns: ["email"],
    },
    {
      name: "users_username_idx",
      unique: false,
      indexType: "btree",
      columns: ["username"],
    },
  ],
  foreignKeys: [
    {
      name: "fk_user_role",
      column: "role_id",
      referencedTable: "roles",
      referencedColumn: "id",
      onUpdate: "CASCADE",
      onDelete: "SET NULL",
    },
  ],
};

export const mockSchemaOverview: SchemaOverview = {
  tables: [
    {
      schema: "public",
      name: "users",
      columns: [
        { name: "id", type: "integer" },
        { name: "username", type: "varchar" },
        { name: "email", type: "varchar" },
        { name: "created_at", type: "timestamp" },
      ],
    },
    {
      schema: "public",
      name: "posts",
      columns: [
        { name: "id", type: "integer" },
        { name: "user_id", type: "integer" },
        { name: "title", type: "varchar" },
        { name: "content", type: "text" },
        { name: "created_at", type: "timestamp" },
      ],
    },
    {
      schema: "public",
      name: "comments",
      columns: [
        { name: "id", type: "integer" },
        { name: "post_id", type: "integer" },
        { name: "user_id", type: "integer" },
        { name: "content", type: "text" },
        { name: "created_at", type: "timestamp" },
      ],
    },
  ],
};

export const mockTableData = {
  data: [
    {
      id: 1,
      username: "alice",
      email: "alice@example.com",
      password_hash: "hashed_password_1",
      created_at: "2024-01-15 10:30:00",
      updated_at: "2024-01-15 10:30:00",
    },
    {
      id: 2,
      username: "bob",
      email: "bob@example.com",
      password_hash: "hashed_password_2",
      created_at: "2024-01-16 11:45:00",
      updated_at: "2024-01-16 11:45:00",
    },
    {
      id: 3,
      username: "charlie",
      email: "charlie@example.com",
      password_hash: "hashed_password_3",
      created_at: "2024-01-17 14:20:00",
      updated_at: "2024-01-17 14:20:00",
    },
    {
      id: 4,
      username: "diana",
      email: "diana@example.com",
      password_hash: "hashed_password_4",
      created_at: "2024-01-18 09:15:00",
      updated_at: "2024-01-18 09:15:00",
    },
    {
      id: 5,
      username: "eve",
      email: "eve@example.com",
      password_hash: "hashed_password_5",
      created_at: "2024-01-19 16:50:00",
      updated_at: "2024-01-19 16:50:00",
    },
  ],
  total: 5,
  page: 1,
  limit: 10,
  executionTimeMs: 25,
};

export const mockDatabases = [
  "postgres",
  "template1",
  "template0",
  "testdb",
  "myapp_dev",
];

export const mockQueryResult: QueryResult = {
  data: mockTableData.data,
  rowCount: 5,
  columns: [
    { name: "id", type: "integer" },
    { name: "username", type: "varchar" },
    { name: "email", type: "varchar" },
    { name: "password_hash", type: "varchar" },
    { name: "created_at", type: "timestamp" },
    { name: "updated_at", type: "timestamp" },
  ],
  timeTakenMs: 45,
  success: true,
};

const mockDDL = `CREATE TABLE public.users (
  id integer NOT NULL,
  username character varying(255) NOT NULL,
  email character varying(255) NOT NULL,
  password_hash character varying(255) NOT NULL,
  created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP,
  updated_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT users_pkey PRIMARY KEY (id)
);

CREATE INDEX users_email_idx ON public.users USING btree (email);
CREATE INDEX users_username_idx ON public.users USING btree (username);`;

// ==================== Mock 处理函数 ====================

/**
 * 模拟执行查询
 */
export async function mockExecuteQuery(
  id: number,
  query: string,
  database?: string
): Promise<QueryResult> {
  // 模拟网络延迟
  await new Promise((resolve) => setTimeout(resolve, 100));

  // 根据查询类型返回不同的数据
  if (query.toLowerCase().includes("select")) {
    return {
      ...mockQueryResult,
      timeTakenMs: Math.floor(Math.random() * 100) + 20,
    };
  }

  return {
    data: [],
    rowCount: 0,
    columns: [],
    timeTakenMs: Math.floor(Math.random() * 50) + 10,
    success: true,
  };
}

/**
 * 模拟取消查询
 */
export async function mockCancelQuery(
  uuid: string,
  queryId: string
): Promise<boolean> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return true;
}

/**
 * 模拟通过连接信息执行查询
 */
export async function mockExecuteByConn(
  form: ConnectionForm,
  sql: string
): Promise<QueryResult> {
  await new Promise((resolve) => setTimeout(resolve, 100));
  return mockQueryResult;
}

/**
 * 模拟列出表
 */
export async function mockListTables(
  id: number,
  database?: string,
  schema?: string
): Promise<{ schema: string; name: string; type: string }[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockTables;
}

/**
 * 模拟获取表结构
 */
export async function mockGetTableStructure(
  id: number,
  schema: string,
  table: string
): Promise<{ columns: { name: string; type: string; nullable: boolean }[] }> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockTableStructure;
}

/**
 * 模拟获取表 DDL
 */
export async function mockGetTableDDL(
  id: number,
  database: string | undefined,
  schema: string,
  table: string
): Promise<string> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockDDL;
}

/**
 * 模拟获取表元数据
 */
export async function mockGetTableMetadata(
  id: number,
  database: string | undefined,
  schema: string,
  table: string
): Promise<TableMetadata> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockTableMetadata;
}

/**
 * 模拟通过连接信息列出表
 */
export async function mockListTablesByConn(
  form: ConnectionForm
): Promise<{ schema: string; name: string; type: string }[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockTables;
}

/**
 * 模拟列出数据库
 */
export async function mockListDatabases(
  form: ConnectionForm
): Promise<string[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockDatabases;
}

/**
 * 模拟通过 ID 列出数据库
 */
export async function mockListDatabasesById(id: number): Promise<string[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockDatabases;
}

/**
 * 模拟获取 schema 概览
 */
export async function mockGetSchemaOverview(
  id: number,
  database?: string,
  schema?: string
): Promise<SchemaOverview> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockSchemaOverview;
}

/**
 * 模拟获取表数据
 */
export async function mockGetTableData(params: {
  id: number;
  schema: string;
  table: string;
  page: number;
  limit: number;
  filter?: string;
  sortColumn?: string;
  sortDirection?: "asc" | "desc";
  orderBy?: string;
}): Promise<{
  data: any[];
  total: number;
  page: number;
  limit: number;
  executionTimeMs: number;
}> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  const { page = 1, limit = 10 } = params;
  const start = (page - 1) * limit;
  const end = start + limit;

  return {
    data: mockTableData.data.slice(start, end),
    total: mockTableData.total,
    page,
    limit,
    executionTimeMs: Math.floor(Math.random() * 50) + 20,
  };
}

/**
 * 模拟通过连接信息获取表数据
 */
export async function mockGetTableDataByConn(
  form: ConnectionForm,
  schema: string,
  table: string,
  page: number,
  limit: number
): Promise<{
  data: any[];
  total: number;
  page: number;
  limit: number;
  executionTimeMs: number;
}> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  const start = (page - 1) * limit;
  const end = start + limit;

  return {
    data: mockTableData.data.slice(start, end),
    total: mockTableData.total,
    page,
    limit,
    executionTimeMs: Math.floor(Math.random() * 50) + 20,
  };
}

/**
 * 模拟获取连接列表
 */
export async function mockGetConnections(): Promise<any[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockConnections;
}

/**
 * 模拟创建连接
 */
export async function mockCreateConnection(form: ConnectionForm): Promise<any> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  return {
    id: mockConnections.length + 1,
    name: form.name || "New Connection",
    driver: form.driver,
    host: form.host,
    port: form.port,
    database: form.database,
    username: form.username,
    createdAt: new Date().toISOString(),
  };
}

/**
 * 模拟测试连接
 */
export async function mockTestConnectionEphemeral(
  form: ConnectionForm
): Promise<TestConnectionResult> {
  await new Promise((resolve) => setTimeout(resolve, 200));

  return {
    success: true,
    message: "Connection test successful",
    latencyMs: Math.floor(Math.random() * 100) + 50,
  };
}

/**
 * 根据命令名调用对应的 Mock 处理函数
 */
export async function invokeMock<T>(cmd: string, args?: any): Promise<T> {
  console.log(`[Mock] ${cmd}`, args);

  switch (cmd) {
    // Query commands
    case "execute_query":
      return mockExecuteQuery(args.id, args.query, args.database) as Promise<T>;

    case "cancel_query":
      return mockCancelQuery(args.uuid, args.queryId) as Promise<T>;

    case "execute_by_conn":
      return mockExecuteByConn(args.form, args.sql) as Promise<T>;

    // Metadata commands
    case "list_tables":
      return mockListTables(
        args.id,
        args.database,
        args.schema
      ) as Promise<T>;

    case "get_table_structure":
      return mockGetTableStructure(
        args.id,
        args.schema,
        args.table
      ) as Promise<T>;

    case "get_table_ddl":
      return mockGetTableDDL(
        args.id,
        args.database,
        args.schema,
        args.table
      ) as Promise<T>;

    case "get_table_metadata":
      return mockGetTableMetadata(
        args.id,
        args.database,
        args.schema,
        args.table
      ) as Promise<T>;

    case "list_tables_by_conn":
      return mockListTablesByConn(args.form) as Promise<T>;

    case "list_databases":
      return mockListDatabases(args.form) as Promise<T>;

    case "list_databases_by_id":
      return mockListDatabasesById(args.id) as Promise<T>;

    case "get_schema_overview":
      return mockGetSchemaOverview(
        args.id,
        args.database,
        args.schema
      ) as Promise<T>;

    // Table data commands
    case "get_table_data":
      return mockGetTableData(args) as Promise<T>;

    case "get_table_data_by_conn":
      return mockGetTableDataByConn(
        args.form,
        args.schema,
        args.table,
        args.page,
        args.limit
      ) as Promise<T>;

    // Connection commands
    case "get_connections":
      return mockGetConnections() as Promise<T>;

    case "create_connection":
      return mockCreateConnection(args.form) as Promise<T>;

    case "test_connection_ephemeral":
      return mockTestConnectionEphemeral(args.form) as Promise<T>;

    default:
      console.warn(`[Mock] Unknown command: ${cmd}`);
      throw new Error(`Mock: Unknown command '${cmd}'`);
  }
}
