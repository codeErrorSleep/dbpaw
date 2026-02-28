import {
  QueryResult,
  TableMetadata,
  SchemaOverview,
  ConnectionForm,
  TestConnectionResult,
  SavedQuery,
  ExportResult,
  AIProviderConfig,
  AIConversation,
  AIConversationDetail,
} from "./api";

/**
 * Mock data layer - provides mock implementation for all API commands
 * Used for frontend standalone development and debugging in non-Tauri environments
 */

// ==================== Mock Data ====================

export const mockConnections: any[] = [
  {
    id: 1,
    name: "PostgreSQL Dev",
    dbType: "postgres",
    host: "localhost",
    port: 5432,
    database: "testdb",
    username: "postgres",
    createdAt: new Date().toISOString(),
  },
  {
    id: 2,
    name: "SQLite Local",
    dbType: "sqlite",
    filePath: "/path/to/database.db",
    createdAt: new Date().toISOString(),
  },
];

export const mockTables: { schema: string; name: string; type: string }[] = [
  { schema: "public", name: "users", type: "table" },
  { schema: "public", name: "posts", type: "table" },
  { schema: "public", name: "comments", type: "table" },
  { schema: "public", name: "tags", type: "table" },
  { schema: "public", name: "orders", type: "table" },
  { schema: "public", name: "order_items", type: "table" },
  { schema: "public", name: "products", type: "table" },
  { schema: "public", name: "product_categories", type: "table" },
  { schema: "public", name: "categories", type: "table" },
  { schema: "public", name: "payments", type: "table" },
  { schema: "public", name: "refunds", type: "table" },
  { schema: "public", name: "invoices", type: "table" },
  { schema: "public", name: "addresses", type: "table" },
  { schema: "public", name: "audit_logs", type: "table" },
  { schema: "public", name: "sessions", type: "table" },
  { schema: "public", name: "roles", type: "table" },
  { schema: "public", name: "user_roles", type: "table" },
  { schema: "analytics", name: "page_views", type: "table" },
  { schema: "analytics", name: "events", type: "table" },
  { schema: "analytics", name: "funnels", type: "table" },
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

export const mockSavedQueries: SavedQuery[] = [
  {
    id: 1,
    name: "Get all users",
    query: "SELECT * FROM users",
    description: "Fetch all users from the database",
    connectionId: 1,
    database: "testdb",
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 2,
    name: "Active posts",
    query: "SELECT * FROM posts WHERE status = 'active'",
    description: null,
    connectionId: 1,
    database: "testdb",
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
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

const mockAiProviders: AIProviderConfig[] = [
  {
    id: 1,
    name: "OpenAI",
    providerType: "openai",
    baseUrl: "https://api.openai.com/v1",
    model: "gpt-4.1-mini",
    apiKey: "sk-mock",
    isDefault: false,
    enabled: true,
    extraJson: null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 2,
    name: "OpenAI Compat",
    providerType: "openai_compat",
    baseUrl: "http://localhost:11434/v1",
    model: "qwen2.5-coder:14b",
    apiKey: "sk-mock",
    isDefault: true,
    enabled: true,
    extraJson: JSON.stringify({ note: "mock provider" }),
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 3,
    name: "Disabled Provider",
    providerType: "openai",
    baseUrl: "https://example.invalid/v1",
    model: "gpt-4.1",
    apiKey: "sk-mock",
    isDefault: false,
    enabled: false,
    extraJson: null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
];

const mockAiConversations: AIConversation[] = [
  {
    id: 1,
    title: "生成：订单列表 SQL",
    scenario: "sql_generate",
    connectionId: 1,
    database: "testdb",
    createdAt: new Date(Date.now() - 86400000).toISOString(),
    updatedAt: new Date(Date.now() - 86400000).toISOString(),
  },
  {
    id: 2,
    title: "优化：慢查询日志",
    scenario: "sql_optimize",
    connectionId: 1,
    database: "testdb",
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 3,
    title: "解释：JOIN 语句",
    scenario: "sql_explain",
    connectionId: 1,
    database: "testdb",
    createdAt: new Date(Date.now() - 3 * 3600 * 1000).toISOString(),
    updatedAt: new Date(Date.now() - 3 * 3600 * 1000).toISOString(),
  },
];

const mockAiMessages: Record<number, AIConversationDetail["messages"]> = {
  1: [
    {
      id: 1,
      conversationId: 1,
      role: "user",
      content: "列出最近 7 天每个用户的订单数量与总金额，按金额倒序。",
      createdAt: new Date(Date.now() - 86400000).toISOString(),
    },
    {
      id: 2,
      conversationId: 1,
      role: "assistant",
      content:
        "SELECT u.id,\n       u.username,\n       COUNT(o.id) AS order_count,\n       COALESCE(SUM(o.total_amount), 0) AS total_amount\nFROM public.users u\nLEFT JOIN public.orders o\n  ON o.user_id = u.id\n AND o.created_at >= NOW() - INTERVAL '7 days'\nGROUP BY u.id, u.username\nORDER BY total_amount DESC;",
      model: "mock-model",
      createdAt: new Date(Date.now() - 86400000 + 1000).toISOString(),
    },
  ],
  2: [
    {
      id: 3,
      conversationId: 2,
      role: "user",
      content:
        "优化这个查询：SELECT * FROM audit_logs WHERE created_at > NOW() - INTERVAL '30 days' AND action = 'login'",
      createdAt: new Date().toISOString(),
    },
    {
      id: 4,
      conversationId: 2,
      role: "assistant",
      content:
        "SELECT id, user_id, action, created_at, ip\nFROM public.audit_logs\nWHERE action = 'login'\n  AND created_at > NOW() - INTERVAL '30 days'\nORDER BY created_at DESC;",
      model: "mock-model",
      createdAt: new Date(Date.now() + 2000).toISOString(),
    },
  ],
  3: [
    {
      id: 5,
      conversationId: 3,
      role: "user",
      content:
        "解释这条 SQL 在做什么：SELECT p.id, p.title FROM posts p JOIN users u ON u.id = p.user_id WHERE u.email LIKE '%@example.com' ORDER BY p.id DESC LIMIT 20",
      createdAt: new Date(Date.now() - 3 * 3600 * 1000).toISOString(),
    },
    {
      id: 6,
      conversationId: 3,
      role: "assistant",
      content:
        "这条 SQL 的意图是：\n1) 从 posts 表取文章（p.id, p.title）。\n2) 通过 p.user_id = u.id 关联 users 表，筛选作者邮箱以 @example.com 结尾（LIKE '%@example.com'）。\n3) 结果按文章 id 倒序排列，取最近 20 条。\n\n如果 posts 很大，建议确保 posts(user_id) 有索引，users(email) 也有索引（或使用更合适的模式匹配策略）。",
      model: "mock-model",
      createdAt: new Date(Date.now() - 3 * 3600 * 1000 + 1000).toISOString(),
    },
  ],
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

// ==================== Mock Handler Functions ====================

/**
 * Mock query execution
 */
export async function mockExecuteQuery(
  _id: number,
  query: string,
  _database?: string
): Promise<QueryResult> {
  // Simulate network latency
  await new Promise((resolve) => setTimeout(resolve, 100));

  // Return different data based on query type
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
 * Mock query cancellation
 */
export async function mockCancelQuery(
  _uuid: string,
  _queryId: string
): Promise<boolean> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return true;
}

/**
 * Mock query execution by connection info
 */
export async function mockExecuteByConn(
  _form: ConnectionForm,
  _sql: string
): Promise<QueryResult> {
  await new Promise((resolve) => setTimeout(resolve, 100));
  return mockQueryResult;
}

/**
 * Mock list tables
 */
export async function mockListTables(
  _id: number,
  _database?: string,
  _schema?: string
): Promise<{ schema: string; name: string; type: string }[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockTables;
}

/**
 * Mock get table structure
 */
export async function mockGetTableStructure(
  _id: number,
  _schema: string,
  _table: string
): Promise<{ columns: { name: string; type: string; nullable: boolean }[] }> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockTableStructure;
}

/**
 * Mock get table DDL
 */
export async function mockGetTableDDL(
  _id: number,
  _database: string | undefined,
  _schema: string,
  _table: string
): Promise<string> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockDDL;
}

/**
 * Mock get table metadata
 */
export async function mockGetTableMetadata(
  _id: number,
  _database: string | undefined,
  _schema: string,
  _table: string
): Promise<TableMetadata> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockTableMetadata;
}

/**
 * Mock list tables by connection info
 */
export async function mockListTablesByConn(
  _form: ConnectionForm
): Promise<{ schema: string; name: string; type: string }[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockTables;
}

/**
 * Mock list databases
 */
export async function mockListDatabases(
  _form: ConnectionForm
): Promise<string[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockDatabases;
}

/**
 * Mock list databases by ID
 */
export async function mockListDatabasesById(_id: number): Promise<string[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockDatabases;
}

/**
 * Mock get schema overview
 */
export async function mockGetSchemaOverview(
  _id: number,
  _database?: string,
  _schema?: string
): Promise<SchemaOverview> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockSchemaOverview;
}

/**
 * Mock get table data
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
 * Mock get table data by connection info
 */
export async function mockGetTableDataByConn(
  _form: ConnectionForm,
  _schema: string,
  _table: string,
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
 * Mock get connections list
 */
export async function mockGetConnections(): Promise<any[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockConnections;
}

/**
 * Mock create connection
 */
export async function mockCreateConnection(form: ConnectionForm): Promise<any> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  const newConnection = {
    id: mockConnections.length + 1,
    name: form.name || "New Connection",
    dbType: form.driver,
    host: form.host,
    port: form.port,
    database: form.database,
    username: form.username,
    ssl: form.ssl ?? false,
    filePath: form.filePath ?? null,
    sshEnabled: form.sshEnabled ?? false,
    sshHost: form.sshHost ?? null,
    sshPort: form.sshPort ?? null,
    sshUsername: form.sshUsername ?? null,
    sshPassword: form.sshPassword ?? null,
    sshKeyPath: form.sshKeyPath ?? null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };

  mockConnections.push(newConnection);
  return newConnection;
}

/**
 * Mock update connection
 */
export async function mockUpdateConnection(
  id: number,
  form: ConnectionForm
): Promise<any> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  const index = mockConnections.findIndex((c) => c.id === id);
  if (index === -1) {
    throw new Error(`Connection with id ${id} not found`);
  }

  const existing = mockConnections[index];
  const nextPassword =
    form.password !== undefined && form.password !== ""
      ? form.password
      : existing.password;

  const updatedConnection = {
    ...existing,
    name: form.name || existing.name,
    dbType: form.driver || existing.dbType,
    host: form.host ?? existing.host,
    port: form.port ?? existing.port,
    database: form.database ?? existing.database,
    username: form.username ?? existing.username,
    password: nextPassword,
    ssl: form.ssl ?? existing.ssl ?? false,
    filePath: form.filePath ?? existing.filePath ?? null,
    sshEnabled: form.sshEnabled ?? existing.sshEnabled ?? false,
    sshHost: form.sshHost ?? existing.sshHost ?? null,
    sshPort: form.sshPort ?? existing.sshPort ?? null,
    sshUsername: form.sshUsername ?? existing.sshUsername ?? null,
    sshPassword: form.sshPassword ?? existing.sshPassword ?? null,
    sshKeyPath: form.sshKeyPath ?? existing.sshKeyPath ?? null,
    updatedAt: new Date().toISOString(),
  };

  mockConnections[index] = updatedConnection;
  return updatedConnection;
}

