import { AlertTriangle, Plus, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { DbDriver } from "@/lib/sql-gen/createTable";
import { indexGridTemplate } from "@/lib/sql-gen/ddlUtils";
import {
  getIndexMethodOptions,
  supportsIndexManagement,
} from "@/lib/sql-gen/manageIndexes";
import type { IndexDef } from "@/lib/sql-gen/manageIndexes";

interface IndexEditorSectionProps {
  defs: IndexDef[];
  tableColumns: string[];
  driver: DbDriver;
  /** Highlight rows with originalName === null in green (for alter-table mode) */
  highlightNew?: boolean;
  onAdd: () => void;
  onRemove: (id: string) => void;
  onUpdate: (id: string, patch: Partial<IndexDef>) => void;
  onToggleColumn: (defId: string, colName: string) => void;
}

export function IndexEditorSection({
  defs,
  tableColumns,
  driver,
  highlightNew = false,
  onAdd,
  onRemove,
  onUpdate,
  onToggleColumn,
}: IndexEditorSectionProps) {
  const { t } = useTranslation();

  const indexSupported = supportsIndexManagement(driver);
  const indexMethodOptions = getIndexMethodOptions(driver);
  const showIndexMethod = indexMethodOptions.length > 0;
  const showIndexClustered = driver === "mssql";
  const showIndexConcurrently = driver === "postgres";
  const showMethodCol = showIndexMethod || showIndexClustered || showIndexConcurrently;

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium">{t("manageIndexes.form.indexes")}</span>
        {indexSupported && (
          <Button size="sm" variant="outline" onClick={onAdd}>
            <Plus className="w-3.5 h-3.5 mr-1" />
            {t("manageIndexes.form.addIndex")}
          </Button>
        )}
      </div>

      {!indexSupported ? (
        <div className="rounded-md border border-yellow-500/40 bg-yellow-500/5 px-3 py-2">
          <div className="flex items-center gap-1.5 text-xs font-medium text-yellow-700 dark:text-yellow-400">
            <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
            {t("manageIndexes.unsupported.message")}
          </div>
        </div>
      ) : defs.length === 0 ? (
        <div className="border border-dashed rounded-md py-6 text-center text-sm text-muted-foreground">
          {t("manageIndexes.form.noIndexes")}
        </div>
      ) : (
        <div className="border rounded-md overflow-hidden">
          <div
            className="grid bg-muted/50 border-b text-xs font-medium text-muted-foreground px-2 py-1.5 gap-2"
            style={{ gridTemplateColumns: indexGridTemplate(showMethodCol) }}
          >
            <div>{t("manageIndexes.form.indexName")}</div>
            <div className="text-center">{t("manageIndexes.form.unique")}</div>
            <div>{t("manageIndexes.form.columns")}</div>
            {showMethodCol && <div>{t("manageIndexes.form.method")}</div>}
            <div />
          </div>

          {defs.map((def) => (
            <div
              key={def.id}
              className={`grid items-start px-2 py-2 gap-2 border-b last:border-b-0 hover:bg-muted/20 ${
                highlightNew && def.originalName === null ? "bg-green-500/5" : ""
              }`}
              style={{ gridTemplateColumns: indexGridTemplate(showMethodCol) }}
            >
              <Input
                className="h-7 text-xs px-2 font-mono"
                value={def.name}
                onChange={(e) => onUpdate(def.id, { name: e.target.value })}
                placeholder={t("manageIndexes.form.indexName")}
              />

              <div className="flex justify-center pt-1">
                <Checkbox
                  checked={def.unique}
                  onCheckedChange={(v) => onUpdate(def.id, { unique: !!v })}
                />
              </div>

              <div className="space-y-1">
                {def.columns.length > 0 && (
                  <p className="text-xs text-muted-foreground font-mono leading-tight">
                    {def.columns.join(", ")}
                  </p>
                )}
                <div className="flex flex-wrap gap-1">
                  {tableColumns.map((col) => {
                    const selected = def.columns.includes(col);
                    return (
                      <button
                        key={col}
                        type="button"
                        onClick={() => onToggleColumn(def.id, col)}
                        className={`inline-flex items-center px-1.5 py-0.5 rounded text-xs font-mono border transition-colors ${
                          selected
                            ? "bg-primary/10 border-primary/40 text-primary"
                            : "bg-muted/40 border-border text-muted-foreground hover:border-foreground/40 hover:text-foreground"
                        }`}
                      >
                        {selected && <span className="mr-0.5 text-primary">✓</span>}
                        {col}
                      </button>
                    );
                  })}
                </div>
              </div>

              {showMethodCol && (
                <div className="space-y-1">
                  {showIndexMethod && (
                    <Select
                      value={def.indexMethod || "__default__"}
                      onValueChange={(v) =>
                        onUpdate(def.id, { indexMethod: v === "__default__" ? "" : v })
                      }
                    >
                      <SelectTrigger className="h-7 text-xs px-2 font-mono w-full">
                        <SelectValue placeholder={t("manageIndexes.form.method")} />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="__default__" className="text-xs">
                          default
                        </SelectItem>
                        {indexMethodOptions.map((m) => (
                          <SelectItem key={m} value={m} className="text-xs font-mono">
                            {m}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  )}
                  {showIndexClustered && (
                    <label className="flex items-center gap-1.5 text-xs text-muted-foreground cursor-pointer">
                      <Checkbox
                        checked={def.clustered}
                        onCheckedChange={(v) => onUpdate(def.id, { clustered: !!v })}
                      />
                      {t("manageIndexes.form.clustered")}
                    </label>
                  )}
                  {showIndexConcurrently && (
                    <label className="flex items-center gap-1.5 text-xs text-muted-foreground cursor-pointer">
                      <Checkbox
                        checked={def.concurrently}
                        onCheckedChange={(v) => onUpdate(def.id, { concurrently: !!v })}
                      />
                      {t("manageIndexes.form.concurrently")}
                    </label>
                  )}
                </div>
              )}

              <div className="flex justify-center pt-0.5">
                <button
                  className="text-muted-foreground hover:text-destructive"
                  onClick={() => onRemove(def.id)}
                  tabIndex={-1}
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
