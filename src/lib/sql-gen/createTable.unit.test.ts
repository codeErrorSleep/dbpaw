import { describe, expect, test } from "bun:test";
import {
  formatDefault,
  generateCreateTableSQL,
  supportsAutoIncrement,
  type ColumnDef,
  type CreateTableDef,
} from "./createTable";
import type { DbDriver } from "./createTable";

// ─── helpers ─────────────────────────────────────────────────────────────────

function col(
  name: string,
  dataType: string,
  opts: Partial<Omit<ColumnDef, "id" | "name" | "dataType">> = {},
): ColumnDef {
  return {
    id: name,
    name,
    dataType,
    length: "",
    notNull: false,
    primaryKey: false,
    autoIncrement: false,
    defaultValue: "",
    comment: "",
    ...opts,
  };
}

function def(
  tableName: string,
  schema: string,
  columns: ColumnDef[],
  opts: Partial<Omit<CreateTableDef, "tableName" | "schema" | "columns">> = {},
): CreateTableDef {
  return { tableName, schema, columns, ...opts };
}

// ─── formatDefault ────────────────────────────────────────────────────────────

describe("formatDefault", () => {
  test("already single-quoted values pass through unchanged", () => {
    expect(formatDefault("'active'", "VARCHAR")).toBe("'active'");
  });

  test("already double-quoted values pass through unchanged", () => {
    expect(formatDefault('"active"', "VARCHAR")).toBe('"active"');
  });

  test("SQL keywords are not quoted", () => {
    expect(formatDefault("NULL", "TEXT")).toBe("NULL");
    expect(formatDefault("CURRENT_TIMESTAMP", "TIMESTAMP")).toBe(
      "CURRENT_TIMESTAMP",
    );
    expect(formatDefault("CURRENT_DATE", "DATE")).toBe("CURRENT_DATE");
    expect(formatDefault("CURRENT_TIME", "TIME")).toBe("CURRENT_TIME");
    expect(formatDefault("NOW()", "TIMESTAMP")).toBe("NOW()");
  });

  test("function call expressions pass through", () => {
    expect(formatDefault("gen_random_uuid()", "UUID")).toBe(
      "gen_random_uuid()",
    );
    expect(formatDefault("uuid_generate_v4()", "UUID")).toBe(
      "uuid_generate_v4()",
    );
  });

  test("numeric values pass through unquoted", () => {
    expect(formatDefault("0", "INTEGER")).toBe("0");
    expect(formatDefault("42", "BIGINT")).toBe("42");
    expect(formatDefault("-3.14", "NUMERIC")).toBe("-3.14");
  });

  test("bare word with string type gets quoted", () => {
    expect(formatDefault("active", "VARCHAR")).toBe("'active'");
    expect(formatDefault("active", "TEXT")).toBe("'active'");
    expect(formatDefault("active", "CHAR")).toBe("'active'");
    expect(formatDefault("active", "NVARCHAR")).toBe("'active'");
  });

  test("bare word with non-string type is left as-is", () => {
    // e.g. expressions like 'true' for boolean — not a STRING_TYPES match
    expect(formatDefault("true", "BOOLEAN")).toBe("true");
    expect(formatDefault("hello", "INTEGER")).toBe("hello");
  });

  test("bare word with single quote inside gets escaped when auto-quoted", () => {
    expect(formatDefault("it's", "VARCHAR")).toBe("'it''s'");
  });

  test("case-insensitive SQL keywords", () => {
    expect(formatDefault("null", "TEXT")).toBe("null");
    expect(formatDefault("current_timestamp", "TIMESTAMP")).toBe(
      "current_timestamp",
    );
  });
});

// ─── supportsAutoIncrement ────────────────────────────────────────────────────

describe("supportsAutoIncrement", () => {
  const cases: [DbDriver, boolean][] = [
    ["mysql", true],
    ["mariadb", true],
    ["tidb", true],
    ["starrocks", true],
    ["sqlite", true],
    ["postgres", false],
    ["duckdb", false],
    ["clickhouse", false],
    ["mssql", false],
    ["oracle", false],
  ];
  for (const [driver, expected] of cases) {
    test(`${driver} → ${expected}`, () => {
      expect(supportsAutoIncrement(driver)).toBe(expected);
    });
  }
});

// ─── generateCreateTableSQL ───────────────────────────────────────────────────

describe("generateCreateTableSQL: empty inputs", () => {
  test("returns empty string for blank table name", () => {
    expect(
      generateCreateTableSQL(def("", "public", [col("id", "INTEGER")]), "postgres"),
    ).toBe("");
  });

  test("returns empty string for whitespace-only table name", () => {
    expect(
      generateCreateTableSQL(def("   ", "public", [col("id", "INTEGER")]), "postgres"),
    ).toBe("");
  });

  test("returns empty string for empty column list", () => {
    expect(generateCreateTableSQL(def("users", "public", []), "postgres")).toBe(
      "",
    );
  });

  test("skips columns with blank name or type", () => {
    const result = generateCreateTableSQL(
      def("users", "public", [col("", "INTEGER"), col("id", "")]),
      "postgres",
    );
    expect(result).toBe("");
  });
});

