import { describe, expect, test } from "bun:test";
import {
  CUSTOM_TYPE_SENTINEL,
  columnGridTemplate,
  indexGridTemplate,
  isTextBlobType,
  splitSqlStatements,
} from "./ddlUtils";

// ─── isTextBlobType ───────────────────────────────────────────────────────────

describe("isTextBlobType", () => {
  test.each([
    "TEXT",
    "text",
    "BLOB",
    "blob",
    "TINYTEXT",
    "MEDIUMTEXT",
    "LONGTEXT",
    "TINYBLOB",
    "MEDIUMBLOB",
    "LONGBLOB",
  ])("returns true for %s", (t) => {
    expect(isTextBlobType(t)).toBe(true);
  });

  test.each([
    "VARCHAR",
    "CHAR",
    "INT",
    "BIGINT",
    "JSON",
    "DECIMAL",
    "TIMESTAMP",
  ])("returns false for %s", (t) => {
    expect(isTextBlobType(t)).toBe(false);
  });

  test("ignores trailing length suffix", () => {
    expect(isTextBlobType("TEXT(65535)")).toBe(true);
  });

  test("ignores leading/trailing whitespace", () => {
    expect(isTextBlobType("  BLOB  ")).toBe(true);
  });
});

// ─── splitSqlStatements ───────────────────────────────────────────────────────

describe("splitSqlStatements", () => {
  test("single statement passes through", () => {
    expect(splitSqlStatements("SELECT 1;")).toEqual(["SELECT 1;"]);
  });

  test("splits on semicolon+newline boundary", () => {
    const sql = "CREATE TABLE t (id INT);\nCREATE INDEX idx ON t (id);";
    expect(splitSqlStatements(sql)).toEqual([
      "CREATE TABLE t (id INT);",
      "CREATE INDEX idx ON t (id);",
    ]);
  });

  test("appends semicolon when missing from last statement", () => {
    const sql = "CREATE TABLE t (id INT);\nSELECT 1";
    const result = splitSqlStatements(sql);
    expect(result[1]).toBe("SELECT 1;");
  });

  test("filters empty parts", () => {
    const sql = "CREATE TABLE t (id INT);\n\n\nCREATE INDEX idx ON t (id);";
    expect(splitSqlStatements(sql)).toHaveLength(2);
  });

  test("does not split on semicolons inside a statement (no newline)", () => {
    // semicolon without newline should stay as one statement
    const sql = "SELECT 1; SELECT 2;";
    expect(splitSqlStatements(sql)).toHaveLength(1);
  });

  test("handles Windows line endings", () => {
    const sql = "CREATE TABLE t (id INT);\r\nCREATE INDEX idx ON t (id);";
    expect(splitSqlStatements(sql)).toHaveLength(2);
  });
});

// ─── columnGridTemplate ───────────────────────────────────────────────────────

describe("columnGridTemplate", () => {
  test("includes auto-increment column when enabled", () => {
    const withAI = columnGridTemplate(true);
    const withoutAI = columnGridTemplate(false);
    expect(withAI.split(" ").length).toBe(withoutAI.split(" ").length + 1);
  });

  test("returns a non-empty CSS grid template string", () => {
    expect(columnGridTemplate(false)).toMatch(/\d+px|fr/);
  });
});

// ─── indexGridTemplate ────────────────────────────────────────────────────────

describe("indexGridTemplate", () => {
  test("includes method column when enabled", () => {
    const withMethod = indexGridTemplate(true);
    const withoutMethod = indexGridTemplate(false);
    expect(withMethod.split(" ").length).toBe(withoutMethod.split(" ").length + 1);
  });
});

// ─── CUSTOM_TYPE_SENTINEL ─────────────────────────────────────────────────────

describe("CUSTOM_TYPE_SENTINEL", () => {
  test("is a non-empty string", () => {
    expect(typeof CUSTOM_TYPE_SENTINEL).toBe("string");
    expect(CUSTOM_TYPE_SENTINEL.length).toBeGreaterThan(0);
  });
});
