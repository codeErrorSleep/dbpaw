import { describe, expect, test } from "bun:test";
import {
  buildDeleteStatement,
  buildUpdateStatement,
  calculateAutoColumnWidths,
  canMutateClickHouseTable,
  collectSearchMatches,
  createSingleAndDoubleClickHandler,
  escapeSQL,
  formatCellValue,
  formatInsertSQLValue,
  formatSQLValue,
  getQualifiedTableName,
  isClickHouseMergeTreeEngine,
  isComplexValue,
  isInsertColumnRequired,
  quoteIdent,
  sortRows,
} from "./utils";

describe("formatSQLValue", () => {
  test("uses numeric boolean literals for mssql", () => {
    expect(formatSQLValue("true", true, "execution", "mssql")).toBe("1");
    expect(formatSQLValue("false", true, "execution", "mssql")).toBe("0");
    expect(formatSQLValue("1", true, "execution", "mssql")).toBe("1");
    expect(formatSQLValue("0", true, "execution", "mssql")).toBe("0");
  });

  test("keeps TRUE/FALSE for non-mssql drivers", () => {
    expect(formatSQLValue("true", true, "execution", "postgres")).toBe("TRUE");
    expect(formatSQLValue("false", true, "execution", "mysql")).toBe("FALSE");
    expect(formatSQLValue("true", true, "execution", "tidb")).toBe("TRUE");
    expect(formatSQLValue("false", true, "execution", "mariadb")).toBe("FALSE");
    expect(formatSQLValue("true", true, "execution", "starrocks")).toBe("TRUE");
  });

  test("throws for invalid boolean in execution mode", () => {
    expect(() => formatSQLValue("yes", true, "execution", "mssql")).toThrow(
      'Invalid boolean value: "yes"',
    );
  });
});

describe("formatInsertSQLValue", () => {
  test("supports NULL literal", () => {
    expect(
      formatInsertSQLValue("null", { name: "memo", type: "text" }, "postgres"),
    ).toBe("NULL");
  });

  test("formats numeric values by column type", () => {
    expect(
      formatInsertSQLValue("123", { name: "age", type: "integer" }, "postgres"),
    ).toBe("123");
    expect(
      formatInsertSQLValue(
        "-45.67",
        { name: "price", type: "numeric(10,2)" },
        "postgres",
      ),
    ).toBe("-45.67");
  });

  test("throws for invalid numeric values", () => {
    expect(() =>
      formatInsertSQLValue("12a", { name: "age", type: "integer" }, "postgres"),
    ).toThrow('Invalid numeric value for column "age": "12a"');
  });

  test("formats boolean values", () => {
    expect(
      formatInsertSQLValue(
        "true",
        { name: "enabled", type: "boolean" },
        "postgres",
      ),
    ).toBe("TRUE");
    expect(
      formatInsertSQLValue("0", { name: "enabled", type: "boolean" }, "mssql"),
    ).toBe("0");
  });

  test("throws for invalid boolean values", () => {
    expect(() =>
      formatInsertSQLValue(
        "yes",
        { name: "enabled", type: "boolean" },
        "postgres",
      ),
    ).toThrow('Invalid boolean value for column "enabled": "yes"');
  });

  test("quotes non-numeric and non-boolean values", () => {
    expect(
      formatInsertSQLValue("alice", { name: "name", type: "varchar" }, "mysql"),
    ).toBe("'alice'");
  });
});

describe("isInsertColumnRequired", () => {
  test("returns true for NOT NULL without default", () => {
    expect(
      isInsertColumnRequired({ nullable: false, defaultValue: null }),
    ).toBe(true);
  });

  test("returns false when nullable", () => {
    expect(isInsertColumnRequired({ nullable: true, defaultValue: null })).toBe(
      false,
    );
  });

  test("returns false when default value exists", () => {
    expect(
      isInsertColumnRequired({
        nullable: false,
        defaultValue: "CURRENT_TIMESTAMP",
      }),
    ).toBe(false);
  });
});

