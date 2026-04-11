import { describe, expect, test, mock, beforeEach, afterEach } from "bun:test";

// Must be declared before importing the module under test so bun resolves mocks first
let tauriInvokeImpl: (cmd: string, args?: any) => Promise<any> = async () =>
  "tauri-result";
mock.module("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: any) => tauriInvokeImpl(cmd, args),
}));

let mockInvokeImpl: (cmd: string, args?: any) => Promise<any> = async () =>
  "mock-result";
mock.module("./mocks", () => ({
  invokeMock: (cmd: string, args?: any) => mockInvokeImpl(cmd, args),
}));

import {
  isTauri,
  normalizeImportDriver,
  getImportDriverCapability,
  api,
} from "./api";

// Bun test runs in a Node-like env without a DOM.
// isTauri() checks `typeof window !== "undefined"`, so we simulate it via globalThis.
const g = globalThis as any;

// ─── isTauri ─────────────────────────────────────────────────────────────────

describe("isTauri", () => {
  afterEach(() => {
    delete g.window;
  });

  test("returns false when window is undefined", () => {
    delete g.window;
    expect(isTauri()).toBe(false);
  });

  test("returns false when window exists but __TAURI_INTERNALS__ is absent", () => {
    g.window = {};
    expect(isTauri()).toBe(false);
  });

  test("returns true when __TAURI_INTERNALS__ is present on window", () => {
    g.window = { __TAURI_INTERNALS__: {} };
    expect(isTauri()).toBe(true);
  });
});

// ─── normalizeImportDriver ───────────────────────────────────────────────────

describe("normalizeImportDriver", () => {
  test("normalizes postgresql aliases to postgres", () => {
    expect(normalizeImportDriver("postgresql")).toBe("postgres");
    expect(normalizeImportDriver("pgsql")).toBe("postgres");
  });

  test("is case-insensitive and trims whitespace", () => {
    expect(normalizeImportDriver("PostgreSQL")).toBe("postgres");
    expect(normalizeImportDriver("  PGSQL  ")).toBe("postgres");
    expect(normalizeImportDriver("  MySQL  ")).toBe("mysql");
  });

  test("passes through known drivers unchanged", () => {
    const passThrough = [
      "postgres",
      "mysql",
      "mariadb",
      "tidb",
      "starrocks",
      "sqlite",
      "duckdb",
      "mssql",
      "clickhouse",
    ];
    for (const driver of passThrough) {
      expect(normalizeImportDriver(driver)).toBe(driver);
    }
  });

  test("returns empty string for empty / falsy input", () => {
    expect(normalizeImportDriver("")).toBe("");
    expect(normalizeImportDriver(null as any)).toBe("");
    expect(normalizeImportDriver(undefined as any)).toBe("");
  });
});

// ─── getImportDriverCapability ───────────────────────────────────────────────

describe("getImportDriverCapability", () => {
  test("clickhouse is read-only-not-supported", () => {
    expect(getImportDriverCapability("clickhouse")).toBe(
      "read_only_not_supported",
    );
  });

  test("starrocks import is unsupported", () => {
    expect(getImportDriverCapability("starrocks")).toBe("unsupported");
  });

  test("all writable drivers are supported", () => {
    const supported = [
      "postgres",
      "postgresql", // via normalizeImportDriver
      "pgsql",
      "mysql",
      "mariadb",
      "tidb",
      "sqlite",
      "duckdb",
      "mssql",
      "oracle",
    ];
    for (const driver of supported) {
      expect(getImportDriverCapability(driver)).toBe("supported");
    }
  });

  test("unknown drivers are unsupported", () => {
    expect(getImportDriverCapability("")).toBe("unsupported");
    expect(getImportDriverCapability("mongodb")).toBe("unsupported");
  });
});

// ─── invoke routing ──────────────────────────────────────────────────────────

describe("invoke: no Tauri + no mock mode", () => {
  beforeEach(() => {
    delete g.window;
    delete (import.meta.env as any).VITE_USE_MOCK;
  });

  test("throws a descriptive error", async () => {
    await expect(api.connections.list()).rejects.toThrow(
      "Tauri API not available",
    );
  });

  test("error message mentions bun tauri dev", async () => {
    let msg = "";
    try {
      await api.connections.list();
    } catch (e) {
      msg = (e as Error).message;
    }
    expect(msg).toContain("bun tauri dev");
  });
});

