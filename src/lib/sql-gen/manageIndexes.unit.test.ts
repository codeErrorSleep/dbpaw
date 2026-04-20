import { describe, expect, test } from "bun:test";
import type { IndexInfo } from "@/services/api";
import {
  generateManageIndexSQL,
  getIndexMethodOptions,
  indexInfoToIndexDef,
  supportsIndexManagement,
  type IndexDef,
} from "./manageIndexes";

// ─── helpers ──────────────────────────────────────────────────────────────────

function idxInfo(
  name: string,
  columns: string[],
  opts: Partial<IndexInfo> = {},
): IndexInfo {
  return { name, columns, unique: false, primary: false, ...opts };
}

function idxDef(
  name: string,
  columns: string[],
  opts: Partial<Omit<IndexDef, "id" | "name" | "columns">> = {},
): IndexDef {
  return {
    id: name,
    originalName: name,
    name,
    columns,
    unique: false,
    indexMethod: "",
    clustered: false,
    concurrently: false,
    ...opts,
  };
}

function newDef(
  name: string,
  columns: string[],
  opts: Partial<Omit<IndexDef, "id" | "name" | "columns">> = {},
): IndexDef {
  return idxDef(name, columns, { ...opts, originalName: null });
}

// ─── supportsIndexManagement ──────────────────────────────────────────────────

describe("supportsIndexManagement", () => {
  test.each(["mysql", "mariadb", "tidb", "postgres", "sqlite", "mssql", "duckdb"])(
    "returns true for %s",
    (driver) => {
      expect(supportsIndexManagement(driver as never)).toBe(true);
    },
  );

  test.each(["clickhouse", "starrocks"])("returns false for %s", (driver) => {
    expect(supportsIndexManagement(driver as never)).toBe(false);
  });
});

// ─── getIndexMethodOptions ────────────────────────────────────────────────────

describe("getIndexMethodOptions", () => {
  test("postgres returns btree/hash/gist/gin/brin", () => {
    expect(getIndexMethodOptions("postgres")).toEqual(
      expect.arrayContaining(["btree", "hash", "gist"]),
    );
  });

  test("mysql returns BTREE and HASH", () => {
    expect(getIndexMethodOptions("mysql")).toEqual(["BTREE", "HASH"]);
  });

  test("sqlite returns empty array", () => {
    expect(getIndexMethodOptions("sqlite")).toEqual([]);
  });
});

// ─── indexInfoToIndexDef ──────────────────────────────────────────────────────

describe("indexInfoToIndexDef", () => {
  test("sets originalName and name from info.name", () => {
    const def = indexInfoToIndexDef(idxInfo("idx_email", ["email"]));
    expect(def.name).toBe("idx_email");
    expect(def.originalName).toBe("idx_email");
  });

  test("maps unique flag", () => {
    const def = indexInfoToIndexDef(idxInfo("u", ["col"], { unique: true }));
    expect(def.unique).toBe(true);
  });

  test("maps columns array", () => {
    const def = indexInfoToIndexDef(idxInfo("i", ["a", "b", "c"]));
    expect(def.columns).toEqual(["a", "b", "c"]);
  });

  test("maps indexMethod from indexType", () => {
    const def = indexInfoToIndexDef(idxInfo("i", ["col"], { indexType: "BTREE" }));
    expect(def.indexMethod).toBe("BTREE");
  });

  test("MSSQL CLUSTERED indexType sets clustered=true", () => {
    const def = indexInfoToIndexDef(idxInfo("i", ["col"], { indexType: "CLUSTERED" }));
    expect(def.clustered).toBe(true);
  });
});

// ─── generateManageIndexSQL ───────────────────────────────────────────────────

describe("generateManageIndexSQL — no changes", () => {
  test("unchanged index produces no SQL", () => {
    const orig = [idxInfo("idx_a", ["a"])];
    const defs = [idxDef("idx_a", ["a"])];
    const { sql } = generateManageIndexSQL("", "t", orig, defs, "mysql");
    expect(sql).toBe("");
  });
});