/**
 * Mock delete connection
 */
export async function mockDeleteConnection(id: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  const index = mockConnections.findIndex((c) => c.id === id);
  if (index === -1) {
    throw new Error(`Connection with id ${id} not found`);
  }
  mockConnections.splice(index, 1);
}

/**
 * Mock test connection
 */
export async function mockTestConnectionEphemeral(
  _form: ConnectionForm
): Promise<TestConnectionResult> {
  await new Promise((resolve) => setTimeout(resolve, 200));

  return {
    success: true,
    message: "Connection test successful",
    latencyMs: Math.floor(Math.random() * 100) + 50,
  };
}

/**
 * Mock get saved queries
 */
export async function mockGetSavedQueries(): Promise<SavedQuery[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return [...mockSavedQueries];
}

/**
 * Mock save query
 */
export async function mockSaveQuery(data: {
  name: string;
  query: string;
  description?: string;
  connectionId?: number;
  database?: string;
}): Promise<SavedQuery> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  const newQuery: SavedQuery = {
    id:
      mockSavedQueries.length > 0
        ? Math.max(...mockSavedQueries.map((q) => q.id)) + 1
        : 1,
    name: data.name,
    query: data.query,
    description: data.description || null,
    connectionId: data.connectionId || null,
    database: data.database || null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };

  mockSavedQueries.push(newQuery);
  return newQuery;
}