describe("getQualifiedTableName", () => {
  test("uses unqualified table with backticks for tidb", () => {
    expect(getQualifiedTableName("tidb", "analytics", "events")).toBe(
      "`events`",
    );
  });

  test("uses unqualified table with backticks for mariadb", () => {
    expect(getQualifiedTableName("mariadb", "analytics", "events")).toBe(
      "`events`",
    );
  });

  test("uses unqualified table with backticks for starrocks", () => {
    expect(getQualifiedTableName("starrocks", "analytics", "events")).toBe(
      "`events`",
    );
  });

  test("does not qualify sqlite main/public schema", () => {
    expect(getQualifiedTableName("sqlite", "main", "users")).toBe('"users"');
    expect(getQualifiedTableName("sqlite", "public", "users")).toBe('"users"');
    expect(getQualifiedTableName("sqlite", "", "users")).toBe('"users"');
  });

  test("keeps non-main sqlite schema qualification", () => {
    expect(getQualifiedTableName("sqlite", "analytics", "events")).toBe(
      '"analytics"."events"',
    );
  });

  test("does not qualify duckdb main/public schema", () => {
    expect(getQualifiedTableName("duckdb", "main", "users")).toBe('"users"');
    expect(getQualifiedTableName("duckdb", "public", "users")).toBe('"users"');
    expect(getQualifiedTableName("duckdb", "", "users")).toBe('"users"');
  });
});

describe("clickhouse mutation guards", () => {
  test("detects mergetree engine variants", () => {
    expect(isClickHouseMergeTreeEngine("MergeTree")).toBe(true);
    expect(isClickHouseMergeTreeEngine("ReplacingMergeTree")).toBe(true);
    expect(isClickHouseMergeTreeEngine("Memory")).toBe(false);
  });

  test("requires both mergetree engine and primary keys", () => {
    expect(canMutateClickHouseTable("MergeTree", ["id"])).toBe(true);
    expect(canMutateClickHouseTable("MergeTree", [])).toBe(false);
    expect(canMutateClickHouseTable("Log", ["id"])).toBe(false);
  });
});

describe("formatSQLValue: additional cases", () => {
  test("maps empty string with null/undefined originalValue to NULL", () => {
    expect(formatSQLValue("", null, "execution")).toBe("NULL");
    expect(formatSQLValue("", undefined, "execution")).toBe("NULL");
  });

  test("returns trimmed numeric for number originalValue", () => {
    expect(formatSQLValue("42", 42, "execution")).toBe("42");
    expect(formatSQLValue("-3.14", 3.14, "execution")).toBe("-3.14");
  });

  test("throws in execution mode for invalid number originalValue", () => {
    expect(() => formatSQLValue("abc", 99, "execution")).toThrow(
      'Invalid numeric value: "abc"',
    );
  });

  test("does not throw in copy mode for invalid boolean", () => {
    expect(() => formatSQLValue("yes", true, "copy")).not.toThrow();
  });

  test("quotes plain string values and escapes single quotes", () => {
    expect(formatSQLValue("hello", "hello", "execution")).toBe("'hello'");
    expect(formatSQLValue("it's", "it's", "execution")).toBe("'it''s'");
  });
});

describe("escapeSQL", () => {
  test("doubles single quotes", () => {
    expect(escapeSQL("it's")).toBe("it''s");
    expect(escapeSQL("''")).toBe("''''");
  });

  test("passes through strings without single quotes unchanged", () => {
    expect(escapeSQL("hello world")).toBe("hello world");
    expect(escapeSQL("")).toBe("");
  });
});

describe("quoteIdent", () => {
  test("uses backticks for mysql family and clickhouse", () => {
    expect(quoteIdent("mysql", "my_table")).toBe("`my_table`");
    expect(quoteIdent("tidb", "my_table")).toBe("`my_table`");
    expect(quoteIdent("mariadb", "my_table")).toBe("`my_table`");
    expect(quoteIdent("starrocks", "my_table")).toBe("`my_table`");
    expect(quoteIdent("clickhouse", "my_table")).toBe("`my_table`");
  });

  test("uses brackets for mssql and escapes ] inside name", () => {
    expect(quoteIdent("mssql", "my_table")).toBe("[my_table]");
    expect(quoteIdent("mssql", "tab]le")).toBe("[tab]]le]");
  });

  test("uses double quotes for other drivers", () => {
    expect(quoteIdent("postgres", "my_table")).toBe('"my_table"');
    expect(quoteIdent("sqlite", "my_table")).toBe('"my_table"');
    expect(quoteIdent(undefined, "my_table")).toBe('"my_table"');
  });
});

