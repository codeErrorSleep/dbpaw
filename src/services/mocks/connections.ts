import { ConnectionForm, TestConnectionResult } from "../types";
import { COMMANDS } from "../commands";
import type { CommandMap, CommandArgs, CommandReturn } from "../commands/types";

type ConnectionCommand = Extract<
  keyof CommandMap,
  | "get_connections"
  | "create_connection"
  | "update_connection"
  | "delete_connection"
  | "create_database_by_id"
  | "get_mysql_charsets_by_id"
  | "get_mysql_collations_by_id"
  | "test_connection_ephemeral"
  | "test_connection_saved_edit"
  | "list_sqlite_issues"
  | "import_connections"
>;

export const mockConnections: any[] = [
  {
    id: 1,
    uuid: "mock-1",
    name: "PostgreSQL Dev",
    dbType: "postgres",
    host: "localhost",
    port: 5432,
    database: "testdb",
    username: "postgres",
    ssl: false,
    sshEnabled: false,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 2,
    uuid: "mock-2",
    name: "SQLite Local",
    dbType: "sqlite",
    host: "",
    port: 0,
    database: "",
    username: "",
    ssl: false,
    filePath: "/path/to/database.db",
    sshEnabled: false,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 3,
    uuid: "mock-3",
    name: "PostgreSQL JSONB Test",
    dbType: "postgres",
    host: "localhost",
    port: 5432,
    database: "jsondb",
    username: "postgres",
    ssl: false,
    sshEnabled: false,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 4,
    uuid: "mock-4",
    name: "Redis Dev",
    dbType: "redis",
    host: "localhost",
    port: 6379,
    database: "0",
    username: "",
    ssl: false,
    sshEnabled: false,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 5,
    uuid: "mock-5",
    name: "MongoDB Dev",
    dbType: "mongodb",
    host: "localhost",
    port: 27017,
    database: "testdb",
    username: "",
    ssl: false,
    sshEnabled: false,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 6,
    uuid: "mock-6",
    name: "Elasticsearch Dev",
    dbType: "elasticsearch",
    host: "localhost",
    port: 9200,
    database: "",
    username: "",
    ssl: false,
    sshEnabled: false,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
  {
    id: 7,
    uuid: "mock-7",
    name: "MySQL Dev",
    dbType: "mysql",
    host: "localhost",
    port: 3306,
    database: "testdb",
    username: "root",
    ssl: false,
    sshEnabled: false,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
];

export async function mockGetMysqlCharsets(_id: number): Promise<string[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return [
    "armscii8",
    "ascii",
    "big5",
    "binary",
    "cp1250",
    "cp1251",
    "cp1256",
    "cp1257",
    "cp850",
    "cp852",
    "cp866",
    "cp932",
    "dec8",
    "eucjpms",
    "euckr",
    "gb18030",
    "gb2312",
    "gbk",
    "geostd8",
    "greek",
    "hebrew",
    "hp8",
    "keybcs2",
    "koi8r",
    "koi8u",
    "latin1",
    "latin2",
    "latin5",
    "latin7",
    "macce",
    "macroman",
    "sjis",
    "swe7",
    "tis620",
    "ucs2",
    "ujis",
    "utf16",
    "utf16le",
    "utf32",
    "utf8",
    "utf8mb4",
  ];
}

export async function mockGetMysqlCollations(
  _id: number,
  charset?: string,
): Promise<string[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  const all: Record<string, string[]> = {
    utf8mb4: [
      "utf8mb4_0900_ai_ci",
      "utf8mb4_0900_as_ci",
      "utf8mb4_0900_as_cs",
      "utf8mb4_bin",
      "utf8mb4_general_ci",
      "utf8mb4_unicode_ci",
      "utf8mb4_unicode_520_ci",
    ],
    utf8: ["utf8_bin", "utf8_general_ci", "utf8_unicode_ci"],
    latin1: ["latin1_bin", "latin1_general_ci", "latin1_swedish_ci"],
    ascii: ["ascii_bin", "ascii_general_ci"],
    binary: ["binary"],
  };
  if (charset && all[charset]) return all[charset];
  return Object.values(all).flat().sort();
}

export async function mockGetConnections(): Promise<any[]> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  return mockConnections;
}

export async function mockCreateConnection(form: ConnectionForm): Promise<any> {
  await new Promise((resolve) => setTimeout(resolve, 100));

  const newConnection = {
    id: mockConnections.length + 1,
    uuid: `mock-${mockConnections.length + 1}`,
    name: form.name || "New Connection",
    dbType: form.driver,
    host: form.host ?? "",
    port: form.port ?? 0,
    database: form.database ?? "",
    username: form.username ?? "",
    ssl: form.ssl ?? false,
    filePath: form.filePath ?? null,
    sshEnabled: form.sshEnabled ?? false,
    sshHost: form.sshHost ?? null,
    sshPort: form.sshPort ?? null,
    sshUsername: form.sshUsername ?? null,
    sshPassword: form.sshPassword ?? null,
    sshKeyPath: form.sshKeyPath ?? null,
    proxy: form.proxy ? { ...form.proxy, password: "" } : null,
    mode: form.mode ?? null,
    seedNodes: form.seedNodes ?? null,
    sentinels: form.sentinels ?? null,
    connectTimeoutMs: form.connectTimeoutMs ?? null,
    serviceName: form.serviceName ?? null,
    sentinelPassword: form.sentinelPassword ?? null,
    authMode: form.authMode ?? null,
    apiKeyId: form.apiKeyId ?? null,
    apiKeySecret: form.apiKeySecret ?? null,
    apiKeyEncoded: form.apiKeyEncoded ?? null,
    cloudId: form.cloudId ?? null,
    authSource: form.authSource ?? null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };

  mockConnections.push(newConnection);
  return newConnection;
}

export async function mockUpdateConnection(
  id: number,
  form: ConnectionForm,
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
  const nextApiKeySecret =
    form.apiKeySecret !== undefined && form.apiKeySecret !== ""
      ? form.apiKeySecret
      : existing.apiKeySecret;
  const nextApiKeyEncoded =
    form.apiKeyEncoded !== undefined && form.apiKeyEncoded !== ""
      ? form.apiKeyEncoded
      : existing.apiKeyEncoded;

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
    proxy: form.proxy ? { ...form.proxy, password: "" } : null,
    mode: form.mode ?? existing.mode ?? null,
    seedNodes: form.seedNodes ?? existing.seedNodes ?? null,
    sentinels: form.sentinels ?? existing.sentinels ?? null,
    connectTimeoutMs:
      form.connectTimeoutMs ?? existing.connectTimeoutMs ?? null,
    serviceName: form.serviceName ?? existing.serviceName ?? null,
    sentinelPassword:
      form.sentinelPassword !== undefined && form.sentinelPassword !== ""
        ? form.sentinelPassword
        : (existing.sentinelPassword ?? null),
    authMode: form.authMode ?? existing.authMode ?? null,
    apiKeyId: form.apiKeyId ?? existing.apiKeyId ?? null,
    apiKeySecret: nextApiKeySecret,
    apiKeyEncoded: nextApiKeyEncoded,
    cloudId: form.cloudId ?? existing.cloudId ?? null,
    authSource: form.authSource ?? existing.authSource ?? null,
    updatedAt: new Date().toISOString(),
  };

  mockConnections[index] = updatedConnection;
  return updatedConnection;
}