describe("generateManageIndexSQL — new index", () => {
  test("mysql: CREATE INDEX with backtick quoting", () => {
    const { sql } = generateManageIndexSQL("", "users", [], [newDef("idx_email", ["email"])], "mysql");
    expect(sql).toBe("CREATE INDEX `idx_email` ON `users` (`email`);");
  });

  test("mysql: UNIQUE index", () => {
    const { sql } = generateManageIndexSQL("", "t", [], [newDef("u", ["col"], { unique: true })], "mysql");
    expect(sql).toContain("UNIQUE");
  });

  test("mysql: index method BTREE", () => {
    const { sql } = generateManageIndexSQL(
      "",
      "t",
      [],
      [newDef("i", ["col"], { indexMethod: "BTREE" })],
      "mysql",
    );
    expect(sql).toContain("USING BTREE");
  });

  test("postgres: double-quoted identifiers with schema", () => {
    const { sql } = generateManageIndexSQL("public", "users", [], [newDef("idx_name", ["name"])], "postgres");
    expect(sql).toContain('"idx_name"');
    expect(sql).toContain('"public"."users"');
  });

  test("postgres: CONCURRENTLY flag", () => {
    const { sql } = generateManageIndexSQL(
      "",
      "t",
      [],
      [newDef("i", ["col"], { concurrently: true })],
      "postgres",
    );
    expect(sql).toContain("CONCURRENTLY");
  });

  test("postgres: index method USING", () => {
    const { sql } = generateManageIndexSQL(
      "",
      "t",
      [],
      [newDef("i", ["col"], { indexMethod: "hash" })],
      "postgres",
    );
    expect(sql).toContain("USING hash");
  });

  test("sqlite: no schema prefix", () => {
    const { sql } = generateManageIndexSQL("main", "t", [], [newDef("i", ["col"])], "sqlite");
    expect(sql).not.toContain("main");
    expect(sql).toContain('"i"');
  });

  test("mssql: NONCLUSTERED by default", () => {
    const { sql } = generateManageIndexSQL("dbo", "t", [], [newDef("i", ["col"])], "mssql");
    expect(sql).toContain("NONCLUSTERED");
  });

  test("mssql: CLUSTERED when flagged", () => {
    const { sql } = generateManageIndexSQL(
      "dbo",
      "t",
      [],
      [newDef("i", ["col"], { clustered: true })],
      "mssql",
    );
    expect(sql).toContain("CLUSTERED");
    expect(sql).not.toContain("NONCLUSTERED");
  });

  test("mssql: bracket quoting", () => {
    const { sql } = generateManageIndexSQL("dbo", "t", [], [newDef("i", ["col"])], "mssql");
    expect(sql).toContain("[i]");
    expect(sql).toContain("[dbo].[t]");
  });

  test("duckdb: no UNIQUE support (falls back to plain CREATE)", () => {
    const { sql } = generateManageIndexSQL("", "t", [], [newDef("i", ["col"])], "duckdb");
    expect(sql).toContain("CREATE INDEX");
  });

  test("skips defs with no name or no columns", () => {
    const blank = newDef("", []);
    const { sql } = generateManageIndexSQL("", "t", [], [blank], "mysql");
    expect(sql).toBe("");
  });
});

describe("generateManageIndexSQL — drop removed index", () => {
  test("mysql: DROP INDEX ... ON table", () => {
    const orig = [idxInfo("idx_old", ["col"])];
    const { sql } = generateManageIndexSQL("", "t", orig, [], "mysql");
    expect(sql).toBe("DROP INDEX `idx_old` ON `t`;");
  });

  test("postgres: DROP INDEX IF EXISTS with schema", () => {
    const orig = [idxInfo("idx_old", ["col"])];
    const { sql } = generateManageIndexSQL("public", "t", orig, [], "postgres");
    expect(sql).toBe('DROP INDEX IF EXISTS "public"."idx_old";');
  });

  test("sqlite: simple DROP INDEX", () => {
    const orig = [idxInfo("idx_old", ["col"])];
    const { sql } = generateManageIndexSQL("main", "t", orig, [], "sqlite");
    expect(sql).toBe('DROP INDEX "idx_old";');
  });

  test("duckdb: DROP INDEX IF EXISTS", () => {
    const orig = [idxInfo("idx_old", ["col"])];
    const { sql } = generateManageIndexSQL("", "t", orig, [], "duckdb");
    expect(sql).toContain("DROP INDEX IF EXISTS");
  });
});

describe("generateManageIndexSQL — modify existing index", () => {
  test("rename: produces DROP old + CREATE new", () => {
    const orig = [idxInfo("idx_a", ["col"])];
    const defs = [idxDef("idx_b", ["col"], { originalName: "idx_a" })];
    const { statements } = generateManageIndexSQL("", "t", orig, defs, "mysql");
    expect(statements).toHaveLength(2);
    expect(statements[0]).toContain("DROP");
    expect(statements[1]).toContain("CREATE");
    expect(statements[0]).toContain("`idx_a`");
    expect(statements[1]).toContain("`idx_b`");
  });

  test("column change: produces DROP + CREATE", () => {
    const orig = [idxInfo("idx", ["a"])];
    const defs = [idxDef("idx", ["a", "b"])];
    const { statements } = generateManageIndexSQL("", "t", orig, defs, "mysql");
    expect(statements).toHaveLength(2);
  });

  test("unique flag change: produces DROP + CREATE", () => {
    const orig = [idxInfo("idx", ["col"], { unique: false })];
    const defs = [idxDef("idx", ["col"], { unique: true })];
    const { statements } = generateManageIndexSQL("", "t", orig, defs, "mysql");
    expect(statements).toHaveLength(2);
  });

  test("no change: produces no statements", () => {
    const orig = [idxInfo("idx", ["col"])];
    const defs = [idxDef("idx", ["col"])];
    const { statements } = generateManageIndexSQL("", "t", orig, defs, "mysql");
    expect(statements).toHaveLength(0);
  });
});

describe("generateManageIndexSQL — multi-statement output", () => {
  test("statements array matches sql lines", () => {
    const orig = [idxInfo("idx_a", ["a"])];
    const defs = [newDef("idx_b", ["b"])];
    const { sql, statements } = generateManageIndexSQL("", "t", orig, defs, "mysql");
    expect(statements).toHaveLength(2);
    expect(sql.split("\n")).toHaveLength(2);
  });
});
