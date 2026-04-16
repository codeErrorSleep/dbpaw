import { describe, expect, test } from "bun:test";
import type { ColumnInfo } from "@/services/api";
import {
  columnInfoToAlterDef,
  generateAlterTableSQL,
  type AlterColumnDef,
} from "./alterTable";

// ─── helpers ─────────────────────────────────────────────────────────────────

function col(
  name: string,
  type: string,
  opts: Partial<ColumnInfo> = {},
): ColumnInfo {
  return { name, type, nullable: true, primaryKey: false, ...opts };
}

function alterCol(
  name: string,
  dataType: string,
  originalName: string | null,
  opts: Partial<Omit<AlterColumnDef, "id" | "name" | "dataType" | "originalName">> = {},
): AlterColumnDef {
  return {
    id: name,
    name,
    originalName,
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

// ─── columnInfoToAlterDef ────────────────────────────────────────────────────

describe("columnInfoToAlterDef", () => {
  test("simple type without length", () => {
    const result = columnInfoToAlterDef(
      col("id", "integer", { nullable: false, primaryKey: true }),
    );
    expect(result.name).toBe("id");
    expect(result.originalName).toBe("id");
    expect(result.dataType).toBe("integer");
    expect(result.length).toBe("");
    expect(result.notNull).toBe(true);
    expect(result.primaryKey).toBe(true);
  });

  test("type with length is split into dataType and length", () => {
    const result = columnInfoToAlterDef(col("name", "VARCHAR(255)"));
    expect(result.dataType).toBe("VARCHAR");
    expect(result.length).toBe("255");
  });

  test("type with precision and scale", () => {
    const result = columnInfoToAlterDef(col("price", "DECIMAL(10,2)"));
    expect(result.dataType).toBe("DECIMAL");
    expect(result.length).toBe("10,2");
  });

  test("defaultValue and comment are mapped", () => {
    const result = columnInfoToAlterDef(
      col("status", "TEXT", { defaultValue: "active", comment: "row status" }),
    );
    expect(result.defaultValue).toBe("active");
    expect(result.comment).toBe("row status");
  });

  test("null defaultValue and comment become empty strings", () => {
    const result = columnInfoToAlterDef(
      col("memo", "TEXT", { defaultValue: null, comment: null }),
    );
    expect(result.defaultValue).toBe("");
    expect(result.comment).toBe("");
  });
});

// ─── no changes ──────────────────────────────────────────────────────────────

describe("generateAlterTableSQL: no changes", () => {
  test("returns empty sql when nothing changed", () => {
    const orig = [col("id", "integer"), col("name", "text")];
    const next = [
      alterCol("id", "integer", "id"),
      alterCol("name", "text", "name"),
    ];
    const result = generateAlterTableSQL("public", "users", orig, next, "postgres");
    expect(result.sql).toBe("");
    expect(result.unsupportedOps).toEqual([]);
  });
});

// ─── ADD COLUMN ──────────────────────────────────────────────────────────────

describe("generateAlterTableSQL: ADD COLUMN", () => {
  test("postgres — uses ADD COLUMN with schema-qualified table", () => {
    const orig = [col("id", "integer")];
    const next = [
      alterCol("id", "integer", "id"),
      alterCol("email", "TEXT", null),
    ];
    const { sql } = generateAlterTableSQL("public", "users", orig, next, "postgres");
    expect(sql).toBe(`ALTER TABLE "public"."users" ADD COLUMN "email" TEXT;`);
  });

  test("mysql — uses ADD COLUMN with backtick-quoted table (no schema prefix)", () => {
    const orig = [col("id", "int")];
    const next = [
      alterCol("id", "int", "id"),
      alterCol("email", "VARCHAR", null, { length: "255" }),
    ];
    const { sql } = generateAlterTableSQL("mydb", "users", orig, next, "mysql");
    expect(sql).toBe("ALTER TABLE `users` ADD COLUMN `email` VARCHAR(255);");
  });

  test("mssql — uses ADD (not ADD COLUMN) with bracket-quoted table", () => {
    const orig = [col("id", "INT")];
    const next = [
      alterCol("id", "INT", "id"),
      alterCol("email", "NVARCHAR", null, { length: "255" }),
    ];
    const { sql } = generateAlterTableSQL("dbo", "users", orig, next, "mssql");
    expect(sql).toBe("ALTER TABLE [dbo].[users] ADD [email] NVARCHAR(255);");
  });

  test("mssql — uses [dbo] as default schema when schema is empty", () => {
    const orig = [col("id", "INT")];
    const next = [
      alterCol("id", "INT", "id"),
      alterCol("email", "NVARCHAR", null, { length: "100" }),
    ];
    const { sql } = generateAlterTableSQL("", "users", orig, next, "mssql");
    expect(sql).toBe("ALTER TABLE [dbo].[users] ADD [email] NVARCHAR(100);");
  });

  test("oracle — uses ADD (...) with parentheses", () => {
    const orig = [col("id", "NUMBER")];
    const next = [
      alterCol("id", "NUMBER", "id"),
      alterCol("email", "VARCHAR2", null, { length: "255" }),
    ];
    const { sql } = generateAlterTableSQL("hr", "employees", orig, next, "oracle");
    expect(sql).toBe(`ALTER TABLE "hr"."employees" ADD ("email" VARCHAR2(255));`);
  });

  test("clickhouse — uses ADD COLUMN with backtick table (no schema prefix)", () => {
    const orig = [col("id", "UInt64")];
    const next = [
      alterCol("id", "UInt64", "id"),
      alterCol("score", "Float64", null),
    ];
    const { sql } = generateAlterTableSQL("default", "events", orig, next, "clickhouse");
    expect(sql).toBe("ALTER TABLE `events` ADD COLUMN `score` Float64;");
  });

  test("sqlite — uses ADD COLUMN with double-quoted table", () => {
    const orig = [col("id", "INTEGER")];
    const next = [
      alterCol("id", "INTEGER", "id"),
      alterCol("note", "TEXT", null),
    ];
    const { sql } = generateAlterTableSQL("main", "tasks", orig, next, "sqlite");
    expect(sql).toBe(`ALTER TABLE "tasks" ADD COLUMN "note" TEXT;`);
  });

  test("duckdb — uses ADD COLUMN with schema-qualified table", () => {
    const orig = [col("id", "INTEGER")];
    const next = [
      alterCol("id", "INTEGER", "id"),
      alterCol("note", "TEXT", null),
    ];
    const { sql } = generateAlterTableSQL("main", "tasks", orig, next, "duckdb");
    expect(sql).toBe(`ALTER TABLE "tasks" ADD COLUMN "note" TEXT;`);
  });

  test("ADD COLUMN with NOT NULL constraint", () => {
    const orig = [col("id", "integer")];
    const next = [
      alterCol("id", "integer", "id"),
      alterCol("status", "TEXT", null, { notNull: true }),
    ];
    const { sql } = generateAlterTableSQL("public", "t", orig, next, "postgres");
    expect(sql).toBe(`ALTER TABLE "public"."t" ADD COLUMN "status" TEXT NOT NULL;`);
  });

  test("ADD COLUMN with default value", () => {
    const orig = [col("id", "integer")];
    const next = [
      alterCol("id", "integer", "id"),
      alterCol("active", "BOOLEAN", null, { defaultValue: "TRUE" }),
    ];
    const { sql } = generateAlterTableSQL("public", "t", orig, next, "postgres");
    expect(sql).toBe(`ALTER TABLE "public"."t" ADD COLUMN "active" BOOLEAN DEFAULT TRUE;`);
  });

  test("skips new column with blank name or type", () => {
    const orig = [col("id", "integer")];
    const next = [
      alterCol("id", "integer", "id"),
      alterCol("", "TEXT", null), // blank name — should be skipped
    ];
    const { sql } = generateAlterTableSQL("public", "t", orig, next, "postgres");
    expect(sql).toBe("");
  });
});

// ─── DROP COLUMN ─────────────────────────────────────────────────────────────

describe("generateAlterTableSQL: DROP COLUMN", () => {
  test("postgres — drops column without warning", () => {
    const orig = [col("id", "integer"), col("old_field", "text")];
    const next = [alterCol("id", "integer", "id")];
    const { sql, unsupportedOps } = generateAlterTableSQL(
      "public",
      "users",
      orig,
      next,
      "postgres",
    );
    expect(sql).toBe(`ALTER TABLE "public"."users" DROP COLUMN "old_field";`);
    expect(unsupportedOps).toEqual([]);
  });

  test("sqlite — drops column AND adds unsupportedOps warning", () => {
    const orig = [col("id", "INTEGER"), col("old_field", "TEXT")];
    const next = [alterCol("id", "INTEGER", "id")];
    const { sql, unsupportedOps } = generateAlterTableSQL(
      "main",
      "tasks",
      orig,
      next,
      "sqlite",
    );
    expect(sql).toBe(`ALTER TABLE "tasks" DROP COLUMN "old_field";`);
    expect(unsupportedOps.length).toBe(1);
    expect(unsupportedOps[0]).toMatch(/old_field/);
    expect(unsupportedOps[0]).toMatch(/SQLite/);
  });

  test("mssql — drops column without warning", () => {
    const orig = [col("id", "INT"), col("legacy", "NVARCHAR")];
    const next = [alterCol("id", "INT", "id")];
    const { sql, unsupportedOps } = generateAlterTableSQL(
      "dbo",
      "users",
      orig,
      next,
      "mssql",
    );
    expect(sql).toBe("ALTER TABLE [dbo].[users] DROP COLUMN [legacy];");
    expect(unsupportedOps).toEqual([]);
  });
});

// ─── RENAME COLUMN ───────────────────────────────────────────────────────────

describe("generateAlterTableSQL: RENAME COLUMN", () => {
  test("mysql — uses CHANGE COLUMN for rename (no type change)", () => {
    const orig = [col("old_name", "VARCHAR(255)")];
    const next = [alterCol("new_name", "VARCHAR", "old_name", { length: "255" })];
    const { sql } = generateAlterTableSQL("mydb", "users", orig, next, "mysql");
    expect(sql).toBe(
      "ALTER TABLE `users` CHANGE COLUMN `old_name` `new_name` VARCHAR(255);",
    );
  });

  test("mariadb — uses CHANGE COLUMN for rename", () => {
    const orig = [col("old_name", "TEXT")];
    const next = [alterCol("new_name", "TEXT", "old_name")];
    const { sql } = generateAlterTableSQL("mydb", "t", orig, next, "mariadb");
    expect(sql).toBe("ALTER TABLE `t` CHANGE COLUMN `old_name` `new_name` TEXT;");
  });

  test("tidb — uses CHANGE COLUMN for rename", () => {
    const orig = [col("old_name", "BIGINT")];
    const next = [alterCol("new_name", "BIGINT", "old_name")];
    const { sql } = generateAlterTableSQL("mydb", "t", orig, next, "tidb");
    expect(sql).toBe("ALTER TABLE `t` CHANGE COLUMN `old_name` `new_name` BIGINT;");
  });

  test("postgres — uses RENAME COLUMN", () => {
    const orig = [col("old_name", "text")];
    const next = [alterCol("new_name", "text", "old_name")];
    const { sql } = generateAlterTableSQL("public", "users", orig, next, "postgres");
    expect(sql).toBe(
      `ALTER TABLE "public"."users" RENAME COLUMN "old_name" TO "new_name";`,
    );
  });

  test("duckdb — uses RENAME COLUMN (same as postgres)", () => {
    const orig = [col("old_name", "TEXT")];
    const next = [alterCol("new_name", "TEXT", "old_name")];
    const { sql } = generateAlterTableSQL("main", "t", orig, next, "duckdb");
    expect(sql).toBe(
      `ALTER TABLE "t" RENAME COLUMN "old_name" TO "new_name";`,
    );
  });

  test("sqlite — uses RENAME COLUMN", () => {
    const orig = [col("old_name", "TEXT")];
    const next = [alterCol("new_name", "TEXT", "old_name")];
    const { sql } = generateAlterTableSQL("main", "t", orig, next, "sqlite");
    expect(sql).toBe(`ALTER TABLE "t" RENAME COLUMN "old_name" TO "new_name";`);
  });

  test("mssql — uses sp_rename with schema prefix", () => {
    const orig = [col("old_name", "NVARCHAR")];
    const next = [alterCol("new_name", "NVARCHAR", "old_name")];
    const { sql } = generateAlterTableSQL("dbo", "users", orig, next, "mssql");
    expect(sql).toBe(
      `EXEC sp_rename N'dbo.users.old_name', N'new_name', 'COLUMN';`,
    );
  });

  test("mssql — sp_rename uses 'dbo.' when schema is empty", () => {
    const orig = [col("old_name", "NVARCHAR")];
    const next = [alterCol("new_name", "NVARCHAR", "old_name")];
    const { sql } = generateAlterTableSQL("", "users", orig, next, "mssql");
    expect(sql).toBe(
      `EXEC sp_rename N'dbo.users.old_name', N'new_name', 'COLUMN';`,
    );
  });

  test("clickhouse — uses RENAME COLUMN", () => {
    const orig = [col("old_name", "String")];
    const next = [alterCol("new_name", "String", "old_name")];
    const { sql } = generateAlterTableSQL("default", "events", orig, next, "clickhouse");
    expect(sql).toBe(
      "ALTER TABLE `events` RENAME COLUMN `old_name` TO `new_name`;",
    );
  });

  test("oracle — uses RENAME COLUMN", () => {
    const orig = [col("old_name", "VARCHAR2")];
    const next = [alterCol("new_name", "VARCHAR2", "old_name")];
    const { sql } = generateAlterTableSQL("hr", "employees", orig, next, "oracle");
    expect(sql).toBe(
      `ALTER TABLE "hr"."employees" RENAME COLUMN "old_name" TO "new_name";`,
    );
  });

  test("starrocks — rename is unsupported, emits unsupportedOps", () => {
    const orig = [col("old_name", "STRING")];
    const next = [alterCol("new_name", "STRING", "old_name")];
    const { unsupportedOps } = generateAlterTableSQL(
      "mydb",
      "t",
      orig,
      next,
      "starrocks",
    );
    expect(unsupportedOps.length).toBe(1);
    expect(unsupportedOps[0]).toMatch(/old_name/);
    expect(unsupportedOps[0]).toMatch(/StarRocks/);
  });
});

// ─── TYPE CHANGE ─────────────────────────────────────────────────────────────

describe("generateAlterTableSQL: type change", () => {
  test("postgres — ALTER COLUMN ... TYPE", () => {
    const orig = [col("bio", "text")];
    const next = [alterCol("bio", "VARCHAR", "bio", { length: "500" })];
    const { sql } = generateAlterTableSQL("public", "users", orig, next, "postgres");
    expect(sql).toBe(
      `ALTER TABLE "public"."users" ALTER COLUMN "bio" TYPE VARCHAR(500);`,
    );
  });

  test("duckdb — ALTER COLUMN ... TYPE (same path as postgres)", () => {
    const orig = [col("bio", "TEXT")];
    const next = [alterCol("bio", "VARCHAR", "bio", { length: "500" })];
    const { sql } = generateAlterTableSQL("main", "users", orig, next, "duckdb");
    expect(sql).toBe(
      `ALTER TABLE "users" ALTER COLUMN "bio" TYPE VARCHAR(500);`,
    );
  });

  test("mysql — uses CHANGE COLUMN for type change", () => {
    const orig = [col("status", "VARCHAR(50)")];
    const next = [alterCol("status", "TEXT", "status")];
    const { sql } = generateAlterTableSQL("mydb", "t", orig, next, "mysql");
    expect(sql).toBe("ALTER TABLE `t` CHANGE COLUMN `status` `status` TEXT;");
  });

  test("mssql — uses ALTER COLUMN for type change", () => {
    const orig = [col("bio", "NVARCHAR(255)")];
    const next = [
      alterCol("bio", "NVARCHAR", "bio", { length: "MAX", notNull: false }),
    ];
    const { sql } = generateAlterTableSQL("dbo", "users", orig, next, "mssql");
    expect(sql).toBe(
      "ALTER TABLE [dbo].[users] ALTER COLUMN [bio] NVARCHAR(MAX) NULL;",
    );
  });

  test("sqlite — type change emits unsupported warning", () => {
    const orig = [col("count", "INTEGER")];
    const next = [alterCol("count", "REAL", "count")];
    const { unsupportedOps } = generateAlterTableSQL(
      "main",
      "t",
      orig,
      next,
      "sqlite",
    );
    expect(unsupportedOps.length).toBe(1);
    expect(unsupportedOps[0]).toMatch(/SQLite/);
  });
});

// ─── NOT NULL CHANGE ─────────────────────────────────────────────────────────

describe("generateAlterTableSQL: NOT NULL change", () => {
  test("postgres — SET NOT NULL", () => {
    const orig = [col("email", "text", { nullable: true })];
    const next = [alterCol("email", "text", "email", { notNull: true })];
    const { sql } = generateAlterTableSQL("public", "users", orig, next, "postgres");
    expect(sql).toBe(
      `ALTER TABLE "public"."users" ALTER COLUMN "email" SET NOT NULL;`,
    );
  });

  test("postgres — DROP NOT NULL", () => {
    const orig = [col("email", "text", { nullable: false })];
    const next = [alterCol("email", "text", "email", { notNull: false })];
    const { sql } = generateAlterTableSQL("public", "users", orig, next, "postgres");
    expect(sql).toBe(
      `ALTER TABLE "public"."users" ALTER COLUMN "email" DROP NOT NULL;`,
    );
  });

  test("mssql — type change required when only nullability changes", () => {
    const orig = [col("email", "NVARCHAR(255)", { nullable: true })];
    const next = [
      alterCol("email", "NVARCHAR", "email", { length: "255", notNull: true }),
    ];
    const { sql } = generateAlterTableSQL("dbo", "users", orig, next, "mssql");
    expect(sql).toBe(
      "ALTER TABLE [dbo].[users] ALTER COLUMN [email] NVARCHAR(255) NOT NULL;",
    );
  });
});

// ─── DEFAULT CHANGE ──────────────────────────────────────────────────────────

describe("generateAlterTableSQL: DEFAULT change", () => {
  test("postgres — SET DEFAULT", () => {
    const orig = [col("status", "text", { defaultValue: null })];
    const next = [alterCol("status", "text", "status", { defaultValue: "active" })];
    const { sql } = generateAlterTableSQL("public", "t", orig, next, "postgres");
    expect(sql).toContain("SET DEFAULT");
    expect(sql).toContain("'active'");
  });

  test("postgres — DROP DEFAULT when new default is empty", () => {
    const orig = [col("status", "text", { defaultValue: "active" })];
    const next = [alterCol("status", "text", "status", { defaultValue: "" })];
    const { sql } = generateAlterTableSQL("public", "t", orig, next, "postgres");
    expect(sql).toBe(
      `ALTER TABLE "public"."t" ALTER COLUMN "status" DROP DEFAULT;`,
    );
  });

  test("mssql — DEFAULT change emits unsupportedOps", () => {
    const orig = [col("status", "NVARCHAR(50)", { defaultValue: "active" })];
    const next = [
      alterCol("status", "NVARCHAR", "status", {
        length: "50",
        defaultValue: "inactive",
      }),
    ];
    const { unsupportedOps } = generateAlterTableSQL(
      "dbo",
      "t",
      orig,
      next,
      "mssql",
    );
    expect(unsupportedOps.length).toBe(1);
    expect(unsupportedOps[0]).toMatch(/DEFAULT/);
    expect(unsupportedOps[0]).toMatch(/MSSQL/i);
  });
});

// ─── COMMENT CHANGE ──────────────────────────────────────────────────────────

describe("generateAlterTableSQL: COMMENT change", () => {
  test("postgres — emits COMMENT ON COLUMN", () => {
    const orig = [col("email", "text", { comment: null })];
    const next = [alterCol("email", "text", "email", { comment: "user email" })];
    const { sql } = generateAlterTableSQL("public", "users", orig, next, "postgres");
    expect(sql).toBe(
      `COMMENT ON COLUMN "public"."users"."email" IS 'user email';`,
    );
  });

  test("postgres — sets comment to NULL when cleared", () => {
    const orig = [col("email", "text", { comment: "user email" })];
    const next = [alterCol("email", "text", "email", { comment: "" })];
    const { sql } = generateAlterTableSQL("public", "users", orig, next, "postgres");
    expect(sql).toBe(
      `COMMENT ON COLUMN "public"."users"."email" IS NULL;`,
    );
  });

  test("mysql — includes COMMENT in CHANGE COLUMN", () => {
    const orig = [col("email", "TEXT", { comment: null })];
    const next = [alterCol("email", "TEXT", "email", { comment: "user email" })];
    const { sql } = generateAlterTableSQL("mydb", "users", orig, next, "mysql");
    expect(sql).toBe(
      "ALTER TABLE `users` CHANGE COLUMN `email` `email` TEXT COMMENT 'user email';",
    );
  });

  test("duckdb — does NOT emit COMMENT ON COLUMN (postgres-only feature)", () => {
    const orig = [col("email", "TEXT", { comment: null })];
    const next = [alterCol("email", "TEXT", "email", { comment: "user email" })];
    const { sql } = generateAlterTableSQL("main", "users", orig, next, "duckdb");
    // duckdb shares postgres rename/type path but NOT the COMMENT ON COLUMN path
    expect(sql).not.toContain("COMMENT ON COLUMN");
  });
});

// ─── COMBINED OPERATIONS ─────────────────────────────────────────────────────

describe("generateAlterTableSQL: combined operations", () => {
  test("postgres — rename + type change produces two statements", () => {
    const orig = [col("old_bio", "text")];
    const next = [alterCol("bio", "VARCHAR", "old_bio", { length: "500" })];
    const { sql } = generateAlterTableSQL("public", "users", orig, next, "postgres");
    const lines = sql.split("\n");
    expect(lines.length).toBe(2);
    expect(lines[0]).toContain("RENAME COLUMN");
    expect(lines[1]).toContain("ALTER COLUMN");
    expect(lines[1]).toContain("TYPE");
  });

  test("clickhouse — rename + modify produces two statements", () => {
    const orig = [col("old_score", "Float32")];
    const next = [alterCol("score", "Float64", "old_score")];
    const { sql } = generateAlterTableSQL(
      "default",
      "events",
      orig,
      next,
      "clickhouse",
    );
    const lines = sql.split("\n");
    expect(lines.length).toBe(2);
    expect(lines[0]).toContain("RENAME COLUMN");
    expect(lines[1]).toContain("MODIFY COLUMN");
  });

  test("multiple drops produce multiple statements", () => {
    const orig = [
      col("id", "integer"),
      col("a", "text"),
      col("b", "text"),
    ];
    const next = [alterCol("id", "integer", "id")];
    const { sql } = generateAlterTableSQL("public", "t", orig, next, "postgres");
    expect(sql).toContain('"a"');
    expect(sql).toContain('"b"');
    expect(sql.split("\n").length).toBe(2);
  });
});

// ─── SCHEMA QUALIFICATION ─────────────────────────────────────────────────────

describe("generateAlterTableSQL: schema qualification", () => {
  test("postgres with schema", () => {
    const orig = [col("id", "integer")];
    const next = [alterCol("id", "BIGINT", "id")];
    const { sql } = generateAlterTableSQL("myschema", "t", orig, next, "postgres");
    expect(sql).toContain('"myschema"."t"');
  });

  test("postgres without schema — unqualified table", () => {
    const orig = [col("id", "integer")];
    const next = [alterCol("id", "BIGINT", "id")];
    const { sql } = generateAlterTableSQL("", "t", orig, next, "postgres");
    expect(sql).toContain('"t"');
    expect(sql).not.toContain('"."t"');
  });

  test("mssql with schema", () => {
    const orig = [col("id", "INT")];
    const next = [alterCol("id", "BIGINT", "id", { notNull: true })];
    const { sql } = generateAlterTableSQL("sales", "orders", orig, next, "mssql");
    expect(sql).toContain("[sales].[orders]");
  });

  test("mssql without schema defaults to [dbo]", () => {
    const orig = [col("id", "INT")];
    const next = [alterCol("id", "BIGINT", "id", { notNull: true })];
    const { sql } = generateAlterTableSQL("", "orders", orig, next, "mssql");
    expect(sql).toContain("[dbo].[orders]");
  });

  test("oracle with schema", () => {
    const orig = [col("id", "NUMBER")];
    const next = [alterCol("new_id", "NUMBER", "id")];
    const { sql } = generateAlterTableSQL("hr", "employees", orig, next, "oracle");
    expect(sql).toContain('"hr"."employees"');
  });

  test("mysql — never includes schema prefix", () => {
    const orig = [col("id", "INT")];
    const next = [alterCol("user_id", "INT", "id")];
    const { sql } = generateAlterTableSQL("mydb", "users", orig, next, "mysql");
    expect(sql).not.toContain("mydb");
    expect(sql).toContain("`users`");
  });
});
