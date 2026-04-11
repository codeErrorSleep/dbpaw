import { isMysqlFamilyDriver } from "@/lib/driver-registry";

export interface SearchMatch {
  row: number;
  col: string;
  colIndex: number;
}

export interface InsertColumnMeta {
  name: string;
  type: string;
  nullable: boolean;
  defaultValue?: string | null;
  primaryKey?: boolean;
}

export interface HeaderInteractionState {
  timerId: ReturnType<typeof setTimeout> | null;
}

export function createSingleAndDoubleClickHandler(
  state: HeaderInteractionState,
  onSingleClick: () => void,
  onDoubleClick: () => void,
  delayMs = 250,
) {
  return {
    handleClick() {
      if (state.timerId) {
        clearTimeout(state.timerId);
      }
      state.timerId = setTimeout(() => {
        state.timerId = null;
        onSingleClick();
      }, delayMs);
    },
    handleDoubleClick() {
      if (state.timerId) {
        clearTimeout(state.timerId);
        state.timerId = null;
      }
      onDoubleClick();
    },
    cancelPendingClick() {
      if (state.timerId) {
        clearTimeout(state.timerId);
        state.timerId = null;
      }
    },
  };
}

export function isInsertColumnRequired(
  column: Pick<InsertColumnMeta, "nullable" | "defaultValue">,
): boolean {
  return !column.nullable && !(column.defaultValue || "").trim();
}

interface AutoColumnWidthParams {
  data: any[];
  columns: string[];
  columnWidths: Record<string, number>;
}

export function calculateAutoColumnWidths({
  data,
  columns,
  columnWidths,
}: AutoColumnWidthParams): Record<string, number> {
  if (!data.length || !columns.length) {
    return {};
  }

  const newWidths: Record<string, number> = {};

  const DATA_CHAR_WIDTH = 9;
  const DATA_PADDING = 36;
  const HEADER_CHAR_WIDTH = 9;
  const HEADER_PADDING = 56;
  const MIN_WIDTH = columns.length <= 3 ? 250 : 100;
  const MAX_WIDTH = 900;

  columns.forEach((col) => {
    if (columnWidths[col] !== undefined) return;

    let sampledMaxLen = 0;
    const sampleSize = Math.min(data.length, 20);

    for (let i = 0; i < sampleSize; i++) {
      const val = data[i][col];
      if (val !== null && val !== undefined) {
        const str = formatCellValue(val);
        const len = str.length > 100 ? 100 : str.length;
        if (len > sampledMaxLen) sampledMaxLen = len;
      }
    }

    const headerRequiredWidth = col.length * HEADER_CHAR_WIDTH + HEADER_PADDING;
    const sampledDataWidth = sampledMaxLen * DATA_CHAR_WIDTH + DATA_PADDING;
    const calculatedWidth = Math.min(
      MAX_WIDTH,
      Math.max(MIN_WIDTH, headerRequiredWidth, sampledDataWidth),
    );

    newWidths[col] = calculatedWidth;
  });

  return newWidths;
}

export function sortRows<T extends Record<string, any>>(
  data: T[],
  sortColumn?: string,
  sortDirection?: "asc" | "desc",
): T[] {
  if (!sortColumn || !sortDirection) {
    return data;
  }

  const col = sortColumn;
  const dir = sortDirection;

  return [...data].sort((a, b) => {
    const va = a[col];
    const vb = b[col];

    if (va == null && vb == null) return 0;
    if (va == null) return 1;
    if (vb == null) return -1;

    const numA = Number(va);
    const numB = Number(vb);
    if (!isNaN(numA) && !isNaN(numB)) {
      return dir === "asc" ? numA - numB : numB - numA;
    }

    const strA = String(va);
    const strB = String(vb);
    const cmp = strA.localeCompare(strB);
    return dir === "asc" ? cmp : -cmp;
  });
}

export function collectSearchMatches(
  currentData: any[],
  columns: string[],
  normalizedSearchKeyword: string,
  getCellDisplayValue: (
    rowIndex: number,
    column: string,
    originalValue: any,
  ) => any,
): SearchMatch[] {
  if (!normalizedSearchKeyword) {
    return [];
  }

  const matches: SearchMatch[] = [];
  currentData.forEach((row, rowIndex) => {
    columns.forEach((column, colIndex) => {
      const value = getCellDisplayValue(rowIndex, column, row[column]);
      if (value === null || value === undefined) return;
      const content = formatCellValue(value).toLowerCase();
      if (content.includes(normalizedSearchKeyword)) {
        matches.push({ row: rowIndex, col: column, colIndex });
      }
    });
  });

  return matches;
}