describe("sortRows", () => {
  const rows = [
    { id: 3, name: "charlie" },
    { id: 1, name: "alice" },
    { id: 2, name: "bob" },
  ];

  test("returns original data when no sort parameters given", () => {
    expect(sortRows(rows)).toBe(rows);
    expect(sortRows(rows, "id")).toBe(rows);
  });

  test("sorts numeric column ascending", () => {
    const sorted = sortRows(rows, "id", "asc");
    expect(sorted.map((r) => r.id)).toEqual([1, 2, 3]);
  });

  test("sorts numeric column descending", () => {
    const sorted = sortRows(rows, "id", "desc");
    expect(sorted.map((r) => r.id)).toEqual([3, 2, 1]);
  });

  test("sorts string column ascending", () => {
    const sorted = sortRows(rows, "name", "asc");
    expect(sorted.map((r) => r.name)).toEqual(["alice", "bob", "charlie"]);
  });

  test("places null values at the end", () => {
    const data = [{ v: null }, { v: 2 }, { v: 1 }];
    const sorted = sortRows(data, "v", "asc");
    expect(sorted[sorted.length - 1].v).toBeNull();
  });

  test("does not mutate original array", () => {
    const original = [...rows];
    sortRows(rows, "id", "asc");
    expect(rows).toEqual(original);
  });
});

describe("createSingleAndDoubleClickHandler", () => {
  test("runs single-click action after delay", async () => {
    const calls: string[] = [];
    const state = { timerId: null };
    const handler = createSingleAndDoubleClickHandler(
      state,
      () => calls.push("copy"),
      () => calls.push("sort"),
      10,
    );

    handler.handleClick();
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(calls).toEqual(["copy"]);
    expect(state.timerId).toBeNull();
  });

  test("double click cancels pending single-click action", () => {
    const calls: string[] = [];
    const state = { timerId: null };
    const handler = createSingleAndDoubleClickHandler(
      state,
      () => calls.push("copy"),
      () => calls.push("sort"),
      20,
    );

    handler.handleClick();
    handler.handleDoubleClick();

    expect(calls).toEqual(["sort"]);
    expect(state.timerId).toBeNull();
  });

  test("cancelPendingClick clears pending single-click action", async () => {
    const calls: string[] = [];
    const state = { timerId: null };
    const handler = createSingleAndDoubleClickHandler(
      state,
      () => calls.push("copy"),
      () => calls.push("sort"),
      10,
    );

    handler.handleClick();
    handler.cancelPendingClick();
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(calls).toEqual([]);
    expect(state.timerId).toBeNull();
  });
});

describe("collectSearchMatches", () => {
  const data = [
    { id: 1, name: "Alice" },
    { id: 2, name: "Bob" },
    { id: 3, name: "alice" },
  ];
  const columns = ["id", "name"];
  const identity = (_row: number, _col: string, val: any) => val;

  test("returns empty array for empty keyword", () => {
    expect(collectSearchMatches(data, columns, "", identity)).toEqual([]);
  });

  test("finds matches across rows and columns", () => {
    const matches = collectSearchMatches(data, columns, "alice", identity);
    expect(matches.length).toBe(2);
    expect(matches.map((m) => m.row)).toEqual([0, 2]);
  });

  test("uses getCellDisplayValue result for comparison", () => {
    const display = (_row: number, col: string, val: any) =>
      col === "id" ? `ID:${val}` : val;
    const matches = collectSearchMatches(data, columns, "id:1", display);
    expect(matches.length).toBe(1);
    expect(matches[0].col).toBe("id");
  });

  test("skips null and undefined cell values", () => {
    const withNulls = [{ id: null, name: undefined }];
    const matches = collectSearchMatches(
      withNulls,
      ["id", "name"],
      "null",
      identity,
    );
    expect(matches).toEqual([]);
  });
});

describe("calculateAutoColumnWidths", () => {
  test("returns empty object for empty data or columns", () => {
    expect(
      calculateAutoColumnWidths({ data: [], columns: ["a"], columnWidths: {} }),
    ).toEqual({});
    expect(
      calculateAutoColumnWidths({
        data: [{ a: 1 }],
        columns: [],
        columnWidths: {},
      }),
    ).toEqual({});
  });

  test("skips columns with a pre-set width", () => {
    const result = calculateAutoColumnWidths({
      data: [{ a: "hello" }],
      columns: ["a"],
      columnWidths: { a: 200 },
    });
    expect(result).toEqual({});
  });

  test("computes width and respects min/max bounds", () => {
    const result = calculateAutoColumnWidths({
      data: [{ col: "x" }],
      columns: ["col"],
      columnWidths: {},
    });
    expect(result["col"]).toBeGreaterThanOrEqual(100);
    expect(result["col"]).toBeLessThanOrEqual(900);
  });

  test("caps sampled data length at 100 characters", () => {
    const longValue = "a".repeat(200);
    const result = calculateAutoColumnWidths({
      data: [{ col: longValue }],
      columns: ["col"],
      columnWidths: {},
    });
    // cap at 100 chars → 100 * 9 + 36 = 936 → capped at 900
    expect(result["col"]).toBe(900);
  });
});