describe("generateCreateTableSQL: postgres", () => {
  test("simple table with schema prefix", () => {
    const result = generateCreateTableSQL(
      def("users", "public", [col("id", "BIGINT", { notNull: true, primaryKey: true })]),
      "postgres",
    );
    expect(result).toBe(
      `CREATE TABLE "public"."users" (\n  "id" BIGINT NOT NULL PRIMARY KEY\n);`,
    );
  });

  test("table without schema — unqualified", () => {
    const result = generateCreateTableSQL(
      def("users", "", [col("id", "INTEGER")]),
      "postgres",
    );
    expect(result).toContain(`CREATE TABLE "users"`);
    expect(result).not.toContain('"."users"');
  });

  test("column with length appended", () => {
    const result = generateCreateTableSQL(
      def("users", "public", [col("name", "VARCHAR", { length: "255" })]),
      "postgres",
    );
    expect(result).toContain('"name" VARCHAR(255)');
  });

  test("column with explicit length in type is not doubled", () => {
    const result = generateCreateTableSQL(
      def("users", "public", [col("name", "VARCHAR(100)", { length: "50" })]),
      "postgres",
    );
    // type already has parens, length should NOT be appended again
    expect(result).toContain('"name" VARCHAR(100)');
    expect(result).not.toContain("VARCHAR(100)(50)");
  });

  test("multiple PK columns → PRIMARY KEY constraint at end", () => {
    const result = generateCreateTableSQL(
      def("orders", "public", [
        col("tenant_id", "INTEGER", { primaryKey: true }),
        col("order_id", "INTEGER", { primaryKey: true }),
      ]),
      "postgres",
    );
    expect(result).toContain("PRIMARY KEY");
    expect(result).toContain('"tenant_id"');
    expect(result).toContain('"order_id"');
    // inline PRIMARY KEY should NOT appear on individual column definition lines
    const lines = result.split("\n");
    const colLines = lines.filter(
      (l) =>
        (l.includes("tenant_id") || l.includes("order_id")) &&
        !l.trim().startsWith("PRIMARY KEY"),
    );
    colLines.forEach((l) => expect(l).not.toContain("PRIMARY KEY"));
  });

  test("default value is included", () => {
    const result = generateCreateTableSQL(
      def("users", "public", [
        col("status", "TEXT", { defaultValue: "active" }),
      ]),
      "postgres",
    );
    expect(result).toContain("DEFAULT 'active'");
  });

  test("no ENGINE or CHARSET clauses", () => {
    const result = generateCreateTableSQL(
      def("users", "public", [col("id", "INTEGER")]),
      "postgres",
    );
    expect(result).not.toContain("ENGINE");
    expect(result).not.toContain("CHARSET");
  });
});

describe("generateCreateTableSQL: mysql", () => {
  test("uses backtick quoting and no schema prefix", () => {
    const result = generateCreateTableSQL(
      def("users", "mydb", [col("id", "BIGINT", { primaryKey: true, notNull: true })]),
      "mysql",
    );
    expect(result).toContain("CREATE TABLE `users`");
    expect(result).not.toContain("mydb");
  });

  test("AUTO_INCREMENT on supported column", () => {
    const result = generateCreateTableSQL(
      def("users", "mydb", [
        col("id", "BIGINT", {
          primaryKey: true,
          notNull: true,
          autoIncrement: true,
        }),
      ]),
      "mysql",
    );
    expect(result).toContain("AUTO_INCREMENT");
  });

  test("COMMENT clause on column", () => {
    const result = generateCreateTableSQL(
      def("users", "mydb", [col("email", "VARCHAR", { comment: "user email" })]),
      "mysql",
    );
    expect(result).toContain("COMMENT 'user email'");
  });

  test("ends with ENGINE = InnoDB and utf8mb4 charset", () => {
    const result = generateCreateTableSQL(
      def("users", "mydb", [col("id", "INT")]),
      "mysql",
    );
    expect(result).toContain("ENGINE = InnoDB");
    expect(result).toContain("utf8mb4");
  });

  test("mariadb also gets ENGINE = InnoDB", () => {
    const result = generateCreateTableSQL(
      def("users", "mydb", [col("id", "INT")]),
      "mariadb",
    );
    expect(result).toContain("ENGINE = InnoDB");
  });
});

