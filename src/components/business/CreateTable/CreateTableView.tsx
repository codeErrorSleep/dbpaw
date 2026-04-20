import { useState, useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
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
import { api } from "@/services/api";
import {
  ColumnDef,
  DbDriver,
  StarRocksDistribution,
  TYPE_PRESETS,
  generateCreateTableSQL,
  supportsAutoIncrement,
} from "@/lib/sql-gen/createTable";
import {
  IndexDef,
  generateManageIndexSQL,
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

function defaultColumn(): ColumnDef {
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
  };
}

// ─── props ────────────────────────────────────────────────────────────────────

interface CreateTableViewProps {
  connectionId: number;
  database: string;
  schema: string;
  driver: string;
  onSuccess: (tableName: string) => void;
  onCancel: () => void;
}

// ─── component ────────────────────────────────────────────────────────────────

export function CreateTableView({
  connectionId,
  database,
  schema,
  driver,
  onSuccess,
  onCancel,
}: CreateTableViewProps) {
  const { t } = useTranslation();

  const [tableName, setTableName] = useState("");
  const [columns, setColumns] = useState<ColumnDef[]>([defaultColumn()]);
  const [sqlPreviewOpen, setSqlPreviewOpen] = useState(true);
  const [isExecuting, setIsExecuting] = useState(false);
  const [errors, setErrors] = useState<string[]>([]);
  const [copiedSql, setCopiedSql] = useState(false);

  // Custom-type tracking: columnId → custom type string
  const [customTypes, setCustomTypes] = useState<Record<string, string>>({});

  // StarRocks distribution config
  const [srDistType, setSrDistType] = useState<"hash" | "random">("hash");
  const [srDistColumns, setSrDistColumns] = useState<string[]>([]);
  const [srBuckets, setSrBuckets] = useState("10");

  const [indexDefs, setIndexDefs] = useState<IndexDef[]>([]);

  const dbDriver = driver as DbDriver;
  const isStarRocks = dbDriver === "starrocks";
  const typePresets = TYPE_PRESETS[dbDriver] ?? TYPE_PRESETS["postgres"];
  const indexSupported = supportsIndexManagement(dbDriver);
  const showAutoIncrement = supportsAutoIncrement(dbDriver);

  // Named columns for StarRocks distribution and index column picker
  const namedColumns = columns.map((c) => c.name.trim()).filter(Boolean);

  // ── derived SQL ─────────────────────────────────────────────────────────────

  const starrocksDistribution: StarRocksDistribution | undefined = isStarRocks
    ? { type: srDistType, columns: srDistColumns, buckets: srBuckets }
    : undefined;

  const generatedSQL = useMemo(() => {
    const createSQL = generateCreateTableSQL(
      { tableName, schema, columns, starrocksDistribution },
      dbDriver,
    );
    if (!createSQL || !indexSupported) return createSQL;
    const { sql: idxSQL } = generateManageIndexSQL(
      schema,
      tableName.trim(),
      [],
      indexDefs,
      dbDriver,
    );
    return [createSQL, idxSQL].filter(Boolean).join("\n");
    // starrocksDistribution is derived from srDistType/srDistColumns/srBuckets
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    tableName,
    schema,
    columns,
    dbDriver,
    srDistType,
    srDistColumns,
    srBuckets,
    indexDefs,
    indexSupported,
  ]);

  // ── validation ──────────────────────────────────────────────────────────────

  const validate = useCallback(() => {
    const errs: string[] = [];

    if (!tableName.trim()) errs.push(t("createTable.validation.tableNameRequired"));

    const filledCols = columns.filter((c) => c.name.trim() || c.dataType.trim());
    if (filledCols.length === 0) errs.push(t("createTable.validation.noColumns"));

    errs.push(...validateColumns(filledCols, { driver: dbDriver, showAutoIncrement, t }));

    if (isStarRocks && srDistType === "hash" && srDistColumns.length === 0)
      errs.push(t("createTable.validation.starrocksHashColumnsRequired"));

    const colTypeMap = new Map(filledCols.map((c) => [c.name.trim(), c.dataType]));
    errs.push(...validateIndexDefs(indexDefs, colTypeMap, { driver: dbDriver, t }));

    return errs;
  }, [tableName, columns, t, isStarRocks, srDistType, srDistColumns, showAutoIncrement, dbDriver, indexDefs]);

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

  const updateColumn = (id: string, patch: Partial<ColumnDef>) =>
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
      toast.success(t("createTable.toast.success", { table: tableName.trim() }));
      onSuccess(tableName.trim());
    } catch (e) {
      toast.error(t("createTable.toast.error"), {
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

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Scrollable body */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* Table name */}
        <div className="space-y-1">
          <label className="text-sm font-medium">
            {t("createTable.form.tableName")}
            <span className="text-destructive ml-0.5">*</span>
          </label>
          <div className="flex items-center gap-3">
            <Input
              className="max-w-xs"
              placeholder={t("createTable.form.tableNamePlaceholder")}
              value={tableName}
              onChange={(e) => setTableName(e.target.value)}
              autoFocus
            />
            {schema && (
              <span className="text-xs text-muted-foreground font-mono">
                {schema}.{tableName || "…"}
              </span>
            )}
          </div>
        </div>

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

                return (
                  <div
                    key={col.id}
                    className="grid items-center px-2 py-1 gap-1 border-b last:border-b-0 hover:bg-muted/20"
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

        {/* StarRocks distribution config */}
        {isStarRocks && (
          <div className="space-y-2">
            <span className="text-sm font-medium">
              {t("createTable.starrocks.distributionTitle")}
            </span>
            <div className="border rounded-md p-3 space-y-3">
              <div className="flex items-center gap-4">
                <span className="text-xs text-muted-foreground w-28 shrink-0">
                  {t("createTable.starrocks.distributionType")}
                </span>
                <div className="flex gap-3">
                  {(["hash", "random"] as const).map((type) => (
                    <label key={type} className="flex items-center gap-1.5 cursor-pointer text-sm">
                      <input
                        type="radio"
                        name="sr-dist-type"
                        value={type}
                        checked={srDistType === type}
                        onChange={() => setSrDistType(type)}
                        className="accent-primary"
                      />
                      {type === "hash"
                        ? t("createTable.starrocks.distributionHash")
                        : t("createTable.starrocks.distributionRandom")}
                    </label>
                  ))}
                </div>
              </div>

              {srDistType === "hash" && (
                <div className="flex items-start gap-4">
                  <span className="text-xs text-muted-foreground w-28 shrink-0 pt-1">
                    {t("createTable.starrocks.distributionColumns")}
                  </span>
                  <div className="flex flex-wrap gap-1.5 flex-1">
                    {namedColumns.length === 0 ? (
                      <span className="text-xs text-muted-foreground italic">
                        {t("createTable.starrocks.distributionColumnsPlaceholder")}
                      </span>
                    ) : (
                      namedColumns.map((col) => {
                        const checked = srDistColumns.includes(col);
                        return (
                          <label
                            key={col}
                            className={`flex items-center gap-1 px-2 py-0.5 rounded border text-xs cursor-pointer transition-colors font-mono ${
                              checked
                                ? "bg-primary/10 border-primary/40 text-primary"
                                : "border-border hover:bg-muted/50"
                            }`}
                          >
                            <Checkbox
                              checked={checked}
                              onCheckedChange={(v) =>
                                setSrDistColumns((prev) =>
                                  v ? [...prev, col] : prev.filter((c) => c !== col),
                                )
                              }
                              className="w-3 h-3"
                            />
                            {col}
                          </label>
                        );
                      })
                    )}
                  </div>
                </div>
              )}

              <div className="flex items-center gap-4">
                <span className="text-xs text-muted-foreground w-28 shrink-0">
                  {t("createTable.starrocks.distributionBuckets")}
                </span>
                <Input
                  className="h-7 text-xs px-2 font-mono w-24"
                  value={srBuckets}
                  onChange={(e) => setSrBuckets(e.target.value)}
                  placeholder={t("createTable.starrocks.distributionBucketsPlaceholder")}
                />
              </div>
            </div>
          </div>
        )}

        {/* Index editor */}
        <IndexEditorSection
          defs={indexDefs}
          tableColumns={namedColumns}
          driver={dbDriver}
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
                  {generatedSQL || "—"}
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
          {schema ? `.${schema}` : ""}
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={onCancel} disabled={isExecuting}>
            {t("createTable.actions.cancel")}
          </Button>
          <Button onClick={handleExecute} disabled={isExecuting}>
            {isExecuting ? t("createTable.actions.executing") : t("createTable.actions.execute")}
          </Button>
        </div>
      </div>
    </div>
  );
}
