import { describe, expect, test } from "bun:test";
import type { DbDriver } from "./createTable";
import type { IndexDef } from "./manageIndexes";
import { validateColumns, validateIndexDefs } from "./tableValidation";

// ─── helpers ──────────────────────────────────────────────────────────────────

// Minimal t() that returns the key so assertions are readable
const t = (key: string, opts?: Record<string, unknown>) => {
  if (!opts) return key;
  return Object.entries(opts).reduce(
    (s, [k, v]) => s.replace(`{{${k}}}`, String(v)),
    key,
  );
};

function col(
  name: string,
  dataType: string,
  opts: { length?: string; autoIncrement?: boolean; primaryKey?: boolean } = {},
) {
  return {
    name,
    dataType,
    length: opts.length ?? "",
    autoIncrement: opts.autoIncrement ?? false,
    primaryKey: opts.primaryKey ?? false,
  };
}

function idx(
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

// ─── validateColumns — required fields ───────────────────────────────────────

describe("validateColumns — name/type required", () => {
  const opts = { driver: "mysql" as DbDriver, showAutoIncrement: false, t };

  test("empty name produces error", () => {
    const errs = validateColumns([col("", "INT")], opts);
    expect(errs.some((e) => e.includes("createTable.validation.columnNameRequired"))).toBe(true);
  });

  test("empty type produces error", () => {
    const errs = validateColumns([col("id", "")], opts);
    expect(errs.some((e) => e.includes("createTable.validation.columnTypeRequired"))).toBe(true);
  });

  test("valid name+type produces no error", () => {
    expect(validateColumns([col("id", "INT")], opts)).toEqual([]);
  });
});

describe("validateColumns — duplicate names", () => {
  const opts = { driver: "postgres" as DbDriver, showAutoIncrement: false, t };

  test("duplicate column names produce error", () => {
    const errs = validateColumns([col("id", "INT"), col("id", "TEXT")], opts);
    expect(errs.some((e) => e.includes("duplicateColumnName"))).toBe(true);
  });

  test("duplicate detection is case-insensitive", () => {
    const errs = validateColumns([col("Id", "INT"), col("ID", "TEXT")], opts);
    expect(errs.some((e) => e.includes("duplicateColumnName"))).toBe(true);
  });
});

describe("validateColumns — AUTO_INCREMENT (MySQL)", () => {
  const opts = { driver: "mysql" as DbDriver, showAutoIncrement: true, t };

  test("multiple AUTO_INCREMENT columns produce error", () => {
    const errs = validateColumns(
      [col("a", "INT", { autoIncrement: true, primaryKey: true }),
       col("b", "INT", { autoIncrement: true, primaryKey: true })],
      opts,
    );
    expect(errs.some((e) => e.includes("multipleAutoIncrement"))).toBe(true);
  });

  test("AUTO_INCREMENT without PRIMARY KEY produces error", () => {
    const errs = validateColumns(
      [col("id", "INT", { autoIncrement: true, primaryKey: false })],
      opts,
    );
    expect(errs.some((e) => e.includes("autoIncrementNeedsKey"))).toBe(true);
  });

  test("AUTO_INCREMENT with PRIMARY KEY is valid", () => {
    expect(
      validateColumns(
        [col("id", "INT", { autoIncrement: true, primaryKey: true })],
        opts,
      ),
    ).toEqual([]);
  });

  test("showAutoIncrement=false skips check even with AI columns", () => {
    const errs = validateColumns(
      [col("a", "INT", { autoIncrement: true, primaryKey: false }),
       col("b", "INT", { autoIncrement: true, primaryKey: false })],
      { ...opts, showAutoIncrement: false },
    );
    expect(errs.filter((e) => e.includes("AutoIncrement"))).toHaveLength(0);
  });
});

describe("validateColumns — VARCHAR length (MySQL/MariaDB/TiDB/MSSQL)", () => {
  test.each(["mysql", "mariadb", "tidb", "mssql"] as DbDriver[])(
    "%s: VARCHAR without length produces error",
    (driver) => {
      const errs = validateColumns(
        [col("name", "VARCHAR")],
        { driver, showAutoIncrement: false, t },
      );
      expect(errs.some((e) => e.includes("varcharNeedsLength"))).toBe(true);
    },
  );

  test("postgres: VARCHAR without length is allowed", () => {
    const errs = validateColumns(
      [col("name", "VARCHAR")],
      { driver: "postgres", showAutoIncrement: false, t },
    );
    expect(errs.filter((e) => e.includes("varcharNeedsLength"))).toHaveLength(0);
  });

  test("mysql: VARCHAR(0) produces zero-length error", () => {
    const errs = validateColumns(
      [col("name", "VARCHAR", { length: "0" })],
      { driver: "mysql", showAutoIncrement: false, t },
    );
    expect(errs.some((e) => e.includes("varcharZeroLength"))).toBe(true);
  });

  test("mysql: VARCHAR with valid length is fine", () => {
    expect(
      validateColumns(
        [col("name", "VARCHAR", { length: "255" })],
        { driver: "mysql", showAutoIncrement: false, t },
      ),
    ).toEqual([]);
  });
});

describe("validateColumns — DECIMAL scale", () => {
  const opts = { driver: "mysql" as DbDriver, showAutoIncrement: false, t };

  test("scale > precision produces error", () => {
    const errs = validateColumns(
      [col("price", "DECIMAL", { length: "2,5" })],
      opts,
    );
    expect(errs.some((e) => e.includes("decimalScaleExceedsPrecision"))).toBe(true);
  });

  test("scale == precision is valid", () => {
    expect(
      validateColumns([col("price", "DECIMAL", { length: "5,5" })], opts),
    ).toEqual([]);
  });

  test("scale < precision is valid", () => {
    expect(
      validateColumns([col("price", "DECIMAL", { length: "10,2" })], opts),
    ).toEqual([]);
  });

  test("single-part length (no scale) produces no error", () => {
    expect(
      validateColumns([col("price", "DECIMAL", { length: "10" })], opts),
    ).toEqual([]);
  });
});

// ─── validateIndexDefs ────────────────────────────────────────────────────────

describe("validateIndexDefs — TEXT/BLOB columns (MySQL prefix restriction)", () => {
  const colTypeMap = new Map([["body", "TEXT"], ["name", "VARCHAR"]]);

  test.each(["mysql", "mariadb", "tidb"] as DbDriver[])(
    "%s: TEXT column in index produces error",
    (driver) => {
      const errs = validateIndexDefs(
        [idx("i", ["body"])],
        colTypeMap,
        { driver, t },
      );
      expect(errs.some((e) => e.includes("indexTextColumn"))).toBe(true);
    },
  );

  test("postgres: TEXT column in index is allowed", () => {
    const errs = validateIndexDefs(
      [idx("i", ["body"])],
      colTypeMap,
      { driver: "postgres", t },
    );
    expect(errs.filter((e) => e.includes("indexTextColumn"))).toHaveLength(0);
  });

  test("varchar column in index is always fine", () => {
    const errs = validateIndexDefs(
      [idx("i", ["name"])],
      colTypeMap,
      { driver: "mysql", t },
    );
    expect(errs).toEqual([]);
  });
});

describe("validateIndexDefs — duplicate columns within an index", () => {
  const colTypeMap = new Map<string, string>();

  test("duplicate column in same index produces error", () => {
    const errs = validateIndexDefs(
      [idx("i", ["a", "a"])],
      colTypeMap,
      { driver: "postgres", t },
    );
    expect(errs.some((e) => e.includes("indexDuplicateColumn"))).toBe(true);
  });

  test("same column in different indexes is fine", () => {
    const errs = validateIndexDefs(
      [idx("i1", ["a"]), idx("i2", ["a"])],
      colTypeMap,
      { driver: "mysql", t },
    );
    expect(errs).toEqual([]);
  });

  test("no duplicates produces no error", () => {
    const errs = validateIndexDefs(
      [idx("i", ["a", "b", "c"])],
      colTypeMap,
      { driver: "mysql", t },
    );
    expect(errs).toEqual([]);
  });
});

describe("validateIndexDefs — unknown column type", () => {
  test("unknown column treated as non-text (no false-positive error)", () => {
    const colTypeMap = new Map<string, string>(); // empty — col type unknown
    const errs = validateIndexDefs(
      [idx("i", ["some_col"])],
      colTypeMap,
      { driver: "mysql", t },
    );
    expect(errs.filter((e) => e.includes("indexTextColumn"))).toHaveLength(0);
  });
});
