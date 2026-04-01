import { describe, expect, test, mock } from "bun:test";

// Set environment variable before importing api
process.env.VITE_USE_MOCK = "true";

// Mock invokeMock
const mockInvokeMock = mock();

mock.module("./mocks", () => ({
  invokeMock: mockInvokeMock,
}));

// Mock Tauri invoke
const mockTauriInvoke = mock();

mock.module("@tauri-apps/api/core", () => ({
  invoke: mockTauriInvoke,
}));

// Mock localStorage
const mockLocalStorage: Record<string, string> = {};

Object.defineProperty(global, "window", {
  value: {
    __TAURI_INTERNALS__: undefined,
  },
  writable: true,
});

const loadApi = async () => import(`./api?test=${Date.now()}-${Math.random()}`);

describe("api", () => {
  test("normalizeImportDriver should normalize postgresql to postgres", async () => {
    const api = await loadApi();

    expect(api.normalizeImportDriver("postgresql")).toBe("postgres");
    expect(api.normalizeImportDriver("PostgreSQL")).toBe("postgres");
    expect(api.normalizeImportDriver("pgsql")).toBe("postgres");
  });

  test("normalizeImportDriver should return original driver for non-postgres", async () => {
    const api = await loadApi();

    expect(api.normalizeImportDriver("mysql")).toBe("mysql");
    expect(api.normalizeImportDriver("sqlite")).toBe("sqlite");
    expect(api.normalizeImportDriver("duckdb")).toBe("duckdb");
    expect(api.normalizeImportDriver("mssql")).toBe("mssql");
  });

  test("normalizeImportDriver should handle empty input", async () => {
    const api = await loadApi();

    expect(api.normalizeImportDriver("")).toBe("");
    expect(api.normalizeImportDriver("  ")).toBe("");
    expect(api.normalizeImportDriver("  mysql  ")).toBe("mysql");
  });

  test("getImportDriverCapability should return read_only_not_supported for clickhouse", async () => {
    const api = await loadApi();

    expect(api.getImportDriverCapability("clickhouse")).toBe(
      "read_only_not_supported",
    );
  });

  test("getImportDriverCapability should return supported for supported drivers", async () => {
    const api = await loadApi();

    expect(api.getImportDriverCapability("postgres")).toBe("supported");
    expect(api.getImportDriverCapability("mysql")).toBe("supported");
    expect(api.getImportDriverCapability("sqlite")).toBe("supported");
    expect(api.getImportDriverCapability("duckdb")).toBe("supported");
    expect(api.getImportDriverCapability("mssql")).toBe("supported");
  });

  test("getImportDriverCapability should return unsupported for unknown drivers", async () => {
    const api = await loadApi();

    expect(api.getImportDriverCapability("oracle")).toBe("unsupported");
    expect(api.getImportDriverCapability("")).toBe("unsupported");
  });

  test("isTauri should return false when __TAURI_INTERNALS__ not exists", async () => {
    delete (global.window as any).__TAURI_INTERNALS__;
    const api = await loadApi();
    expect(api.isTauri()).toBe(false);
  });
});
