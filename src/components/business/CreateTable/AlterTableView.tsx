import { useState, useMemo, useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  Copy,
  GripVertical,
  Plus,
  Trash2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { api, ColumnInfo, IndexInfo } from "@/services/api";
import {
  DbDriver,
  TYPE_PRESETS,
  supportsAutoIncrement,
} from "@/lib/sql-gen/createTable";
import {
  AlterColumnDef,
  columnInfoToAlterDef,
  generateAlterTableSQL,
} from "@/lib/sql-gen/alterTable";
import {
  IndexDef,
  generateManageIndexSQL,
  indexInfoToIndexDef,
  newIndexId,
  supportsIndexManagement,
} from "@/lib/sql-gen/manageIndexes";
import {
  CUSTOM_TYPE_SENTINEL,
  columnGridTemplate,
  splitSqlStatements,
} from "@/lib/sql-gen/ddlUtils";
import { validateColumns, validateIndexDefs } from "@/lib/sql-gen/tableValidation";
import { IndexEditorSection } from "./IndexEditorSection";

// ─── helpers ──────────────────────────────────────────────────────────────────

let _colIdCounter = 0;
function newColumnId() {
  return `col-${++_colIdCounter}-${Date.now()}`;
}

function defaultColumn(): AlterColumnDef {
  return {
    id: newColumnId(),
    name: "",
    dataType: "",
    length: "",
    notNull: false,
    primaryKey: false,
    autoIncrement: false,
    defaultValue: "",
    comment: "",
    originalName: null,
  };
}

// ─── props ────────────────────────────────────────────────────────────────────

interface AlterTableViewProps {
  connectionId: number;
  database: string;
  schema: string;
  table: string;
  driver: string;
  onSuccess: () => void;
  onCancel: () => void;
}

// ─── component ────────────────────────────────────────────────────────────────

export function AlterTableView({
  connectionId,
  database,
  schema,
  table,
  driver,
  onSuccess,
  onCancel,
}: AlterTableViewProps) {
  const { t } = useTranslation();

  const [columns, setColumns] = useState<AlterColumnDef[]>([]);
  const [originalCols, setOriginalCols] = useState<ColumnInfo[]>([]);
  const [indexDefs, setIndexDefs] = useState<IndexDef[]>([]);
  const [originalIndexes, setOriginalIndexes] = useState<IndexInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [sqlPreviewOpen, setSqlPreviewOpen] = useState(true);
  const [isExecuting, setIsExecuting] = useState(false);
  const [errors, setErrors] = useState<string[]>([]);
  const [copiedSql, setCopiedSql] = useState(false);

  const [customTypes, setCustomTypes] = useState<Record<string, string>>({});

  const dbDriver = driver as DbDriver;
  const typePresets = TYPE_PRESETS[dbDriver] ?? TYPE_PRESETS["postgres"];
  const showAutoIncrement = supportsAutoIncrement(dbDriver);
  const indexSupported = supportsIndexManagement(dbDriver);

  // ── load existing table metadata ─────────────────────────────────────────

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    api.metadata
      .getTableMetadata(connectionId, database, schema, table)
      .then((meta) => {
        if (cancelled) return;
        setOriginalCols(meta.columns);
        setColumns(meta.columns.map(columnInfoToAlterDef));
        const idxs = meta.indexes ?? [];
        setOriginalIndexes(idxs);
        setIndexDefs(idxs.map(indexInfoToIndexDef));
      })
      .catch(() => {
        if (cancelled) return;
        toast.error(t("alterTable.toast.loadError"));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connectionId, database, schema, table]);

  // ── derived SQL ─────────────────────────────────────────────────────────────

  const { sql: generatedSQL, unsupportedOps } = useMemo(() => {
    if (loading || originalCols.length === 0) return { sql: "", unsupportedOps: [] };
    const colResult = generateAlterTableSQL(schema, table, originalCols, columns, dbDriver);
    const idxResult = indexSupported
      ? generateManageIndexSQL(schema, table, originalIndexes, indexDefs, dbDriver)
      : { sql: "", statements: [] };
    const parts = [colResult.sql, idxResult.sql].filter(Boolean);
    return { sql: parts.join("\n"), unsupportedOps: colResult.unsupportedOps };
  }, [loading, schema, table, originalCols, columns, dbDriver, indexSupported, originalIndexes, indexDefs]);

  // ── validation ──────────────────────────────────────────────────────────────

  const validate = useCallback(() => {
    const filledCols = columns.filter((c) => c.name.trim() || c.dataType.trim());
    const colTypeMap = new Map(originalCols.map((c) => [c.name, c.type]));
    return [
      ...validateColumns(filledCols, { driver: dbDriver, showAutoIncrement, t }),
      ...validateIndexDefs(indexDefs, colTypeMap, { driver: dbDriver, t }),
    ];
  }, [columns, t, showAutoIncrement, dbDriver, indexDefs, originalCols]);

  // ── column mutations ─────────────────────────────────────────────────────────

  const addColumn = () => setColumns((prev) => [...prev, defaultColumn()]);

  const removeColumn = (id: string) => {
    setColumns((prev) => prev.filter((c) => c.id !== id));
    setCustomTypes((prev) => {
      const next = { ...prev };
      delete next[id];
      return next;
    });
  };

  const updateColumn = (id: string, patch: Partial<AlterColumnDef>) =>
    setColumns((prev) => prev.map((c) => (c.id === id ? { ...c, ...patch } : c)));

  const moveColumn = (id: string, direction: -1 | 1) => {
    setColumns((prev) => {
      const idx = prev.findIndex((c) => c.id === id);
      if (idx === -1) return prev;
      const next = idx + direction;
      if (next < 0 || next >= prev.length) return prev;
      const arr = [...prev];
      [arr[idx], arr[next]] = [arr[next], arr[idx]];
      return arr;
    });
  };

  // ── index mutations ──────────────────────────────────────────────────────────

  const tableColumnNames = originalCols.map((c) => c.name);

  const addIndexDef = () =>
    setIndexDefs((prev) => [
      ...prev,
      { id: newIndexId(), originalName: null, name: "", unique: false, columns: [], indexMethod: "", clustered: false, concurrently: false },
    ]);

  const removeIndexDef = (id: string) =>
    setIndexDefs((prev) => prev.filter((d) => d.id !== id));

  const updateIndexDef = (id: string, patch: Partial<IndexDef>) =>
    setIndexDefs((prev) => prev.map((d) => (d.id === id ? { ...d, ...patch } : d)));

  const toggleIndexColumn = (defId: string, colName: string) =>
    setIndexDefs((prev) =>
      prev.map((d) => {
        if (d.id !== defId) return d;
        const cols = d.columns.includes(colName)
          ? d.columns.filter((c) => c !== colName)
          : [...d.columns, colName];
        return { ...d, columns: cols };
      }),
    );

  // ── execute ──────────────────────────────────────────────────────────────────

  const handleExecute = async () => {
    if (!generatedSQL.trim()) return;
    const errs = validate();
    if (errs.length > 0) {
      setErrors(errs);
      return;
    }
    setErrors([]);
    setIsExecuting(true);
    try {
      for (const stmt of splitSqlStatements(generatedSQL)) {
        await api.query.execute(connectionId, stmt, database, "sql_editor");
      }
      toast.success(t("alterTable.toast.success", { table }));
      onSuccess();
    } catch (e) {
      toast.error(t("alterTable.toast.error"), {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsExecuting(false);
    }
  };

  // ── copy SQL ─────────────────────────────────────────────────────────────────

  const handleCopySql = () => {
    void navigator.clipboard.writeText(generatedSQL).then(() => {
      setCopiedSql(true);
      setTimeout(() => setCopiedSql(false), 2000);
    });
  };

  // ── render ───────────────────────────────────────────────────────────────────

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center text-sm text-muted-foreground">
        {t("common.loading")}
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Scrollable body */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* Table name (read-only) */}
        <div className="space-y-1">
          <label className="text-sm font-medium">{t("alterTable.form.tableName")}</label>
          <div className="flex items-center gap-3">
            <Input className="max-w-xs font-mono bg-muted/40" value={table} readOnly />
            {schema && (
              <span className="text-xs text-muted-foreground font-mono">
                {schema}.{table}
              </span>
            )}
          </div>
        </div>

        {/* Unsupported operations warning */}
        {unsupportedOps.length > 0 && (
          <div className="rounded-md border border-yellow-500/40 bg-yellow-500/5 px-3 py-2 space-y-1">
            <div className="flex items-center gap-1.5 text-xs font-medium text-yellow-700 dark:text-yellow-400">
              <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
              {t("alterTable.unsupported.title")}
            </div>
            {unsupportedOps.map((op, i) => (
              <p key={i} className="text-xs text-yellow-700/80 dark:text-yellow-400/80 pl-5">
                {op}
              </p>
            ))}
          </div>
        )}

        {/* Column editor */}
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium">{t("createTable.form.columns")}</span>
            <Button size="sm" variant="outline" onClick={addColumn}>
              <Plus className="w-3.5 h-3.5 mr-1" />
              {t("createTable.form.addColumn")}
            </Button>
          </div>

          {columns.length === 0 ? (
            <div className="border border-dashed rounded-md py-8 text-center text-sm text-muted-foreground">
              {t("createTable.form.noColumns")}
            </div>
          ) : (
            <div className="border rounded-md overflow-hidden">
              {/* Header row */}
              <div
                className="grid bg-muted/50 border-b text-xs font-medium text-muted-foreground px-2 py-1.5 gap-1"
                style={{ gridTemplateColumns: columnGridTemplate(showAutoIncrement) }}
              >
                <div className="w-5" />
                <div>{t("createTable.form.columnName")}</div>
                <div>{t("createTable.form.columnType")}</div>
                <div>{t("createTable.form.columnLength")}</div>
                <div className="text-center">{t("createTable.form.columnNotNull")}</div>
                <div className="text-center">{t("createTable.form.columnPrimaryKey")}</div>
                {showAutoIncrement && (
                  <div className="text-center">{t("createTable.form.columnAutoIncrement")}</div>
                )}
                <div>{t("createTable.form.columnDefault")}</div>
                <div>{t("createTable.form.columnComment")}</div>
                <div className="w-16" />
              </div>

              {/* Data rows */}
              {columns.map((col, idx) => {
                const customType = customTypes[col.id] ?? "";
                const isCustom =
                  col.dataType === CUSTOM_TYPE_SENTINEL ||
                  (col.dataType !== "" &&
                    !typePresets.includes(col.dataType) &&
                    col.dataType !== CUSTOM_TYPE_SENTINEL);
                const selectValue = isCustom ? CUSTOM_TYPE_SENTINEL : col.dataType || "";
                const isExisting = col.originalName !== null;

                return (
                  <div
                    key={col.id}
                    className={`grid items-center px-2 py-1 gap-1 border-b last:border-b-0 hover:bg-muted/20 ${
                      isExisting ? "" : "bg-green-500/5"
                    }`}
                    style={{ gridTemplateColumns: columnGridTemplate(showAutoIncrement) }}
                  >
                    <div className="flex flex-col gap-0.5 items-center">
                      <button
                        className="text-muted-foreground hover:text-foreground disabled:opacity-30"
                        disabled={idx === 0}
                        onClick={() => moveColumn(col.id, -1)}
                        tabIndex={-1}
                        title="Move up"
                      >
                        <GripVertical className="w-3.5 h-3.5" style={{ marginBottom: -2 }} />
                      </button>
                    </div>

                    <Input
                      className="h-7 text-xs px-2 font-mono"
                      value={col.name}
                      onChange={(e) => updateColumn(col.id, { name: e.target.value })}
                      placeholder={t("createTable.form.columnName")}
                    />

                    <div className="flex gap-1">
                      <Select
                        value={selectValue}
                        onValueChange={(val) => {
                          if (val === CUSTOM_TYPE_SENTINEL) {
                            updateColumn(col.id, { dataType: customType || CUSTOM_TYPE_SENTINEL });
                          } else {
                            updateColumn(col.id, { dataType: val, length: "" });
                            setCustomTypes((prev) => {
                              const next = { ...prev };
                              delete next[col.id];
                              return next;
                            });
                          }
                        }}
                      >
                        <SelectTrigger className="h-7 text-xs px-2 font-mono min-w-0 w-full">
                          <SelectValue placeholder={t("createTable.form.columnType")} />
                        </SelectTrigger>
                        <SelectContent>
                          {typePresets.map((tp) => (
                            <SelectItem key={tp} value={tp} className="text-xs font-mono">
                              {tp}
                            </SelectItem>
                          ))}
                          <SelectItem value={CUSTOM_TYPE_SENTINEL} className="text-xs">
                            Other…
                          </SelectItem>
                        </SelectContent>
                      </Select>
                      {isCustom && (
                        <Input
                          className="h-7 text-xs px-2 font-mono w-28 shrink-0"
                          value={col.dataType === CUSTOM_TYPE_SENTINEL ? customType : col.dataType}
                          onChange={(e) => {
                            const val = e.target.value;
                            setCustomTypes((prev) => ({ ...prev, [col.id]: val }));
                            updateColumn(col.id, { dataType: val || CUSTOM_TYPE_SENTINEL });
                          }}
                          placeholder="custom type"
                          autoFocus
                        />
                      )}
                    </div>

                    <Input
                      className="h-7 text-xs px-2 font-mono"
                      value={col.length}
                      onChange={(e) => updateColumn(col.id, { length: e.target.value })}
                      placeholder="—"
                    />

                    <div className="flex justify-center">
                      <Checkbox
                        checked={col.notNull}
                        onCheckedChange={(v) => updateColumn(col.id, { notNull: !!v })}
                      />
                    </div>

                    <div className="flex justify-center">
                      <Checkbox
                        checked={col.primaryKey}
                        onCheckedChange={(v) => updateColumn(col.id, { primaryKey: !!v })}
                      />
                    </div>

                    {showAutoIncrement && (
                      <div className="flex justify-center">
                        <Checkbox
                          checked={col.autoIncrement}
                          onCheckedChange={(v) => updateColumn(col.id, { autoIncrement: !!v })}
                        />
                      </div>
                    )}

                    <Input
                      className="h-7 text-xs px-2 font-mono"
                      value={col.defaultValue}
                      onChange={(e) => updateColumn(col.id, { defaultValue: e.target.value })}
                      placeholder="—"
                    />

                    <Input
                      className="h-7 text-xs px-2"
                      value={col.comment}
                      onChange={(e) => updateColumn(col.id, { comment: e.target.value })}
                      placeholder="—"
                    />

                    <div className="flex items-center justify-end gap-1">
                      <button
                        className="text-muted-foreground hover:text-foreground disabled:opacity-30"
                        disabled={idx === columns.length - 1}
                        onClick={() => moveColumn(col.id, 1)}
                        tabIndex={-1}
                        title={t("createTable.form.remove")}
                      >
                        <GripVertical className="w-3.5 h-3.5 rotate-180" />
                      </button>
                      <button
                        className="text-muted-foreground hover:text-destructive"
                        onClick={() => removeColumn(col.id)}
                        tabIndex={-1}
                        title={t("createTable.form.remove")}
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Index editor */}
        <IndexEditorSection
          defs={indexDefs}
          tableColumns={tableColumnNames}
          driver={dbDriver}
          highlightNew
          onAdd={addIndexDef}
          onRemove={removeIndexDef}
          onUpdate={updateIndexDef}
          onToggleColumn={toggleIndexColumn}
        />

        {/* Validation errors */}
        {errors.length > 0 && (
          <div className="rounded-md border border-destructive/40 bg-destructive/5 px-3 py-2 space-y-0.5">
            {errors.map((err, i) => (
              <p key={i} className="text-xs text-destructive">
                {err}
              </p>
            ))}
          </div>
        )}

        {/* SQL Preview */}
        <Collapsible open={sqlPreviewOpen} onOpenChange={setSqlPreviewOpen}>
          <div className="border rounded-md overflow-hidden">
            <CollapsibleTrigger asChild>
              <button className="w-full flex items-center justify-between px-3 py-2 bg-muted/40 hover:bg-muted/70 text-sm font-medium transition-colors">
                <span>{t("createTable.sqlPreview.title")}</span>
                <div className="flex items-center gap-2">
                  {sqlPreviewOpen ? (
                    <ChevronDown className="w-3.5 h-3.5 text-muted-foreground" />
                  ) : (
                    <ChevronRight className="w-3.5 h-3.5 text-muted-foreground" />
                  )}
                </div>
              </button>
            </CollapsibleTrigger>
            <CollapsibleContent>
              <div className="relative">
                <pre className="p-3 text-xs font-mono overflow-x-auto bg-background text-foreground leading-relaxed min-h-16">
                  {generatedSQL || t("alterTable.sqlPreview.noChanges")}
                </pre>
                {generatedSQL && (
                  <button
                    className="absolute top-2 right-2 flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground bg-muted/80 hover:bg-muted px-2 py-1 rounded transition-colors"
                    onClick={handleCopySql}
                  >
                    <Copy className="w-3 h-3" />
                    {copiedSql ? t("createTable.sqlPreview.copied") : t("createTable.sqlPreview.copy")}
                  </button>
                )}
              </div>
            </CollapsibleContent>
          </div>
        </Collapsible>
      </div>

      {/* Sticky action bar */}
      <div className="border-t bg-background px-4 py-3 flex items-center justify-between gap-2 shrink-0">
        <div className="text-xs text-muted-foreground font-mono">
          {database}
          {schema ? `.${schema}` : ""}.{table}
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={onCancel} disabled={isExecuting}>
            {t("createTable.actions.cancel")}
          </Button>
          <Button onClick={handleExecute} disabled={isExecuting || !generatedSQL.trim()}>
            {isExecuting ? t("alterTable.actions.executing") : t("alterTable.actions.execute")}
          </Button>
        </div>
      </div>
    </div>
  );
}
