import type { DbDriver } from "./createTable";
import type { IndexDef } from "./manageIndexes";
import { isTextBlobType } from "./ddlUtils";

type T = (key: string, options?: Record<string, unknown>) => string;

interface ColLike {
  name: string;
  dataType: string;
  length: string;
  autoIncrement: boolean;
  primaryKey: boolean;
}

export function validateColumns(
  filledCols: ColLike[],
  opts: { driver: DbDriver; showAutoIncrement: boolean; t: T },
): string[] {
  const { driver, showAutoIncrement, t } = opts;
  const errs: string[] = [];

  filledCols.forEach((col, i) => {
    if (!col.name.trim())
      errs.push(t("createTable.validation.columnNameRequired", { index: i + 1 }));
    if (!col.dataType.trim())
      errs.push(t("createTable.validation.columnTypeRequired", { index: i + 1 }));
  });

  const names = filledCols.map((c) => c.name.trim().toLowerCase());
  names.forEach((name, i) => {
    if (name && names.indexOf(name) !== i)
      errs.push(t("createTable.validation.duplicateColumnName", { name: filledCols[i].name.trim() }));
  });

  if (showAutoIncrement) {
    const aiCols = filledCols.filter((c) => c.autoIncrement);
    if (aiCols.length > 1) errs.push(t("createTable.validation.multipleAutoIncrement"));
    aiCols.forEach((col) => {
      if (!col.primaryKey)
        errs.push(t("createTable.validation.autoIncrementNeedsKey", { name: col.name.trim() }));
    });
  }

  const needsLengthCheck =
    driver === "mysql" || driver === "mariadb" || driver === "tidb" || driver === "mssql";
  if (needsLengthCheck) {
    filledCols.forEach((col) => {
      const base = col.dataType.trim().toUpperCase().split("(")[0];
      if (base === "VARCHAR" || base === "CHAR") {
        if (!col.length.trim())
          errs.push(t("createTable.validation.varcharNeedsLength", { name: col.name.trim() }));
        else {
          const len = parseInt(col.length.trim(), 10);
          if (!isNaN(len) && len <= 0)
            errs.push(t("createTable.validation.varcharZeroLength", { name: col.name.trim() }));
        }
      }
    });
  }

  filledCols.forEach((col) => {
    const base = col.dataType.trim().toUpperCase().split("(")[0];
    if (base === "DECIMAL" || base === "NUMERIC") {
      const parts = col.length.trim().split(",");
      if (parts.length === 2) {
        const precision = parseInt(parts[0], 10);
        const scale = parseInt(parts[1], 10);
        if (!isNaN(precision) && !isNaN(scale) && scale > precision)
          errs.push(t("createTable.validation.decimalScaleExceedsPrecision", { name: col.name.trim() }));
      }
    }
  });

  return errs;
}

export function validateIndexDefs(
  defs: IndexDef[],
  colTypeMap: Map<string, string>,
  opts: { driver: DbDriver; t: T },
): string[] {
  const { driver, t } = opts;
  const errs: string[] = [];

  const needsPrefixCheck = driver === "mysql" || driver === "mariadb" || driver === "tidb";
  if (needsPrefixCheck) {
    defs.forEach((def) => {
      def.columns.forEach((colName) => {
        const type = colTypeMap.get(colName) ?? "";
        if (isTextBlobType(type))
          errs.push(t("createTable.validation.indexTextColumn", { col: colName }));
      });
    });
  }

  defs.forEach((def) => {
    const seen = new Set<string>();
    def.columns.forEach((col) => {
      if (seen.has(col))
        errs.push(t("createTable.validation.indexDuplicateColumn", { col, index: def.name || "?" }));
      seen.add(col);
    });
  });

  return errs;
}
