import { DbDriver } from "./createTable";
import { IndexInfo } from "@/services/api";

export interface IndexDef {
  id: string;
  originalName: string | null;
  name: string;
  unique: boolean;
  columns: string[];
  indexMethod: string;
  clustered: boolean;
  concurrently: boolean;
}

export interface ManageIndexResult {
  sql: string;
  statements: string[];
}

let _idxIdCounter = 0;
export function newIndexId(): string {
  return `idx-${++_idxIdCounter}-${Date.now()}`;
}

export function indexInfoToIndexDef(info: IndexInfo): IndexDef {
  return {
    id: info.name,
    originalName: info.name,
    name: info.name,
    unique: info.unique,
    columns: info.columns,
    indexMethod: info.indexType ?? "",
    clustered: (info.indexType ?? "").toUpperCase() === "CLUSTERED",
    concurrently: false,
  };
}

export function getIndexMethodOptions(driver: DbDriver): string[] {
  switch (driver) {
    case "postgres":
      return ["btree", "hash", "gist", "gin", "brin"];
    case "mysql":
    case "mariadb":
    case "tidb":
      return ["BTREE", "HASH"];
    default:
      return [];
  }
}

export function supportsIndexManagement(driver: DbDriver): boolean {
  return driver !== "clickhouse" && driver !== "starrocks";
}

function quoteIdent(name: string, driver: DbDriver): string {
  if (driver === "mssql") return `[${name}]`;
  if (driver === "mysql" || driver === "mariadb" || driver === "tidb")
    return `\`${name}\``;
  return `"${name}"`;
}

function buildTableRef(schema: string, table: string, driver: DbDriver): string {
  const q = (n: string) => quoteIdent(n, driver);
  if (driver === "sqlite" || driver === "duckdb") return q(table);
  if (schema) return `${q(schema)}.${q(table)}`;
  return q(table);
}

function buildDropSQL(
  indexName: string,
  schema: string,
  table: string,
  driver: DbDriver,
): string {
  const q = (n: string) => quoteIdent(n, driver);
  switch (driver) {
    case "mysql":
    case "mariadb":
    case "tidb":
      return `DROP INDEX ${q(indexName)} ON ${buildTableRef(schema, table, driver)};`;
    case "postgres":
      return `DROP INDEX IF EXISTS ${schema ? `${q(schema)}.` : ""}${q(indexName)};`;
    case "sqlite":
      return `DROP INDEX ${q(indexName)};`;
    case "mssql":
      return `DROP INDEX ${q(indexName)} ON ${buildTableRef(schema, table, driver)};`;
    case "duckdb":
      return `DROP INDEX IF EXISTS ${q(indexName)};`;
    case "oracle":
      return `DROP INDEX ${q(indexName)};`;
    default:
      return `DROP INDEX ${q(indexName)};`;
  }
}

function buildCreateSQL(
  def: IndexDef,
  schema: string,
  table: string,
  driver: DbDriver,
): string {
  const q = (n: string) => quoteIdent(n, driver);
  const tableRef = buildTableRef(schema, table, driver);
  const cols = def.columns.map((c) => q(c)).join(", ");
  const unique = def.unique ? "UNIQUE " : "";

  switch (driver) {
    case "mysql":
    case "mariadb":
    case "tidb": {
      const using = def.indexMethod ? ` USING ${def.indexMethod}` : "";
      return `CREATE ${unique}INDEX ${q(def.name)} ON ${tableRef} (${cols})${using};`;
    }
    case "postgres": {
      const concurrently = def.concurrently ? "CONCURRENTLY " : "";
      const using = def.indexMethod ? ` USING ${def.indexMethod}` : "";
      return `CREATE ${unique}INDEX ${concurrently}${q(def.name)} ON ${tableRef}${using} (${cols});`;
    }
    case "sqlite":
      return `CREATE ${unique}INDEX ${q(def.name)} ON ${tableRef} (${cols});`;
    case "mssql": {
      const clustered = def.clustered ? "CLUSTERED " : "NONCLUSTERED ";
      return `CREATE ${unique}${clustered}INDEX ${q(def.name)} ON ${tableRef} (${cols});`;
    }
    case "duckdb":
      return `CREATE INDEX ${q(def.name)} ON ${tableRef} (${cols});`;
    case "oracle":
      return `CREATE ${unique}INDEX ${q(def.name)} ON ${tableRef} (${cols});`;
    default:
      return `CREATE ${unique}INDEX ${q(def.name)} ON ${tableRef} (${cols});`;
  }
}

export function generateManageIndexSQL(
  schema: string,
  table: string,
  originalIndexes: IndexInfo[],
  currentDefs: IndexDef[],
  driver: DbDriver,
): ManageIndexResult {
  const statements: string[] = [];

  const originalMap = new Map(originalIndexes.map((idx) => [idx.name, idx]));
  const currentOriginalNames = new Set(
    currentDefs
      .filter((d) => d.originalName !== null)
      .map((d) => d.originalName as string),
  );

  // DROP indexes that were removed
  for (const orig of originalIndexes) {
    if (!currentOriginalNames.has(orig.name)) {
      statements.push(buildDropSQL(orig.name, schema, table, driver));
    }
  }

  // For each current def, determine if it needs create/recreate
  for (const def of currentDefs) {
    if (!def.name.trim() || def.columns.length === 0) continue;

    if (def.originalName === null) {
      // New index
      statements.push(buildCreateSQL(def, schema, table, driver));
    } else {
      // Existing index — check if changed
      const orig = originalMap.get(def.originalName);
      if (!orig) {
        statements.push(buildCreateSQL(def, schema, table, driver));
        continue;
      }

      const nameChanged = def.name !== def.originalName;
      const uniqueChanged = def.unique !== orig.unique;
      const colsChanged =
        JSON.stringify(def.columns) !== JSON.stringify(orig.columns);
      const methodChanged =
        def.indexMethod !== (orig.indexType ?? "") &&
        def.indexMethod !== "";

      if (nameChanged || uniqueChanged || colsChanged || methodChanged) {
        statements.push(buildDropSQL(def.originalName, schema, table, driver));
        statements.push(buildCreateSQL(def, schema, table, driver));
      }
    }
  }

  return {
    sql: statements.join("\n"),
    statements,
  };
}