describe("invoke: Tauri environment", () => {
  beforeEach(() => {
    g.window = { __TAURI_INTERNALS__: {} };
  });

  afterEach(() => {
    delete g.window;
    tauriInvokeImpl = async () => "tauri-result";
  });

  test("delegates to tauriInvoke and returns its result", async () => {
    const expected = [{ id: 1 }];
    tauriInvokeImpl = async (cmd) => {
      expect(cmd).toBe("get_connections");
      return expected;
    };
    const result = await api.connections.list();
    expect(result).toBe(expected);
  });

  test("forwards command name correctly for query.execute", async () => {
    let capturedCmd = "";
    let capturedArgs: any = null;
    tauriInvokeImpl = async (cmd, args) => {
      capturedCmd = cmd;
      capturedArgs = args;
      return {
        data: [],
        rowCount: 0,
        columns: [],
        timeTakenMs: 0,
        success: true,
      };
    };

    await api.query.execute(42, "SELECT 1", "mydb", "sql_editor");

    expect(capturedCmd).toBe("execute_query");
    expect(capturedArgs).toMatchObject({
      id: 42,
      query: "SELECT 1",
      database: "mydb",
    });
  });

  test("tauriInvoke error propagates to caller", async () => {
    tauriInvokeImpl = async () => {
      throw new Error("connection refused");
    };
    await expect(api.connections.list()).rejects.toThrow("connection refused");
  });
});

describe("invoke: mock mode (VITE_USE_MOCK=true)", () => {
  beforeEach(() => {
    delete g.window;
    (import.meta.env as any).VITE_USE_MOCK = "true";
  });

  afterEach(() => {
    delete (import.meta.env as any).VITE_USE_MOCK;
    mockInvokeImpl = async () => "mock-result";
  });

  test("delegates to invokeMock and returns its result", async () => {
    const expected = [{ id: 99, name: "mock-conn" }];
    mockInvokeImpl = async (cmd) => {
      expect(cmd).toBe("get_connections");
      return expected;
    };
    const result = await api.connections.list();
    expect(result).toBe(expected);
  });

  test("invokeMock error propagates to caller", async () => {
    mockInvokeImpl = async () => {
      throw new Error("mock error");
    };
    await expect(api.connections.list()).rejects.toThrow("mock error");
  });

  test("Tauri path takes precedence over mock mode when both are active", async () => {
    g.window = { __TAURI_INTERNALS__: {} };
    let usedTauri = false;
    tauriInvokeImpl = async () => {
      usedTauri = true;
      return [];
    };
    mockInvokeImpl = async () => {
      throw new Error("should not be called");
    };

    await api.connections.list();
    expect(usedTauri).toBe(true);

    delete g.window;
  });
});

// ─── api command mapping spot-checks ─────────────────────────────────────────

describe("api command mapping", () => {
  beforeEach(() => {
    g.window = { __TAURI_INTERNALS__: {} };
  });

  afterEach(() => {
    delete g.window;
    tauriInvokeImpl = async () => undefined;
  });

  const commands: [string, () => Promise<any>][] = [
    ["list_sql_execution_logs", () => api.sqlLogs.list()],
    ["list_tables", () => api.metadata.listTables(1)],
    ["get_table_ddl", () => api.metadata.getTableDDL(1, "db", "public", "t")],
    [
      "get_table_metadata",
      () => api.metadata.getTableMetadata(1, "db", "public", "t"),
    ],
    ["get_connections", () => api.connections.list()],
    ["create_connection", () => api.connections.create({ driver: "postgres" })],
    ["delete_connection", () => api.connections.delete(1)],
    ["get_saved_queries", () => api.queries.list()],
    ["delete_saved_query", () => api.queries.delete(1)],
    ["ai_list_providers", () => api.ai.providers.list()],
    ["ai_delete_provider", () => api.ai.providers.delete(1)],
    ["ai_list_conversations", () => api.ai.conversations.list()],
    ["cancel_query", () => api.query.cancel("uuid-abc", "qid-1")],
    ["get_mysql_charsets_by_id", () => api.connections.getMysqlCharsets(1)],
    ["get_mysql_collations_by_id", () => api.connections.getMysqlCollations(1)],
    [
      "get_mysql_collations_by_id",
      () => api.connections.getMysqlCollations(1, "utf8mb4"),
    ],
  ];

  for (const [expectedCmd, callFn] of commands) {
    test(`api method maps to Tauri command "${expectedCmd}"`, async () => {
      let captured = "";
      tauriInvokeImpl = async (cmd) => {
        captured = cmd;
        return undefined;
      };
      await callFn().catch(() => {});
      expect(captured).toBe(expectedCmd);
    });
  }
});
