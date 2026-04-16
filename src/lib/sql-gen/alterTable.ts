import type { ColumnInfo } from "@/services/api";
import {
  ColumnDef,
  DbDriver,
  formatDefault,
  supportsAutoIncrement,
} from "./createTable";

// ─── types ────────────────────────────────────────────────────────────────────

export interface AlterColumnDef extends ColumnDef {
  /**
   * The original column name as fetched from the database.
   * - `null`   → brand-new column (will become ADD COLUMN)
   * - `string` → pre-loaded from existing schema (RENAME / MODIFY / keep)
   */
  originalName: string | null;
}

export interface AlterTableResult {
  /** One or more ALTER TABLE statements joined by newlines. Empty string = no changes. */
  sql: string;
  /** Human-readable list of operations that cannot be performed on this driver. */
  unsupportedOps: string[];
}

// ─── conversion helpers ───────────────────────────────────────────────────────

/**
 * Convert a `ColumnInfo` from the backend into an `AlterColumnDef` suitable for
 * pre-populating the alter-table editor.  The type string "VARCHAR(255)" is
 * split into dataType="VARCHAR" and length="255" so the type selector works.
 */
export function columnInfoToAlterDef(col: ColumnInfo): AlterColumnDef {
  const typeMatch = /^([^(]+)\(([^)]+)\)$/.exec(col.type);
  return {
    id: col.name,
    name: col.name,
    originalName: col.name,
    dataType: typeMatch ? typeMatch[1].trim() : col.type,
    length: typeMatch ? typeMatch[2].trim() : "",
    notNull: !col.nullable,
    primaryKey: col.primaryKey,
    autoIncrement: false,
    defaultValue: col.defaultValue ?? "",
    comment: col.comment ?? "",
  };
}

// ─── internal helpers ─────────────────────────────────────────────────────────

function q(name: string, driver: DbDriver): string {
  switch (driver) {
    case "mysql":
    case "mariadb":
    case "tidb":
    case "starrocks":
    case "clickhouse":
      return `\`${name}\``;
    case "mssql":
      return `[${name}]`;
    default:
      return `"${name}"`;
  }
}

function tref(schema: string, table: string, driver: DbDriver): string {
  const qt = q(table, driver);
  switch (driver) {
    case "mysql":
    case "mariadb":
    case "tidb":
    case "starrocks":
    case "clickhouse":
    case "sqlite":
    case "duckdb":
      return qt;
    case "mssql":
      return schema.trim() ? `[${schema}].${qt}` : `[dbo].${qt}`;
    case "oracle":
      return schema.trim() ? `"${schema}".${qt}` : qt;
    default: // postgres
      return schema.trim() ? `"${schema}".${qt}` : qt;
  }
}

function colTypePart(col: ColumnDef): string {
  let t = col.dataType.trim();
  if (col.length.trim() && !t.includes("(")) t += `(${col.length.trim()})`;
  return t;
}

function supportsComment(driver: DbDriver): boolean {
  return ["mysql", "mariadb", "tidb", "starrocks", "clickhouse"].includes(
    driver,
  );
}

/** Build the inline column definition for ADD / CHANGE / MODIFY statements. */
function buildColDef(
  col: ColumnDef,
  driver: DbDriver,
  includePk = true,
): string {
  const parts = [`${q(col.name, driver)} ${colTypePart(col)}`];
  if (col.notNull) parts.push("NOT NULL");
  if (col.defaultValue.trim())
    parts.push(`DEFAULT ${formatDefault(col.defaultValue, col.dataType)}`);
  if (col.autoIncrement && supportsAutoIncrement(driver)) {
    parts.push(driver === "sqlite" ? "AUTOINCREMENT" : "AUTO_INCREMENT");
  }
  if (includePk && col.primaryKey) parts.push("PRIMARY KEY");
  if (col.comment.trim() && supportsComment(driver)) {
    parts.push(`COMMENT '${col.comment.replace(/'/g, "''")}'`);
  }
  return parts.join(" ");
}

// ─── main export ──────────────────────────────────────────────────────────────

/**
 * Diff `originalCols` (from the database) against `newCols` (from the editor)
 * and produce the ALTER TABLE SQL required to migrate the table.
 */