/**
 * Mock update saved query
 */
export async function mockUpdateSavedQuery(
  id: number,
  data: {
    name: string;
    query: string;
    description?: string;
    connectionId?: number;
    database?: string;
  }
): Promise<SavedQuery> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  const index = mockSavedQueries.findIndex((q) => q.id === id);
  if (index === -1) {
    throw new Error(`Saved query with id ${id} not found`);
  }

  const updatedQuery = {
    ...mockSavedQueries[index],
    ...data,
    updatedAt: new Date().toISOString(),
  };

  mockSavedQueries[index] = updatedQuery;
  return updatedQuery;
}

/**
 * Mock delete saved query
 */
export async function mockDeleteSavedQuery(id: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  const index = mockSavedQueries.findIndex((q) => q.id === id);
  if (index !== -1) {
    mockSavedQueries.splice(index, 1);
  }
}

/**
 * Mock export table data
 */
export async function mockExportTableData(_params: any): Promise<ExportResult> {
  await new Promise((resolve) => setTimeout(resolve, 120));
  return {
    filePath: `/tmp/dbpaw-table-export-${Date.now()}.csv`,
    rowCount: mockTableData.total,
  };
}

/**
 * Mock export query result
 */
export async function mockExportQueryResult(_params: any): Promise<ExportResult> {
  await new Promise((resolve) => setTimeout(resolve, 120));
  return {
    filePath: `/tmp/dbpaw-query-export-${Date.now()}.csv`,
    rowCount: mockQueryResult.rowCount,
  };
}