describe("generateCreateTableSQL: mssql", () => {
  test("uses bracket quoting with schema prefix", () => {
    const result = generateCreateTableSQL(
      def("users", "dbo", [col("id", "INT", { primaryKey: true, notNull: true })]),
      "mssql",
    );
    expect(result).toContain("CREATE TABLE [dbo].[users]");
  });

  test("uses [dbo] as default schema when schema is empty", () => {
    const result = generateCreateTableSQL(
      def("users", "", [col("id", "INT")]),
      "mssql",
    );
    expect(result).toContain("[dbo].[users]");
  });

  test("no AUTO_INCREMENT (MSSQL uses IDENTITY)", () => {
    const result = generateCreateTableSQL(
      def("users", "dbo", [col("id", "INT", { autoIncrement: true })]),
      "mssql",
    );
    expect(result).not.toContain("AUTO_INCREMENT");
    expect(result).not.toContain("AUTOINCREMENT");
  });

  test("no COMMENT clause (MSSQL does not support inline COMMENT)", () => {
    const result = generateCreateTableSQL(
      def("users", "dbo", [col("email", "NVARCHAR", { comment: "user email" })]),
      "mssql",
    );
    expect(result).not.toContain("COMMENT");
  });
});

describe("generateCreateTableSQL: sqlite", () => {
  test("no schema prefix", () => {
    const result = generateCreateTableSQL(
      def("tasks", "main", [col("id", "INTEGER", { primaryKey: true })]),
      "sqlite",
    );
    expect(result).toContain(`CREATE TABLE "tasks"`);
    expect(result).not.toContain("main");
  });

  test("AUTOINCREMENT (not AUTO_INCREMENT) for sqlite", () => {
    const result = generateCreateTableSQL(
      def("tasks", "main", [
        col("id", "INTEGER", { primaryKey: true, autoIncrement: true }),
      ]),
      "sqlite",
    );
    expect(result).toContain("AUTOINCREMENT");
    expect(result).not.toContain("AUTO_INCREMENT");
  });
});

describe("generateCreateTableSQL: clickhouse", () => {
  test("uses backtick quoting without schema prefix", () => {
    const result = generateCreateTableSQL(
      def("events", "default", [col("id", "UInt64")]),
      "clickhouse",
    );
    expect(result).toContain("CREATE TABLE `events`");
    expect(result).not.toContain("default");
  });

  test("appends ENGINE = MergeTree() and ORDER BY", () => {
    const result = generateCreateTableSQL(
      def("events", "default", [col("id", "UInt64")]),
      "clickhouse",
    );
    expect(result).toContain("ENGINE = MergeTree()");
    expect(result).toContain("ORDER BY");
  });

  test("supports COMMENT clause on columns", () => {
    const result = generateCreateTableSQL(
      def("events", "default", [
        col("id", "UInt64", { comment: "event identifier" }),
      ]),
      "clickhouse",
    );
    expect(result).toContain("COMMENT 'event identifier'");
  });
});

describe("generateCreateTableSQL: oracle", () => {
  test("uses double-quote quoting with schema prefix", () => {
    const result = generateCreateTableSQL(
      def("employees", "hr", [col("id", "NUMBER", { primaryKey: true })]),
      "oracle",
    );
    expect(result).toContain(`CREATE TABLE "hr"."employees"`);
  });

  test("no AUTO_INCREMENT (oracle uses SEQUENCE/IDENTITY)", () => {
    const result = generateCreateTableSQL(
      def("employees", "hr", [col("id", "NUMBER", { autoIncrement: true })]),
      "oracle",
    );
    expect(result).not.toContain("AUTO_INCREMENT");
    expect(result).not.toContain("AUTOINCREMENT");
  });
});

describe("generateCreateTableSQL: duckdb", () => {
  test("uses double-quote quoting with schema prefix", () => {
    const result = generateCreateTableSQL(
      def("events", "myschema", [col("id", "INTEGER")]),
      "duckdb",
    );
    expect(result).toContain(`CREATE TABLE "myschema"."events"`);
  });

  test("no AUTO_INCREMENT (DuckDB uses SEQUENCE)", () => {
    const result = generateCreateTableSQL(
      def("events", "main", [col("id", "INTEGER", { autoIncrement: true })]),
      "duckdb",
    );
    expect(result).not.toContain("AUTO_INCREMENT");
  });
});

describe("generateCreateTableSQL: starrocks", () => {
  test("includes DISTRIBUTED BY when distribution is provided", () => {
    const result = generateCreateTableSQL(
      def(
        "events",
        "mydb",
        [col("id", "BIGINT")],
        {
          starrocksDistribution: {
            type: "hash",
            columns: ["id"],
            buckets: "10",
          },
        },
      ),
      "starrocks",
    );
    expect(result).toContain("DISTRIBUTED BY HASH(`id`) BUCKETS 10");
  });

  test("random distribution", () => {
    const result = generateCreateTableSQL(
      def(
        "events",
        "mydb",
        [col("id", "BIGINT")],
        {
          starrocksDistribution: { type: "random", columns: [], buckets: "4" },
        },
      ),
      "starrocks",
    );
    expect(result).toContain("DISTRIBUTED BY RANDOM BUCKETS 4");
  });

  test("falls back to placeholder when distribution is missing", () => {
    const result = generateCreateTableSQL(
      def("events", "mydb", [col("id", "BIGINT")]),
      "starrocks",
    );
    expect(result).toContain("DISTRIBUTED BY HASH(/* column */)");
  });
});
