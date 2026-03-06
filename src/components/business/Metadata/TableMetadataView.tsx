import { useEffect, useMemo, useState } from "react";
import { api, type TableMetadata } from "@/services/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Copy, Check } from "lucide-react";
import { toast } from "sonner";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

function extractCreateTableStatement(ddl: string): string {
  const trimmed = ddl.trim();
  if (!trimmed) return "";

  const match = trimmed.match(/create\s+table\b[\s\S]*?;/i);
  return (match?.[0] ?? trimmed).trim();
}

interface CopyButtonProps {
  text: string | null;
}

function CopyButton({ text }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      toast.success("Copied");
      setTimeout(() => setCopied(false), 2000);
    } catch {
      toast.error("Copy failed");
    }
  };

  return (
    <Button
      variant="ghost"
      size="sm"
      onClick={handleCopy}
      disabled={!text}
      className="h-7 px-2 text-xs"
    >
      {copied ? (
        <Check className="h-3.5 w-3.5 mr-1" />
      ) : (
        <Copy className="h-3.5 w-3.5 mr-1" />
      )}
      {copied ? "Copied" : "Copy"}
    </Button>
  );
}

interface TableMetadataViewProps {
  connectionId: number;
  database: string;
  schema: string;
  table: string;
}

export function TableMetadataView({
  connectionId,
  database,
  schema,
  table,
}: TableMetadataViewProps) {
  const [metadata, setMetadata] = useState<TableMetadata | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ddl, setDdl] = useState<string | null>(null);
  const [ddlLoading, setDdlLoading] = useState(false);
  const [ddlError, setDdlError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setMetadata(null);
    setDdlLoading(true);
    setDdlError(null);
    setDdl(null);

    api.metadata
      .getTableMetadata(connectionId, database, schema, table)
      .then((res) => {
        if (cancelled) return;
        setMetadata(res);
      })
      .catch((e) => {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (cancelled) return;
        setLoading(false);
      });

    api.metadata
      .getTableDDL(connectionId, database, schema, table)
      .then((res) => {
        if (cancelled) return;
        setDdl(extractCreateTableStatement(res));
      })
      .catch((e) => {
        if (cancelled) return;
        setDdlError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (cancelled) return;
        setDdlLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [connectionId, database, schema, table]);

  const fkColumns = useMemo(() => {
    const set = new Set<string>();
    for (const fk of metadata?.foreignKeys ?? []) {
      set.add(fk.column);
    }
    return set;
  }, [metadata]);
  const clickhouseExtra = metadata?.clickhouseExtra ?? null;

  return (
    <div className="h-full overflow-auto bg-background p-4 space-y-6">
      <div className="flex items-center justify-between">
        <div className="min-w-0">
          <div className="text-sm text-muted-foreground truncate">
            {schema}.{table}
          </div>
          <div className="text-lg font-semibold truncate">Table Metadata</div>
        </div>
        {loading && <Badge variant="secondary">Loading</Badge>}
        {error && <Badge variant="destructive">Error</Badge>}
      </div>

      {error && (
        <div className="text-sm text-destructive break-words">{error}</div>
      )}

      <section className="space-y-2">
        <div className="text-sm font-semibold">Columns</div>
        <div className="border border-border rounded-md overflow-hidden">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-[220px]">Column Name</TableHead>
                <TableHead className="w-[220px]">Type</TableHead>
                <TableHead className="w-[90px]">Nullable</TableHead>
                <TableHead className="w-[220px]">Default Value</TableHead>
                <TableHead className="w-[160px]">PK/FK</TableHead>
                <TableHead>Description</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {(metadata?.columns ?? []).length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6} className="text-muted-foreground">
                    No column information
                  </TableCell>
                </TableRow>
              ) : (
                (metadata?.columns ?? []).map((col) => {
                  const isFk = fkColumns.has(col.name);
                  return (
                    <TableRow key={col.name}>
                      <TableCell className="font-mono">{col.name}</TableCell>
                      <TableCell className="font-mono">{col.type}</TableCell>
                      <TableCell>{col.nullable ? "YES" : "NO"}</TableCell>
                      <TableCell className="font-mono">
                        {col.defaultValue ?? ""}
                      </TableCell>
                      <TableCell className="flex items-center gap-2">
                        {col.primaryKey && <Badge variant="default">PK</Badge>}
                        {isFk && <Badge variant="outline">FK</Badge>}
                      </TableCell>
                      <TableCell>{col.comment ?? ""}</TableCell>
                    </TableRow>
                  );
                })
              )}
            </TableBody>
          </Table>
        </div>
      </section>

      <section className="space-y-2">
        {clickhouseExtra && (
          <>
            <div className="text-sm font-semibold">ClickHouse</div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
              <div className="border border-border rounded-md p-2">
                <div className="text-xs text-muted-foreground">Engine</div>
                <div className="font-mono text-sm break-words">
                  {clickhouseExtra.engine}
                </div>
              </div>
              {clickhouseExtra.partitionKey && (
                <div className="border border-border rounded-md p-2">
                  <div className="text-xs text-muted-foreground">Partition Key</div>
                  <div className="font-mono text-sm break-words">
                    {clickhouseExtra.partitionKey}
                  </div>
                </div>
              )}
              {clickhouseExtra.sortingKey && (
                <div className="border border-border rounded-md p-2">
                  <div className="text-xs text-muted-foreground">Sorting Key</div>
                  <div className="font-mono text-sm break-words">
                    {clickhouseExtra.sortingKey}
                  </div>
                </div>
              )}
              {clickhouseExtra.primaryKeyExpr && (
                <div className="border border-border rounded-md p-2">
                  <div className="text-xs text-muted-foreground">
                    Primary Key Expr
                  </div>
                  <div className="font-mono text-sm break-words">
                    {clickhouseExtra.primaryKeyExpr}
                  </div>
                </div>
              )}
              {clickhouseExtra.samplingKey && (
                <div className="border border-border rounded-md p-2">
                  <div className="text-xs text-muted-foreground">Sampling Key</div>
                  <div className="font-mono text-sm break-words">
                    {clickhouseExtra.samplingKey}
                  </div>
                </div>
              )}
              {clickhouseExtra.ttlExpr && (
                <div className="border border-border rounded-md p-2 md:col-span-2">
                  <div className="text-xs text-muted-foreground">TTL</div>
                  <div className="font-mono text-sm break-words">
                    {clickhouseExtra.ttlExpr}
                  </div>
                </div>
              )}
            </div>
          </>
        )}
      </section>

      <section className="space-y-2">
        <div className="text-sm font-semibold">Indexes</div>
        <div className="border border-border rounded-md overflow-hidden">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-[240px]">Index Name</TableHead>
                <TableHead className="w-[120px]">Unique</TableHead>
                <TableHead className="w-[160px]">Type</TableHead>
                <TableHead>Columns</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {(metadata?.indexes ?? []).length === 0 ? (
                <TableRow>
                  <TableCell colSpan={4} className="text-muted-foreground">
                    No index information
                  </TableCell>
                </TableRow>
              ) : (
                (metadata?.indexes ?? []).map((idx) => (
                  <TableRow key={idx.name}>
                    <TableCell className="font-mono">{idx.name}</TableCell>
                    <TableCell>
                      {idx.unique ? (
                        <Badge variant="secondary">YES</Badge>
                      ) : (
                        <Badge variant="outline">NO</Badge>
                      )}
                    </TableCell>
                    <TableCell className="font-mono">
                      {idx.indexType ?? ""}
                    </TableCell>
                    <TableCell className="font-mono">
                      {(idx.columns ?? []).join(", ")}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      </section>

      <section className="space-y-2">
        <div className="text-sm font-semibold">Foreign Keys</div>
        <div className="border border-border rounded-md overflow-hidden">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-[240px]">FK Name</TableHead>
                <TableHead className="w-[180px]">Local Column</TableHead>
                <TableHead className="w-[320px]">References</TableHead>
                <TableHead className="w-[140px]">On Update</TableHead>
                <TableHead className="w-[140px]">On Delete</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {(metadata?.foreignKeys ?? []).length === 0 ? (
                <TableRow>
                  <TableCell colSpan={5} className="text-muted-foreground">
                    No foreign key information
                  </TableCell>
                </TableRow>
              ) : (
                (metadata?.foreignKeys ?? []).map((fk, i) => (
                  <TableRow key={`${fk.name}-${fk.column}-${i}`}>
                    <TableCell className="font-mono">{fk.name}</TableCell>
                    <TableCell className="font-mono">{fk.column}</TableCell>
                    <TableCell className="font-mono">
                      {(fk.referencedSchema ? `${fk.referencedSchema}.` : "") +
                        fk.referencedTable}
                      ({fk.referencedColumn})
                    </TableCell>
                    <TableCell>{fk.onUpdate ?? ""}</TableCell>
                    <TableCell>{fk.onDelete ?? ""}</TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      </section>

      <section className="space-y-2">
        <div className="flex items-center justify-between">
          <div className="text-sm font-semibold">Create Table SQL</div>
          <CopyButton text={ddl} />
        </div>
        <div className="border border-border rounded-md bg-muted/10">
          {ddlLoading ? (
            <div className="p-3 text-sm text-muted-foreground">
              Loading DDL...
            </div>
          ) : ddlError ? (
            <div className="p-3 text-sm text-destructive break-words">
              {ddlError}
            </div>
          ) : ddl ? (
            <pre className="p-3 text-sm font-mono whitespace-pre-wrap break-words overflow-auto max-h-80">
              <code>{ddl}</code>
            </pre>
          ) : (
            <div className="p-3 text-sm text-muted-foreground">
              No DDL available
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