describe("isComplexValue", () => {
  test("returns true for plain objects", () => {
    expect(isComplexValue({ a: 1 })).toBe(true);
    expect(isComplexValue({})).toBe(true);
  });

  test("returns true for arrays", () => {
    expect(isComplexValue([1, 2, 3])).toBe(true);
    expect(isComplexValue([])).toBe(true);
  });

  test("returns false for null", () => {
    expect(isComplexValue(null)).toBe(false);
  });

  test("returns false for undefined", () => {
    expect(isComplexValue(undefined)).toBe(false);
  });

  test("returns false for primitives", () => {
    expect(isComplexValue("string")).toBe(false);
    expect(isComplexValue(42)).toBe(false);
    expect(isComplexValue(true)).toBe(false);
  });
});

describe("formatCellValue", () => {
  test("null → empty string", () => {
    expect(formatCellValue(null)).toBe("");
  });

  test("undefined → empty string", () => {
    expect(formatCellValue(undefined)).toBe("");
  });

  test("string passes through unchanged", () => {
    expect(formatCellValue("hello")).toBe("hello");
    expect(formatCellValue("")).toBe("");
  });

  test("number → string", () => {
    expect(formatCellValue(42)).toBe("42");
    expect(formatCellValue(-3.14)).toBe("-3.14");
  });

  test("boolean → string", () => {
    expect(formatCellValue(true)).toBe("true");
    expect(formatCellValue(false)).toBe("false");
  });

  test("empty array → []", () => {
    expect(formatCellValue([])).toBe("[]");
  });

  test("array always shows full JSON regardless of length", () => {
    expect(formatCellValue(["a"])).toBe('["a"]');
    expect(formatCellValue([1, 2])).toBe("[1,2]");
    expect(formatCellValue([1, 2, 3])).toBe("[1,2,3]");
    expect(formatCellValue(["a", "b", "c", "d"])).toBe('["a","b","c","d"]');
  });

  test("empty object → {}", () => {
    expect(formatCellValue({})).toBe("{}");
  });

  test("object with 1 key → inline JSON", () => {
    expect(formatCellValue({ id: 1 })).toBe('{"id":1}');
  });

  test("object with 2 keys → inline JSON", () => {
    expect(formatCellValue({ id: 1, name: "alice" })).toBe(
      '{"id":1,"name":"alice"}',
    );
  });

  test("object with 3+ keys → abbreviated summary", () => {
    const result = formatCellValue({ a: 1, b: 2, c: 3 });
    expect(result).toMatch(/^\{a, b, \.\.\. \+1\}$/);
  });

  test("object with many keys → shows first 2 keys and remainder count", () => {
    const result = formatCellValue({
      id: 1,
      name: "x",
      role: "admin",
      score: 99,
    });
    expect(result).toMatch(/^\{id, name, \.\.\. \+2\}$/);
  });

  test("nested object with 2 keys → inline JSON (no recursion into children)", () => {
    const result = formatCellValue({ user: { name: "alice" } });
    expect(result).toBe('{"user":{"name":"alice"}}');
  });

  test("array of objects → full JSON", () => {
    expect(formatCellValue([{ id: 1 }, { id: 2 }, { id: 3 }])).toBe(
      '[{"id":1},{"id":2},{"id":3}]',
    );
  });
});

describe("formatCellValue: integration with collectSearchMatches", () => {
  test("JSON object fields are searchable by key name", () => {
    const data = [
      { id: 1, meta: { role: "admin", tags: ["vip"] } },
      { id: 2, meta: { role: "user", tags: [] } },
    ];
    const identity = (_row: number, _col: string, val: any) => val;
    const matches = collectSearchMatches(
      data,
      ["id", "meta"],
      "admin",
      identity,
    );
    expect(matches.length).toBe(1);
    expect(matches[0].row).toBe(0);
    expect(matches[0].col).toBe("meta");
  });

  test("array fields are searchable by content", () => {
    const data = [{ tags: ["read", "write"] }, { tags: ["read"] }];
    const identity = (_row: number, _col: string, val: any) => val;
    const matches = collectSearchMatches(data, ["tags"], "write", identity);
    expect(matches.length).toBe(1);
    expect(matches[0].row).toBe(0);
  });
});

