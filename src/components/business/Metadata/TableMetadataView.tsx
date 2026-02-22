import { useEffect, useMemo, useState } from "react";
import { api, type TableMetadata } from "@/services/api";
import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

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

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setMetadata(null);

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
                        {col.primaryKey && (
                          <Badge variant="default">PK</Badge>
                        )}
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
    </div>
  );
}

