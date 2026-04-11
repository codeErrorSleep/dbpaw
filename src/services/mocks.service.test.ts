import { describe, expect, test } from "bun:test";
import {
  invokeMock,
  mockGetMysqlCharsets,
  mockGetMysqlCollations,
} from "./mocks";

describe("invokeMock service layer", () => {
  test("returns table list for metadata command", async () => {
    const tables = await invokeMock<
      { schema: string; name: string; type: string }[]
    >("list_tables", {
      id: 1,
      database: "test_db",
      schema: "public",
    });

    expect(tables.length).toBeGreaterThan(0);
    expect(tables[0]).toHaveProperty("schema");
    expect(tables[0]).toHaveProperty("name");
    expect(tables[0]).toHaveProperty("type");
  });

  test("returns connection list for connection command", async () => {
    const connections = await invokeMock<any[]>("get_connections");
    expect(connections.length).toBeGreaterThan(0);
  });

  test("throws on unknown command", async () => {
    expect(invokeMock("unknown_command_for_test")).rejects.toThrow(
      "Mock: Unknown command",
    );
  });
});

describe("mockGetMysqlCharsets", () => {
  test("returns a non-empty list", async () => {
    const charsets = await mockGetMysqlCharsets(1);
    expect(charsets.length).toBeGreaterThan(0);
  });

  test("contains the three most common charsets", async () => {
    const charsets = await mockGetMysqlCharsets(1);
    expect(charsets).toContain("utf8mb4");
    expect(charsets).toContain("utf8");
    expect(charsets).toContain("latin1");
  });

  test("contains CJK charsets", async () => {
    const charsets = await mockGetMysqlCharsets(1);
    expect(charsets).toContain("gbk");
    expect(charsets).toContain("gb18030");
    expect(charsets).toContain("euckr");
  });

  test("all entries are non-empty strings", async () => {
    const charsets = await mockGetMysqlCharsets(1);
    for (const cs of charsets) {
      expect(typeof cs).toBe("string");
      expect(cs.trim().length).toBeGreaterThan(0);
    }
  });
});

describe("mockGetMysqlCollations", () => {
  test("returns all collations when no charset given", async () => {
    const collations = await mockGetMysqlCollations(1);
    expect(collations.length).toBeGreaterThan(0);
  });

  test("returns collations for utf8mb4", async () => {
    const collations = await mockGetMysqlCollations(1, "utf8mb4");
    expect(collations.length).toBeGreaterThan(0);
    expect(collations).toContain("utf8mb4_general_ci");
    expect(collations).toContain("utf8mb4_unicode_ci");
    for (const col of collations) {
      expect(col.startsWith("utf8mb4")).toBe(true);
    }
  });

  test("returns collations for utf8", async () => {
    const collations = await mockGetMysqlCollations(1, "utf8");
    expect(collations).toContain("utf8_general_ci");
  });

  test("falls back to all collations for unknown charset", async () => {
    const all = await mockGetMysqlCollations(1);
    const unknown = await mockGetMysqlCollations(1, "euckr");
    expect(unknown.length).toBe(all.length);
  });

  test("all entries are non-empty strings", async () => {
    const collations = await mockGetMysqlCollations(1);
    for (const col of collations) {
      expect(typeof col).toBe("string");
      expect(col.trim().length).toBeGreaterThan(0);
    }
  });
});

describe("invokeMock charset/collation commands", () => {
  test("get_mysql_charsets_by_id returns charset array", async () => {
    const charsets = await invokeMock<string[]>("get_mysql_charsets_by_id", {
      id: 1,
    });
    expect(Array.isArray(charsets)).toBe(true);
    expect(charsets).toContain("utf8mb4");
  });

  test("get_mysql_collations_by_id without charset returns all collations", async () => {
    const collations = await invokeMock<string[]>(
      "get_mysql_collations_by_id",
      { id: 1 },
    );
    expect(Array.isArray(collations)).toBe(true);
    expect(collations.length).toBeGreaterThan(0);
  });

  test("get_mysql_collations_by_id with charset filters results", async () => {
    const collations = await invokeMock<string[]>(
      "get_mysql_collations_by_id",
      { id: 1, charset: "utf8mb4" },
    );
    expect(collations).toContain("utf8mb4_general_ci");
    for (const col of collations) {
      expect(col.startsWith("utf8mb4")).toBe(true);
    }
  });
});