export function escapeSQL(value: string): string {
  return value.replace(/'/g, "''");
}

export function quoteIdent(driver: string | undefined, name: string): string {
  if (
    (driver && isMysqlFamilyDriver(driver as any)) ||
    driver === "clickhouse"
  ) {
    return `\`${name}\``;
  }
  if (driver === "mssql") {
    return `[${name.replace(/]/g, "]]")}]`;
  }
  return `"${name}"`;
}

export function formatSQLValue(
  value: string,
  originalValue: any,
  context: "execution" | "copy" = "execution",
  driver?: string,
): string {
  if (value === "" && (originalValue === null || originalValue === undefined)) {
    return "NULL";
  }

  const trimmed = value.trim();
  const numericRegex = /^-?\d+(\.\d+)?$/;

  if (typeof originalValue === "number") {
    if (numericRegex.test(trimmed)) {
      return trimmed;
    }
    if (context === "execution") {
      throw new Error(`Invalid numeric value: "${value}"`);
    }
  } else if (!isNaN(Number(value)) && trimmed !== "") {
    if (numericRegex.test(trimmed)) {
      return trimmed;
    }
  }

  if (typeof originalValue === "boolean") {
    const lower = value.toLowerCase();
    if (["true", "t", "1"].includes(lower)) {
      return driver === "mssql" ? "1" : "TRUE";
    }
    if (["false", "f", "0"].includes(lower)) {
      return driver === "mssql" ? "0" : "FALSE";
    }

    if (context === "execution") {
      throw new Error(`Invalid boolean value: "${value}"`);
    }
  }

  return `'${escapeSQL(value)}'`;
}

function isBooleanType(type: string): boolean {
  return /\b(bool|boolean|bit)\b/.test(type.toLowerCase());
}

function isNumericType(type: string): boolean {
  return /\b(tinyint|smallint|mediumint|int|integer|bigint|serial|bigserial|decimal|numeric|real|double|float|money|number)\b/.test(
    type.toLowerCase(),
  );
}

export function formatInsertSQLValue(
  raw: string,
  column: Pick<InsertColumnMeta, "name" | "type">,
  driver?: string,
): string {
  const value = String(raw);
  const trimmed = value.trim();
  if (trimmed.toUpperCase() === "NULL") {
    return "NULL";
  }

  if (isBooleanType(column.type)) {
    const lower = trimmed.toLowerCase();
    if (["true", "t", "1"].includes(lower)) {
      return driver === "mssql" ? "1" : "TRUE";
    }
    if (["false", "f", "0"].includes(lower)) {
      return driver === "mssql" ? "0" : "FALSE";
    }
    throw new Error(
      `Invalid boolean value for column "${column.name}": "${value}"`,
    );
  }

  if (isNumericType(column.type)) {
    const numericRegex = /^-?\d+(\.\d+)?$/;
    if (numericRegex.test(trimmed)) {
      return trimmed;
    }
    throw new Error(
      `Invalid numeric value for column "${column.name}": "${value}"`,
    );
  }

  return `'${escapeSQL(value)}'`;
}

export function getQualifiedTableName(
  driver: string,
  schema: string,
  table: string,
): string {
  if (isMysqlFamilyDriver(driver as any)) {
    return quoteIdent(driver, table);
  }

  if (driver === "sqlite" || driver === "duckdb") {
    const normalizedSchema = schema.trim().toLowerCase();
    if (
      normalizedSchema === "" ||
      normalizedSchema === "main" ||
      normalizedSchema === "public"
    ) {
      return quoteIdent(driver, table);
    }
  }

  return `${quoteIdent(driver, schema)}.${quoteIdent(driver, table)}`;
}

export function isComplexValue(value: unknown): boolean {
  return value !== null && value !== undefined && typeof value === "object";
}

/**
 * Converts a cell value to its full-fidelity string representation.
 * Used for editing, clipboard copy, and CSV/TSV export — anywhere the
 * complete value is needed rather than an abbreviated display summary.
 * Objects and arrays are serialized as JSON; primitives use String().
 */
export function cellValueToString(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

export function formatCellValue(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value !== "object") return String(value);
  if (Array.isArray(value)) {
    return JSON.stringify(value);
  }
  const keys = Object.keys(value as object);
  if (keys.length === 0) return "{}";
  if (keys.length <= 2) return JSON.stringify(value);
  return `{${keys.slice(0, 2).join(", ")}, ... +${keys.length - 2}}`;
}

export function isClickHouseMergeTreeEngine(
  engine: string | undefined | null,
): boolean {
  if (!engine) return false;
  return engine.toLowerCase().includes("mergetree");
}

export function canMutateClickHouseTable(
  engine: string | undefined | null,
  primaryKeys: string[],
): boolean {
  return isClickHouseMergeTreeEngine(engine) && primaryKeys.length > 0;
}

export function buildUpdateStatement(
  driver: string,
  tableName: string,
  setClause: string,
  whereClause: string,
): string {
  if (driver === "clickhouse") {
    return `ALTER TABLE ${tableName} UPDATE ${setClause} WHERE ${whereClause}`;
  }
  return `UPDATE ${tableName} SET ${setClause} WHERE ${whereClause}`;
}

export function buildDeleteStatement(
  driver: string,
  tableName: string,
  whereClause: string,
): string {
  if (driver === "clickhouse") {
    return `ALTER TABLE ${tableName} DELETE WHERE ${whereClause}`;
  }
  return `DELETE FROM ${tableName} WHERE ${whereClause}`;
}