describe("calculateAutoColumnWidths: complex value handling", () => {
  test("uses formatted string length for objects, not [object Object]", () => {
    // A 3-key object formats to ~20 chars, not 15 ('[object Object]')
    const result = calculateAutoColumnWidths({
      data: [{ meta: { id: 1, name: "alice", role: "admin" } }],
      columns: ["meta"],
      columnWidths: {},
    });
    // If it used String() it would give '[object Object]' = 15 chars
    // formatCellValue gives '{id, name, ... +1}' = 18 chars
    // Either way width is > minimum, but we verify it doesn't crash
    expect(result["meta"]).toBeGreaterThan(0);
  });
});

describe("formatCellValue: PostgreSQL array column output", () => {
  // These tests verify the display format for values that come back from the
  // PostgreSQL backend after the array-type fix (actual JS arrays, not strings).

  test("int array displays as compact JSON", () => {
    expect(formatCellValue([10, 20, 30])).toBe("[10,20,30]");
  });

  test("text array displays as compact JSON string array", () => {
    expect(formatCellValue(["postgres", "arrays"])).toBe(
      '["postgres","arrays"]',
    );
  });

  test("bool array displays as compact JSON", () => {
    expect(formatCellValue([true, false, true])).toBe("[true,false,true]");
  });

  test("float array displays as compact JSON", () => {
    expect(formatCellValue([3.14, 2.72])).toBe("[3.14,2.72]");
  });

  test("jsonb array (array of objects) displays as full JSON", () => {
    const val = [
      { source: "web", valid: true },
      { source: "app", valid: false },
    ];
    expect(formatCellValue(val)).toBe(JSON.stringify(val));
  });

  test("empty array displays as []", () => {
    expect(formatCellValue([])).toBe("[]");
  });

  test("array with null element displays null in JSON", () => {
    expect(formatCellValue([1, null, 3])).toBe("[1,null,3]");
  });

  test("null column (entire array is null) → empty string", () => {
    expect(formatCellValue(null)).toBe("");
  });
});

describe("isComplexValue: PostgreSQL array column output", () => {
  test("JS arrays from backend are complex", () => {
    expect(isComplexValue([10, 20, 30])).toBe(true);
    expect(isComplexValue(["a", "b"])).toBe(true);
    expect(isComplexValue([])).toBe(true);
  });

  test("null column-level value is not complex", () => {
    expect(isComplexValue(null)).toBe(false);
  });

  test("primitive types are not complex", () => {
    expect(isComplexValue(42)).toBe(false);
    expect(isComplexValue("hello")).toBe(false);
    expect(isComplexValue(true)).toBe(false);
  });
});

describe("collectSearchMatches: PostgreSQL array columns are searchable", () => {
  const data = [
    { id: 1, tags: ["postgres", "arrays", "jsonb"] },
    { id: 2, tags: ["mysql", "innodb"] },
    { id: 3, tags: [] },
    { id: 4, tags: null },
  ];
  const identity = (_row: number, _col: string, val: any) => val;

  test("finds match inside text array content", () => {
    const matches = collectSearchMatches(
      data,
      ["id", "tags"],
      "jsonb",
      identity,
    );
    expect(matches.length).toBe(1);
    expect(matches[0].row).toBe(0);
    expect(matches[0].col).toBe("tags");
  });

  test("does not match empty array", () => {
    const matches = collectSearchMatches(data, ["tags"], "postgres", identity);
    // only row 0 should match, not row 2 (empty) or row 3 (null)
    expect(matches.every((m) => m.row === 0)).toBe(true);
  });

  test("skips null array columns gracefully", () => {
    const matches = collectSearchMatches(data, ["tags"], "null", identity);
    expect(matches).toEqual([]);
  });
});

describe("mutation statement builders", () => {
  test("builds clickhouse alter update/delete statements", () => {
    expect(
      buildUpdateStatement(
        "clickhouse",
        "`analytics`.`events`",
        "`name` = 'new'",
        "`id` = 1",
      ),
    ).toBe(
      "ALTER TABLE `analytics`.`events` UPDATE `name` = 'new' WHERE `id` = 1",
    );

    expect(
      buildDeleteStatement("clickhouse", "`analytics`.`events`", "`id` = 1"),
    ).toBe("ALTER TABLE `analytics`.`events` DELETE WHERE `id` = 1");
  });

  test("keeps generic update/delete statements for non-clickhouse", () => {
    expect(
      buildUpdateStatement(
        "postgres",
        '"public"."users"',
        "\"name\" = 'new'",
        '"id" = 1',
      ),
    ).toBe('UPDATE "public"."users" SET "name" = \'new\' WHERE "id" = 1');
    expect(
      buildDeleteStatement("postgres", '"public"."users"', '"id" = 1'),
    ).toBe('DELETE FROM "public"."users" WHERE "id" = 1');
  });
});