export async function mockDeleteConnection(id: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 50));
  const index = mockConnections.findIndex((c) => c.id === id);
  if (index === -1) {
    throw new Error(`Connection with id ${id} not found`);
  }
  mockConnections.splice(index, 1);
}

export async function mockCreateDatabaseById(
  _id: number,
  _payload: { name: string },
): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 80));
}

export async function mockTestConnectionEphemeral(
  _form: ConnectionForm,
): Promise<TestConnectionResult> {
  await new Promise((resolve) => setTimeout(resolve, 200));

  return {
    success: true,
    message: "Connection test successful",
    latencyMs: Math.floor(Math.random() * 100) + 50,
  };
}

export async function mockTestConnectionSavedEdit(
  id: number,
  form: ConnectionForm,
): Promise<TestConnectionResult> {
  const existing = mockConnections.find((connection) => connection.id === id);
  if (!existing) {
    throw new Error(`Connection with id ${id} not found`);
  }
  return mockTestConnectionEphemeral({
    ...form,
    password:
      form.password !== undefined && form.password !== ""
        ? form.password
        : existing.password,
    sshPassword:
      form.sshPassword !== undefined && form.sshPassword !== ""
        ? form.sshPassword
        : existing.sshPassword,
    sentinelPassword:
      form.sentinelPassword !== undefined && form.sentinelPassword !== ""
        ? form.sentinelPassword
        : existing.sentinelPassword,
    apiKeySecret:
      form.apiKeySecret !== undefined && form.apiKeySecret !== ""
        ? form.apiKeySecret
        : existing.apiKeySecret,
    apiKeyEncoded:
      form.apiKeyEncoded !== undefined && form.apiKeyEncoded !== ""
        ? form.apiKeyEncoded
        : existing.apiKeyEncoded,
  });
}