export function generateAlterTableSQL(
  schema: string,
  table: string,
  originalCols: ColumnInfo[],
  newCols: AlterColumnDef[],
  driver: DbDriver,
): AlterTableResult {
  const tr = tref(schema, table, driver);
  const statements: string[] = [];
  const unsupportedOps: string[] = [];

  // Which original columns were kept (identified by originalName in new list)
  const keptOriginalNames = new Set(
    newCols.filter((c) => c.originalName !== null).map((c) => c.originalName!),
  );
  const droppedCols = originalCols.filter(
    (c) => !keptOriginalNames.has(c.name),
  );

  // 1. DROP columns ─────────────────────────────────────────────────────────
  for (const col of droppedCols) {
    if (driver === "sqlite") {
      unsupportedOps.push(
        `DROP COLUMN "${col.name}" (requires SQLite ≥ 3.35; may not be supported)`,
      );
    }
    statements.push(`ALTER TABLE ${tr} DROP COLUMN ${q(col.name, driver)};`);
  }

  // 2. ADD new columns ───────────────────────────────────────────────────────
  const addedCols = newCols.filter(
    (c) => c.originalName === null && c.name.trim() && c.dataType.trim(),
  );
  for (const col of addedCols) {
    if (driver === "oracle") {
      statements.push(`ALTER TABLE ${tr} ADD (${buildColDef(col, driver)});`);
    } else if (driver === "mssql") {
      // SQL Server uses ADD without the COLUMN keyword
      statements.push(`ALTER TABLE ${tr} ADD ${buildColDef(col, driver)};`);
    } else {
      statements.push(
        `ALTER TABLE ${tr} ADD COLUMN ${buildColDef(col, driver)};`,
      );
    }
  }

  // 3. RENAME / MODIFY existing columns ─────────────────────────────────────
  for (const col of newCols.filter((c) => c.originalName !== null)) {
    const orig = originalCols.find((c) => c.name === col.originalName);
    if (!orig) continue;

    const renamed = col.name !== col.originalName;
    const origTypeRaw = orig.type;
    const newType = colTypePart(col);
    const typeChanged = newType.toUpperCase() !== origTypeRaw.toUpperCase();
    const notNullChanged = col.notNull !== !orig.nullable;
    const defaultChanged =
      col.defaultValue.trim() !== (orig.defaultValue ?? "").trim();
    const commentChanged = col.comment.trim() !== (orig.comment ?? "").trim();
    const hasPropertyChange =
      typeChanged || notNullChanged || defaultChanged || commentChanged;

    if (!renamed && !hasPropertyChange) continue;

    switch (driver) {
      case "mysql":
      case "mariadb":
      case "tidb": {
        // CHANGE COLUMN handles both rename and modify in one statement
        statements.push(
          `ALTER TABLE ${tr} CHANGE COLUMN ${q(col.originalName!, driver)} ${buildColDef(col, driver, false)};`,
        );
        break;
      }

      case "starrocks": {
        if (renamed) {
          unsupportedOps.push(
            `RENAME COLUMN "${col.originalName}" — StarRocks does not support column renaming`,
          );
        }
        if (hasPropertyChange) {
          statements.push(
            `ALTER TABLE ${tr} MODIFY COLUMN ${buildColDef(col, driver, false)};`,
          );
        }
        break;
      }

      case "sqlite": {
        if (renamed) {
          statements.push(
            `ALTER TABLE ${tr} RENAME COLUMN ${q(col.originalName!, driver)} TO ${q(col.name, driver)};`,
          );
        }
        if (typeChanged || notNullChanged || defaultChanged) {
          unsupportedOps.push(
            `MODIFY "${col.originalName}" — SQLite does not support changing column type or constraints`,
          );
        }
        break;
      }

      case "postgres":
      case "duckdb": {
        if (renamed) {
          statements.push(
            `ALTER TABLE ${tr} RENAME COLUMN ${q(col.originalName!, driver)} TO ${q(col.name, driver)};`,
          );
        }
        // After a rename use the new name for subsequent ops
        const colName = renamed ? col.name : col.originalName!;
        if (typeChanged) {
          statements.push(
            `ALTER TABLE ${tr} ALTER COLUMN ${q(colName, driver)} TYPE ${newType};`,
          );
        }
        if (notNullChanged) {
          statements.push(
            col.notNull
              ? `ALTER TABLE ${tr} ALTER COLUMN ${q(colName, driver)} SET NOT NULL;`
              : `ALTER TABLE ${tr} ALTER COLUMN ${q(colName, driver)} DROP NOT NULL;`,
          );
        }
        if (defaultChanged) {
          statements.push(
            col.defaultValue.trim()
              ? `ALTER TABLE ${tr} ALTER COLUMN ${q(colName, driver)} SET DEFAULT ${formatDefault(col.defaultValue, col.dataType)};`
              : `ALTER TABLE ${tr} ALTER COLUMN ${q(colName, driver)} DROP DEFAULT;`,
          );
        }
        if (commentChanged && driver === "postgres") {
          const commentVal = col.comment.trim()
            ? `'${col.comment.replace(/'/g, "''")}'`
            : "NULL";
          statements.push(
            `COMMENT ON COLUMN ${tr}.${q(colName, driver)} IS ${commentVal};`,
          );
        }
        break;
      }

      case "mssql": {
        if (renamed) {
          const schemaPrefix = schema.trim() ? `${schema}.` : "dbo.";
          statements.push(
            `EXEC sp_rename N'${schemaPrefix}${table}.${col.originalName}', N'${col.name}', 'COLUMN';`,
          );
        }
        const colName = renamed ? col.name : col.originalName!;
        if (typeChanged || notNullChanged) {
          const nullPart = col.notNull ? "NOT NULL" : "NULL";
          statements.push(
            `ALTER TABLE ${tr} ALTER COLUMN ${q(colName, driver)} ${newType} ${nullPart};`,
          );
        }
        if (defaultChanged) {
          unsupportedOps.push(
            `DEFAULT change for "${col.originalName}" — requires dropping and re-adding a named DEFAULT constraint in MSSQL`,
          );
        }
        break;
      }

      case "clickhouse": {
        if (renamed) {
          statements.push(
            `ALTER TABLE ${tr} RENAME COLUMN ${q(col.originalName!, driver)} TO ${q(col.name, driver)};`,
          );
        }
        if (hasPropertyChange) {
          const colName = renamed ? col.name : col.originalName!;
          statements.push(
            `ALTER TABLE ${tr} MODIFY COLUMN ${buildColDef({ ...col, name: colName }, driver, false)};`,
          );
        }
        break;
      }

      case "oracle": {
        if (renamed) {
          statements.push(
            `ALTER TABLE ${tr} RENAME COLUMN ${q(col.originalName!, driver)} TO ${q(col.name, driver)};`,
          );
        }
        if (hasPropertyChange) {
          const colName = renamed ? col.name : col.originalName!;
          statements.push(
            `ALTER TABLE ${tr} MODIFY (${buildColDef({ ...col, name: colName }, driver, false)});`,
          );
        }
        break;
      }
    }
  }

  return {
    sql: statements.join("\n"),
    unsupportedOps,
  };
}
