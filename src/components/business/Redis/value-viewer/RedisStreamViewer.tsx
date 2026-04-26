import { useEffect, useMemo, useState, type Dispatch, type SetStateAction } from "react";
import {
  Check,
  ChevronDown,
  ChevronRight,
  Filter,
  Info,
  Loader2,
  Plus,
  RotateCcw,
  Trash2,
  Users,
  X,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  api,
  type RedisKeyExtra,
  type RedisStreamEntry,
  type RedisStreamGroup,
  type RedisStreamView,
} from "@/services/api";

const DEFAULT_PAGE_SIZE = 200;

interface Props {
  connectionId: number;
  database: string;
  redisKey: string;
  value: RedisStreamEntry[];
  onChange: (v: RedisStreamEntry[]) => void;
  totalLen?: number | null;
  extra?: RedisKeyExtra | null;
  isCreateMode?: boolean;
}

interface StreamBrowserState {
  startIdInput: string;
  endIdInput: string;
  countInput: string;
  appliedStartId: string;
  appliedEndId: string;
  pageSize: number;
  nextStartId: string | null;
  totalLen: number | null;
  streamInfo: RedisKeyExtra["streamInfo"];
  groups: RedisStreamGroup[];
}

const createInitialBrowserState = (
  entries: RedisStreamEntry[],
  totalLen?: number | null,
  extra?: RedisKeyExtra | null,
): StreamBrowserState => ({
  startIdInput: "",
  endIdInput: "",
  countInput: String(DEFAULT_PAGE_SIZE),
  appliedStartId: "-",
  appliedEndId: "+",
  pageSize: DEFAULT_PAGE_SIZE,
  nextStartId:
    totalLen !== null && totalLen !== undefined && entries.length < totalLen && entries.length > 0
      ? `(${entries[entries.length - 1].id}`
      : null,
  totalLen: totalLen ?? null,
  streamInfo: extra?.streamInfo ?? null,
  groups: extra?.streamGroups ?? [],
});

function formatFields(fields: Record<string, string>) {
  const keys = Object.keys(fields);
  if (keys.length === 0) return "{}";
  if (keys.length <= 3) {
    return "{ " + keys.map((key) => `${key}: ${fields[key]}`).join(", ") + " }";
  }
  return `{ ${keys[0]}: ${fields[keys[0]]}, ${keys[1]}: ${fields[keys[1]]}, ... +${keys.length - 2} }`;
}

function parseFieldsRaw(raw: string): Record<string, string> | null {
  const result: Record<string, string> = {};
  const lines = raw.split(/\n|,/);
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const eqIdx = trimmed.indexOf("=");
    if (eqIdx === -1) return null;
    const key = trimmed.slice(0, eqIdx).trim();
    const value = trimmed.slice(eqIdx + 1).trim();
    if (!key) return null;
    result[key] = value;
  }
  return result;
}

function resolvePageSize(raw: string) {
  const parsed = Number(raw.trim() || String(DEFAULT_PAGE_SIZE));
  if (!Number.isInteger(parsed) || parsed < 1 || parsed > 1000) {
    throw new Error("Count must be an integer between 1 and 1000");
  }
  return parsed;
}

function mapViewResultToBrowserState(result: RedisStreamView, current: StreamBrowserState): StreamBrowserState {
  return {
    ...current,
    appliedStartId: result.startId,
    appliedEndId: result.endId,
    pageSize: result.count,
    nextStartId: result.nextStartId ?? null,
    totalLen: result.totalLen,
    streamInfo: result.streamInfo ?? null,
    groups: result.groups,
  };
}