export function handleConnections<T extends ConnectionCommand>(
  cmd: T,
  args: CommandArgs<T>,
): Promise<CommandReturn<T>> | null {
  switch (cmd) {
    case COMMANDS.GET_CONNECTIONS:
      return mockGetConnections() as Promise<CommandReturn<T>>;
    case COMMANDS.CREATE_CONNECTION:
      return mockCreateConnection(
        (args as CommandArgs<"create_connection">).form,
      ) as Promise<CommandReturn<T>>;
    case COMMANDS.UPDATE_CONNECTION:
      return mockUpdateConnection(
        (args as CommandArgs<"update_connection">).id,
        (args as CommandArgs<"update_connection">).form,
      ) as Promise<CommandReturn<T>>;
    case COMMANDS.DELETE_CONNECTION:
      return mockDeleteConnection(
        (args as CommandArgs<"delete_connection">).id,
      ) as Promise<CommandReturn<T>>;
    case COMMANDS.IMPORT_CONNECTIONS:
      return Promise.resolve({ imported: [], skipped: 0 }) as Promise<
        CommandReturn<T>
      >;
    case COMMANDS.CREATE_DATABASE_BY_ID:
      return mockCreateDatabaseById(
        (args as CommandArgs<"create_database_by_id">).id,
        (args as CommandArgs<"create_database_by_id">).payload,
      ) as Promise<CommandReturn<T>>;
    case COMMANDS.GET_MYSQL_CHARSETS_BY_ID:
      return mockGetMysqlCharsets(
        (args as CommandArgs<"get_mysql_charsets_by_id">).id,
      ) as Promise<CommandReturn<T>>;
    case COMMANDS.GET_MYSQL_COLLATIONS_BY_ID:
      return mockGetMysqlCollations(
        (args as CommandArgs<"get_mysql_collations_by_id">).id,
        (args as CommandArgs<"get_mysql_collations_by_id">).charset,
      ) as Promise<CommandReturn<T>>;
    case COMMANDS.TEST_CONNECTION_EPHEMERAL:
      return mockTestConnectionEphemeral(
        (args as CommandArgs<"test_connection_ephemeral">).form,
      ) as Promise<CommandReturn<T>>;
    case COMMANDS.TEST_CONNECTION_SAVED_EDIT:
      return mockTestConnectionSavedEdit(
        (args as CommandArgs<"test_connection_saved_edit">).id,
        (args as CommandArgs<"test_connection_saved_edit">).form,
      ) as Promise<CommandReturn<T>>;
    case COMMANDS.LIST_SQLITE_ISSUES:
      return Promise.resolve([]) as Promise<CommandReturn<T>>;
    default:
      return null;
  }
}