/**
 * Invoke corresponding mock handler function by command name
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

    case "update_connection":
      return mockUpdateConnection(args.id, args.form) as Promise<T>;

    case "delete_connection":
      return mockDeleteConnection(args.id) as Promise<T>;

    case "test_connection_ephemeral":
      return mockTestConnectionEphemeral(args.form) as Promise<T>;

    // Saved Queries commands
    case "get_saved_queries":
      return mockGetSavedQueries() as Promise<T>;

    case "save_query":
      return mockSaveQuery(args) as Promise<T>;

    case "update_saved_query":
      return mockUpdateSavedQuery(args.id, args) as Promise<T>;

    case "delete_saved_query":
      return mockDeleteSavedQuery(args.id) as Promise<T>;

    // Transfer commands
    case "export_table_data":
      return mockExportTableData(args) as Promise<T>;

    case "export_query_result":
      return mockExportQueryResult(args) as Promise<T>;

    case "ai_list_providers":
      return Promise.resolve([...mockAiProviders]) as Promise<T>;

    case "ai_create_provider": {
      const requestedType = String(args.config.providerType || "openai");

      const now = new Date().toISOString();
      const isDefault = args.config.isDefault ?? true;
      if (isDefault) {
        mockAiProviders.forEach((p) => (p.isDefault = false));
      }

      const idx = mockAiProviders.findIndex((p) => p.providerType === requestedType);
      if (idx >= 0) {
        mockAiProviders[idx] = {
          ...mockAiProviders[idx],
          ...args.config,
          providerType: requestedType,
          enabled: args.config.enabled ?? true,
          isDefault,
          updatedAt: now,
        };
        return Promise.resolve(mockAiProviders[idx]) as Promise<T>;
      }

      const id = mockAiProviders.length
        ? Math.max(...mockAiProviders.map((p) => p.id)) + 1
        : 1;
      const created: AIProviderConfig = {
        id,
        providerType: requestedType,
        isDefault,
        enabled: args.config.enabled ?? true,
        extraJson: args.config.extraJson ?? null,
        createdAt: now,
        updatedAt: now,
        ...args.config,
      };
      mockAiProviders.push(created);
      return Promise.resolve(created) as Promise<T>;
    }

    case "ai_update_provider": {
      const idx = mockAiProviders.findIndex((p) => p.id === args.id);
      if (idx < 0) throw new Error("Provider not found");
      const requestedType = String(
        args.config.providerType || mockAiProviders[idx].providerType
      );
      const conflict = mockAiProviders.find(
        (p) => p.providerType === requestedType && p.id !== args.id
      );
      if (conflict) {
        throw new Error("UNIQUE constraint failed: ai_providers.provider_type");
      }
      if (args.config.isDefault) {
        mockAiProviders.forEach((p) => (p.isDefault = false));
      }
      mockAiProviders[idx] = {
        ...mockAiProviders[idx],
        ...args.config,
        providerType: requestedType,
        updatedAt: new Date().toISOString(),
      };
      return Promise.resolve(mockAiProviders[idx]) as Promise<T>;
    }

    case "ai_delete_provider": {
      const idx = mockAiProviders.findIndex((p) => p.id === args.id);
      if (idx >= 0) mockAiProviders.splice(idx, 1);
      return Promise.resolve(undefined) as Promise<T>;
    }

    case "ai_set_default_provider": {
      mockAiProviders.forEach((p) => (p.isDefault = p.id === args.id));
      return Promise.resolve(undefined) as Promise<T>;
    }

    case "ai_list_conversations":
      return Promise.resolve([...mockAiConversations]) as Promise<T>;

    case "ai_get_conversation": {
      const c = mockAiConversations.find((x) => x.id === args.conversationId);
      if (!c) throw new Error("Conversation not found");
      return Promise.resolve({
        conversation: c,
        messages: mockAiMessages[c.id] || [],
      }) as Promise<T>;
    }

    case "ai_delete_conversation": {
      const idx = mockAiConversations.findIndex((x) => x.id === args.conversationId);
      if (idx >= 0) mockAiConversations.splice(idx, 1);
      delete mockAiMessages[args.conversationId];
      return Promise.resolve(undefined) as Promise<T>;
    }

    case "ai_chat_start":
    case "ai_chat_continue": {
      const input = args.request.input as string;
      const selectedTables = (args.request.selectedTables as Array<{ schema: string; name: string }> | undefined) || [];
      let conversationId = args.request.conversationId as number | undefined;
      if (!conversationId) {
        conversationId = mockAiConversations.length
          ? Math.max(...mockAiConversations.map((x) => x.id)) + 1
          : 1;
        mockAiConversations.unshift({
          id: conversationId,
          title: args.request.title || input.slice(0, 20),
          scenario: args.request.scenario || "sql_generate",
          connectionId: args.request.connectionId || null,
          database: args.request.database || null,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        });
      }
      const msgs = mockAiMessages[conversationId] || [];
      const now = new Date().toISOString();
      msgs.push({
        id: msgs.length + 1,
        conversationId,
        role: "user",
        content: input,
        createdAt: now,
      } as any);
      msgs.push({
        id: msgs.length + 1,
        conversationId,
        role: "assistant",
        content: (() => {
          const scenario = String(args.request.scenario || "sql_generate");
          const first = selectedTables[0];
          const from = first ? `${first.schema}.${first.name}` : "public.users";
          if (scenario === "sql_optimize") {
            return `SELECT *\nFROM ${from}\nWHERE 1=1\nLIMIT 100;`;
          }
          if (scenario === "sql_explain") {
            return `这是一条 mock 解释：SQL 主要从 ${from} 读取数据。`;
          }
          if (selectedTables.length > 0) {
            const names = selectedTables.map((t) => `${t.schema}.${t.name}`).join(", ");
            return `SELECT *\nFROM ${from}\n-- selected tables: ${names}\nLIMIT 50;`;
          }
          return `SELECT *\nFROM ${from}\nLIMIT 50;`;
        })(),
        model: "mock-model",
        createdAt: now,
      } as any);
      mockAiMessages[conversationId] = msgs;
      const idx = mockAiConversations.findIndex((x) => x.id === conversationId);
      if (idx >= 0) {
        mockAiConversations[idx] = { ...mockAiConversations[idx], updatedAt: now };
      }
      return Promise.resolve({
        conversationId,
        userMessageId: msgs[msgs.length - 2].id,
        assistantMessageId: msgs[msgs.length - 1].id,
      }) as Promise<T>;
    }

    default:
      console.warn(`[Mock] Unknown command: ${cmd}`);
      throw new Error(`Mock: Unknown command '${cmd}'`);
  }
}