export function RedisStreamViewer({
  connectionId,
  database,
  redisKey,
  value,
  onChange,
  totalLen,
  extra,
  isCreateMode,
}: Props) {
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [showNewRow, setShowNewRow] = useState(false);
  const [newId, setNewId] = useState("*");
  const [newFieldsRaw, setNewFieldsRaw] = useState("");
  const [browser, setBrowser] = useState<StreamBrowserState>(() =>
    createInitialBrowserState(value, totalLen, extra),
  );
  const [isLoadingView, setIsLoadingView] = useState(false);

  useEffect(() => {
    setBrowser(createInitialBrowserState(value, totalLen, extra));
    setExpandedIds(new Set());
    setShowNewRow(false);
    setNewId("*");
    setNewFieldsRaw("");
  }, [connectionId, database, redisKey, totalLen, extra]);

  const hasMore = useMemo(() => {
    if (isCreateMode) return false;
    if (browser.nextStartId) return true;
    return browser.totalLen !== null && value.length < browser.totalLen;
  }, [browser.nextStartId, browser.totalLen, isCreateMode, value.length]);

  const loadStreamView = async (
    mode: "replace" | "append",
    overrides?: { startId?: string; endId?: string; count?: number },
  ) => {
    if (isCreateMode) return;

    let count: number;
    try {
      count = overrides?.count ?? resolvePageSize(browser.countInput);
    } catch (e) {
      toast.error("Invalid stream range", {
        description: e instanceof Error ? e.message : String(e),
      });
      return;
    }

    const startId =
      mode === "append"
        ? browser.nextStartId ||
          (value.length > 0 ? `(${value[value.length - 1].id}` : browser.appliedStartId)
        : (overrides?.startId ?? browser.startIdInput.trim()) || "-";
    const endId = (overrides?.endId ?? browser.endIdInput.trim()) || "+";

    setIsLoadingView(true);
    try {
      const result = await api.redis.getStreamView(
        connectionId,
        database,
        redisKey,
        startId,
        endId,
        count,
      );
      onChange(mode === "append" ? [...value, ...result.entries] : result.entries);
      setBrowser((current) => mapViewResultToBrowserState(result, current));
    } catch (e) {
      toast.error(
        mode === "append" ? "Failed to load more stream entries" : "Failed to load stream entries",
        { description: e instanceof Error ? e.message : String(e) },
      );
    } finally {
      setIsLoadingView(false);
    }
  };

  const toggleExpand = (id: string) => {
    setExpandedIds((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const deleteEntry = (id: string) => {
    onChange(value.filter((entry) => entry.id !== id));
  };

  const addEntry = () => {
    const fields = parseFieldsRaw(newFieldsRaw);
    if (!fields) return;
    onChange([{ id: newId.trim() || "*", fields }, ...value]);
    setShowNewRow(false);
    setNewId("*");
    setNewFieldsRaw("");
  };

  return (
    <div className="space-y-3">
      {!isCreateMode && (
        <>
          <StreamFilterBar
            browser={browser}
            isLoading={isLoadingView}
            onChange={setBrowser}
            onApply={() => void loadStreamView("replace")}
            onReset={() => {
              setBrowser((current) => ({
                ...current,
                startIdInput: "",
                endIdInput: "",
                countInput: String(DEFAULT_PAGE_SIZE),
                appliedStartId: "-",
                appliedEndId: "+",
                pageSize: DEFAULT_PAGE_SIZE,
              }));
              void loadStreamView("replace", {
                startId: "-",
                endId: "+",
                count: DEFAULT_PAGE_SIZE,
              });
            }}
          />

          <StreamSummaryCards
            entryCount={value.length}
            totalLen={browser.totalLen}
            streamInfo={browser.streamInfo}
            groups={browser.groups}
            appliedStartId={browser.appliedStartId}
            appliedEndId={browser.appliedEndId}
          />

          <StreamGroupsTable groups={browser.groups} />
        </>
      )}

      <div className="flex items-center justify-between">
        <span className="text-sm text-muted-foreground">
          {value.length} entries
          {browser.totalLen !== null ? ` / ${browser.totalLen}` : ""}
        </span>
        <div className="flex gap-2">
          {!isCreateMode && (
            <span className="inline-flex items-center gap-1 rounded border px-2 py-1 text-xs text-muted-foreground">
              <Info className="h-3 w-3" />
              Page size {browser.pageSize}
            </span>
          )}
          <Button
            variant="outline"
            size="sm"
            className="h-7"
            onClick={() => setShowNewRow(true)}
            disabled={showNewRow}
          >
            <Plus className="mr-1 h-3 w-3" />
            Add entry
          </Button>
        </div>
      </div>

      {showNewRow && (
        <StreamAddEntryForm
          newId={newId}
          newFieldsRaw={newFieldsRaw}
          onIdChange={setNewId}
          onFieldsChange={setNewFieldsRaw}
          onAdd={addEntry}
          onCancel={() => {
            setShowNewRow(false);
            setNewId("*");
            setNewFieldsRaw("");
          }}
        />
      )}

      <StreamEntriesTable
        entries={value}
        expandedIds={expandedIds}
        onToggleExpand={toggleExpand}
        onDelete={deleteEntry}
      />

      {!isCreateMode && hasMore && (
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <span>
            Showing {value.length}
            {browser.totalLen !== null ? ` of ${browser.totalLen}` : ""}
          </span>
          <Button
            variant="outline"
            size="sm"
            onClick={() => void loadStreamView("append")}
            disabled={isLoadingView}
          >
            {isLoadingView ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : null}
            Load more
          </Button>
        </div>
      )}
    </div>
  );
}

function StreamFilterBar({
  browser,
  isLoading,
  onChange,
  onApply,
  onReset,
}: {
  browser: StreamBrowserState;
  isLoading: boolean;
  onChange: Dispatch<SetStateAction<StreamBrowserState>>;
  onApply: () => void;
  onReset: () => void;
}) {
  return (
    <div className="grid gap-2 rounded-md border bg-muted/20 p-3 md:grid-cols-[1fr_1fr_120px_auto_auto]">
      <Input
        className="h-8 font-mono text-xs"
        value={browser.startIdInput}
        onChange={(e) => onChange((current) => ({ ...current, startIdInput: e.target.value }))}
        placeholder="Start ID (-)"
      />
      <Input
        className="h-8 font-mono text-xs"
        value={browser.endIdInput}
        onChange={(e) => onChange((current) => ({ ...current, endIdInput: e.target.value }))}
        placeholder="End ID (+)"
      />
      <Input
        className="h-8 font-mono text-xs"
        value={browser.countInput}
        onChange={(e) => onChange((current) => ({ ...current, countInput: e.target.value }))}
        placeholder="Count"
        inputMode="numeric"
      />
      <Button variant="outline" size="sm" className="h-8" onClick={onApply} disabled={isLoading}>
        {isLoading ? (
          <Loader2 className="mr-1 h-3 w-3 animate-spin" />
        ) : (
          <Filter className="mr-1 h-3 w-3" />
        )}
        Apply
      </Button>
      <Button variant="ghost" size="sm" className="h-8" onClick={onReset}>
        <RotateCcw className="mr-1 h-3 w-3" />
        Reset
      </Button>
    </div>
  );
}

function StreamSummaryCards({
  entryCount,
  totalLen,
  streamInfo,
  groups,
  appliedStartId,
  appliedEndId,
}: {
  entryCount: number;
  totalLen: number | null;
  streamInfo: RedisKeyExtra["streamInfo"];
  groups: RedisStreamGroup[];
  appliedStartId: string;
  appliedEndId: string;
}) {
  return (
    <div className="grid gap-2 md:grid-cols-4">
      <div className="rounded-md border bg-card px-3 py-2 text-xs">
        <div className="text-muted-foreground">Length</div>
        <div className="mt-1 font-mono text-sm">
          {(streamInfo?.length ?? totalLen ?? entryCount).toLocaleString()}
        </div>
      </div>
      <div className="rounded-md border bg-card px-3 py-2 text-xs">
        <div className="text-muted-foreground">Groups</div>
        <div className="mt-1 font-mono text-sm">
          {(streamInfo?.groups ?? groups.length).toLocaleString()}
        </div>
      </div>
      <div className="rounded-md border bg-card px-3 py-2 text-xs">
        <div className="text-muted-foreground">Last generated ID</div>
        <div className="mt-1 truncate font-mono text-sm">
          {streamInfo?.lastGeneratedId || "n/a"}
        </div>
      </div>
      <div className="rounded-md border bg-card px-3 py-2 text-xs">
        <div className="text-muted-foreground">Current range</div>
        <div className="mt-1 font-mono text-sm">
          {appliedStartId} .. {appliedEndId}
        </div>
      </div>
    </div>
  );
}

function StreamGroupsTable({ groups }: { groups: RedisStreamGroup[] }) {
  return (
    <div className="rounded-md border">
      <div className="flex items-center justify-between border-b px-3 py-2">
        <div className="flex items-center gap-2 text-sm">
          <Users className="h-4 w-4 text-muted-foreground" />
          <span>Consumer groups</span>
        </div>
        <span className="text-xs text-muted-foreground">{groups.length} groups</span>
      </div>
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="text-xs">Group</TableHead>
            <TableHead className="text-xs">Consumers</TableHead>
            <TableHead className="text-xs">Pending</TableHead>
            <TableHead className="text-xs">Last delivered ID</TableHead>
            <TableHead className="text-xs">Lag</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {groups.length === 0 ? (
            <TableRow>
              <TableCell colSpan={5} className="py-5 text-center text-sm text-muted-foreground">
                No consumer groups
              </TableCell>
            </TableRow>
          ) : (
            groups.map((group) => (
              <TableRow key={group.name}>
                <TableCell className="font-mono text-xs">{group.name}</TableCell>
                <TableCell className="text-xs">{group.consumers}</TableCell>
                <TableCell className="text-xs">{group.pending}</TableCell>
                <TableCell className="font-mono text-xs">
                  {group.lastDeliveredId || "n/a"}
                </TableCell>
                <TableCell className="text-xs">{group.lag ?? group.entriesRead ?? "n/a"}</TableCell>
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
    </div>
  );
}

function StreamAddEntryForm({
  newId,
  newFieldsRaw,
  onIdChange,
  onFieldsChange,
  onAdd,
  onCancel,
}: {
  newId: string;
  newFieldsRaw: string;
  onIdChange: (value: string) => void;
  onFieldsChange: (value: string) => void;
  onAdd: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="space-y-2 rounded-md border bg-muted/20 p-3">
      <Input
        className="h-7 w-40 font-mono text-xs"
        value={newId}
        onChange={(e) => onIdChange(e.target.value)}
        placeholder="ID (* = auto)"
      />
      <textarea
        className="h-20 w-full resize-y rounded-md border bg-background px-3 py-2 text-xs font-mono"
        value={newFieldsRaw}
        onChange={(e) => onFieldsChange(e.target.value)}
        placeholder={"field1=value1\nfield2=value2"}
      />
      <div className="flex gap-2">
        <Button variant="ghost" size="sm" className="h-7" onClick={onAdd}>
          <Check className="mr-1 h-3 w-3 text-green-500" />
          Add
        </Button>
        <Button variant="ghost" size="sm" className="h-7" onClick={onCancel}>
          <X className="mr-1 h-3 w-3 text-muted-foreground" />
          Cancel
        </Button>
      </div>
    </div>
  );
}

function StreamEntriesTable({
  entries,
  expandedIds,
  onToggleExpand,
  onDelete,
}: {
  entries: RedisStreamEntry[];
  expandedIds: Set<string>;
  onToggleExpand: (id: string) => void;
  onDelete: (id: string) => void;
}) {
  return (
    <div className="rounded-md border">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="w-[40px]" />
            <TableHead className="text-xs">Entry ID</TableHead>
            <TableHead className="text-xs">Field count</TableHead>
            <TableHead className="text-xs">Fields</TableHead>
            <TableHead className="w-[72px]" />
          </TableRow>
        </TableHeader>
        <TableBody>
          {entries.length === 0 && (
            <TableRow>
              <TableCell colSpan={5} className="py-6 text-center text-sm text-muted-foreground">
                No entries in this range
              </TableCell>
            </TableRow>
          )}
          {entries.map((entry) => (
            <StreamEntryRow
              key={entry.id}
              entry={entry}
              expanded={expandedIds.has(entry.id)}
              onToggle={() => onToggleExpand(entry.id)}
              onDelete={() => onDelete(entry.id)}
            />
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

function StreamEntryRow({
  entry,
  expanded,
  onToggle,
  onDelete,
}: {
  entry: RedisStreamEntry;
  expanded: boolean;
  onToggle: () => void;
  onDelete: () => void;
}) {
  return (
    <>
      <TableRow className="group">
        <TableCell className="py-1.5">
          <Button variant="ghost" size="icon" className="h-5 w-5" onClick={onToggle}>
            {expanded ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
          </Button>
        </TableCell>
        <TableCell className="max-w-0 truncate py-1.5 font-mono text-xs text-muted-foreground">
          <span title={entry.id}>{entry.id}</span>
        </TableCell>
        <TableCell className="py-1.5 text-xs">{Object.keys(entry.fields).length}</TableCell>
        <TableCell className="py-1.5">
          <span
            className="block cursor-pointer truncate font-mono text-xs hover:text-foreground/70"
            title={formatFields(entry.fields)}
            onClick={onToggle}
          >
            {formatFields(entry.fields)}
          </span>
        </TableCell>
        <TableCell className="py-1.5">
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 opacity-0 transition-opacity group-hover:opacity-100"
            onClick={onDelete}
          >
            <Trash2 className="h-3 w-3 text-destructive" />
          </Button>
        </TableCell>
      </TableRow>
      {expanded && (
        <TableRow className="bg-muted/20">
          <TableCell colSpan={5} className="py-2">
            <div className="space-y-1 px-2">
              {Object.entries(entry.fields).map(([key, fieldValue]) => (
                <div key={key} className="flex gap-2 text-xs">
                  <span className="min-w-[80px] font-mono text-muted-foreground">{key}</span>
                  <span className="font-mono">{fieldValue}</span>
                </div>
              ))}
            </div>
          </TableCell>
        </TableRow>
      )}
    </>
  );
}
