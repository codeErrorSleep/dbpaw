import { useEffect, useState } from "react";
import { Clock, Hash, Loader2, RefreshCw, Save, Trash2 } from "lucide-react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api } from "@/services/api";
import type { RedisKeyPatchPayload, RedisKeyValue, RedisValue } from "@/services/api";
import { toast } from "sonner";
import { RedisStringViewer } from "./value-viewer/RedisStringViewer";
import { RedisHashViewer } from "./value-viewer/RedisHashViewer";
import { RedisListViewer } from "./value-viewer/RedisListViewer";
import { RedisSetViewer } from "./value-viewer/RedisSetViewer";
import { RedisZSetViewer } from "./value-viewer/RedisZSetViewer";
import { RedisStreamViewer } from "./value-viewer/RedisStreamViewer";
import { RedisJsonViewer } from "./value-viewer/RedisJsonViewer";
import {
  countRedisValueItems,
  isRedisValuePagePartial,
  parseRedisTtlSeconds,
} from "./redis-utils";

type RedisKind = RedisValue["kind"];

interface RedisKeyViewProps {
  connectionId: number;
  database: string;
  redisKey: string;
  onDeleted?: () => void;
  onSavedKeyChange?: (key: string) => void;
}

const EDITABLE_KINDS: RedisKind[] = [
  "string",
  "hash",
  "list",
  "set",
  "zSet",
  "stream",
  "json",
];

const KIND_DEFAULT: Record<RedisKind, RedisValue> = {
  string: { kind: "string", value: "" },
  hash: { kind: "hash", value: {} },
  list: { kind: "list", value: [] },
  set: { kind: "set", value: [] },
  zSet: { kind: "zSet", value: [] },
  stream: { kind: "stream", value: [] },
  json: { kind: "json", value: "{}" },
  none: { kind: "none" },
};

const TYPE_BADGE: Record<string, { label: string; className: string }> = {
  string: {
    label: "string",
    className:
      "bg-emerald-100 text-emerald-700 border-emerald-200 dark:bg-emerald-900/30 dark:text-emerald-400 dark:border-emerald-800",
  },
  hash: {
    label: "hash",
    className:
      "bg-blue-100 text-blue-700 border-blue-200 dark:bg-blue-900/30 dark:text-blue-400 dark:border-blue-800",
  },
  list: {
    label: "list",
    className:
      "bg-orange-100 text-orange-700 border-orange-200 dark:bg-orange-900/30 dark:text-orange-400 dark:border-orange-800",
  },
  set: {
    label: "set",
    className:
      "bg-violet-100 text-violet-700 border-violet-200 dark:bg-violet-900/30 dark:text-violet-400 dark:border-violet-800",
  },
  zSet: {
    label: "zset",
    className:
      "bg-rose-100 text-rose-700 border-rose-200 dark:bg-rose-900/30 dark:text-rose-400 dark:border-rose-800",
  },
  stream: {
    label: "stream",
    className:
      "bg-cyan-100 text-cyan-700 border-cyan-200 dark:bg-cyan-900/30 dark:text-cyan-400 dark:border-cyan-800",
  },
  json: {
    label: "json",
    className:
      "bg-amber-100 text-amber-700 border-amber-200 dark:bg-amber-900/30 dark:text-amber-400 dark:border-amber-800",
  },
};

function formatTtl(ttl: number): string {
  if (ttl === -1) return "No expiry";
  if (ttl <= -2) return "Expired";
  const h = Math.floor(ttl / 3600);
  const m = Math.floor((ttl % 3600) / 60);
  const s = ttl % 60;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

function mergeValues(base: RedisValue, next: RedisValue): RedisValue {
  if (base.kind !== next.kind) return base;
  if (base.kind === "list" && next.kind === "list") {
    return { kind: "list", value: [...base.value, ...next.value] };
  }
  if (base.kind === "set" && next.kind === "set") {
    const merged = [
      ...base.value,
      ...next.value.filter((m) => !base.value.includes(m)),
    ];
    return { kind: "set", value: merged };
  }
  if (base.kind === "zSet" && next.kind === "zSet") {
    const existingMembers = new Set(base.value.map((m) => m.member));
    const added = next.value.filter((m) => !existingMembers.has(m.member));
    return { kind: "zSet", value: [...base.value, ...added] };
  }
  if (base.kind === "hash" && next.kind === "hash") {
    return { kind: "hash", value: { ...base.value, ...next.value } };
  }
  if (base.kind === "stream" && next.kind === "stream") {
    const existingIds = new Set(base.value.map((e) => e.id));
    const added = next.value.filter((e) => !existingIds.has(e.id));
    return { kind: "stream", value: [...base.value, ...added] };
  }
  return base;
}

function isValueUnchanged(a: RedisValue, b: RedisValue): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function getJsonValidationError(value: RedisValue): string | null {
  if (value.kind !== "json") return null;
  try {
    JSON.parse(value.value);
    return null;
  } catch (e) {
    return e instanceof Error ? e.message : "Invalid JSON";
  }
}

function buildPatch(
  key: string,
  ttlSeconds: number | null,
  original: RedisValue,
  current: RedisValue,
  originalLoadedCount: number,
): RedisKeyPatchPayload {
  const patch: RedisKeyPatchPayload = { key, ttlSeconds };

  if (current.kind === "hash" && original.kind === "hash") {
    const hashSet: Record<string, string> = {};
    const hashDel: string[] = [];
    for (const [k, v] of Object.entries(current.value)) {
      if (original.value[k] !== v) hashSet[k] = v;
    }
    for (const k of Object.keys(original.value)) {
      if (!(k in current.value)) hashDel.push(k);
    }
    if (Object.keys(hashSet).length > 0) patch.hashSet = hashSet;
    if (hashDel.length > 0) patch.hashDel = hashDel;
    return patch;
  }

  if (current.kind === "set" && original.kind === "set") {
    const origSet = new Set(original.value);
    const currSet = new Set(current.value);
    const setAdd = current.value.filter((m) => !origSet.has(m));
    const setRem = original.value.filter((m) => !currSet.has(m));
    if (setAdd.length > 0) patch.setAdd = setAdd;
    if (setRem.length > 0) patch.setRem = setRem;
    return patch;
  }

  if (current.kind === "zSet" && original.kind === "zSet") {
    const origMap = new Map(original.value.map((m) => [m.member, m.score]));
    const currMap = new Map(current.value.map((m) => [m.member, m.score]));
    const zsetAdd = current.value.filter(
      (m) => !origMap.has(m.member) || origMap.get(m.member) !== m.score,
    );
    const zsetRem = original.value
      .filter((m) => !currMap.has(m.member))
      .map((m) => m.member);
    if (zsetAdd.length > 0) patch.zsetAdd = zsetAdd;
    if (zsetRem.length > 0) patch.zsetRem = zsetRem;
    return patch;
  }

  if (current.kind === "list" && original.kind === "list") {
    const curr = current.value;
    const orig = original.value;

    // Detect modifications within the originally-loaded range
    const modifications: { index: number; value: string }[] = [];
    for (let i = 0; i < Math.min(originalLoadedCount, curr.length); i++) {
      if (curr[i] !== orig[i]) {
        modifications.push({ index: i, value: curr[i] });
      }
    }

    const hasDeletions = curr.length < originalLoadedCount;
    const hasAppends = curr.length > originalLoadedCount;

    // Detect pure prepend: tail matches original prefix
    let hasPrepends = false;
    if (curr.length > originalLoadedCount && modifications.length === 0) {
      const tail = curr.slice(curr.length - originalLoadedCount);
      if (
        JSON.stringify(tail) ===
        JSON.stringify(orig.slice(0, originalLoadedCount))
      ) {
        hasPrepends = true;
      }
    }

    // Detect pure append: head matches original prefix
    let pureAppend = false;
    if (curr.length > originalLoadedCount && modifications.length === 0) {
      const head = curr.slice(0, originalLoadedCount);
      if (
        JSON.stringify(head) ===
        JSON.stringify(orig.slice(0, originalLoadedCount))
      ) {
        pureAppend = true;
      }
    }

    if (!hasDeletions && modifications.length === 0 && pureAppend) {
      patch.listRpush = curr.slice(originalLoadedCount);
      return patch;
    }

    if (!hasDeletions && modifications.length === 0 && hasPrepends) {
      patch.listLpush = curr.slice(0, curr.length - originalLoadedCount);
      return patch;
    }

    if (!hasDeletions && !hasAppends && modifications.length > 0) {
      patch.listSet = modifications;
      return patch;
    }

    if (hasDeletions && modifications.length === 0 && !hasAppends) {
      const deleted = orig.slice(curr.length);
      if (deleted.length > 0) patch.listRem = deleted;
      return patch;
    }

    throw new Error(
      "Mixed list operations in partial-load mode are not supported. " +
        'Use "Load more" to load all items first, then save.',
    );
  }

  if (current.kind === "stream" && original.kind === "stream") {
    const origIds = new Set(original.value.map((e) => e.id));
    const currIds = new Set(current.value.map((e) => e.id));
    const streamAdd = current.value.filter((e) => !origIds.has(e.id));
    const streamDel = original.value
      .filter((e) => !currIds.has(e.id))
      .map((e) => e.id);
    if (streamAdd.length > 0) patch.streamAdd = streamAdd;
    if (streamDel.length > 0) patch.streamDel = streamDel;
    return patch;
  }

  return patch;
}

export function RedisKeyView({
  connectionId,
  database,
  redisKey,
  onDeleted,
  onSavedKeyChange,
}: RedisKeyViewProps) {
  const [record, setRecord] = useState<RedisKeyValue | null>(null);
  const [value, setValue] = useState<RedisValue>({ kind: "string", value: "" });
  const [originalValue, setOriginalValue] = useState<RedisValue>({ kind: "string", value: "" });
  const [originalLoadedCount, setOriginalLoadedCount] = useState(0);
  const [keyName, setKeyName] = useState(redisKey);
  const [ttl, setTtl] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [pendingAction, setPendingAction] = useState<
    "delete" | "overwrite" | "binary_overwrite" | "force_rename" | null
  >(null);
  const [valueIsPartial, setValueIsPartial] = useState(false);
  const [valueTotalLen, setValueTotalLen] = useState<number | null>(null);
  const [loadedOffset, setLoadedOffset] = useState(0);
  const [loadedCount, setLoadedCount] = useState(0);
  const [isLoadingMore, setIsLoadingMore] = useState(false);

  const isCreateMode = redisKey.trim().length === 0;
  const jsonValidationError = getJsonValidationError(value);
  const jsonModuleMissing =
    value.kind === "json" && record?.extra?.subtype === "json-module-missing";

  const load = async () => {
    if (isCreateMode) {
      setRecord(null);
      setKeyName("");
      setTtl("");
      setValue({ kind: "string", value: "" });
      setValueIsPartial(false);
      setValueTotalLen(null);
      return;
    }
    setIsLoading(true);
    try {
      const next = await api.redis.getKey(connectionId, database, redisKey);
      setRecord(next);
      const v = next.value;
      const resolvedKind = EDITABLE_KINDS.includes(v.kind) ? v.kind : "string";
      setValue(resolvedKind === v.kind ? v : KIND_DEFAULT[resolvedKind]);
      setKeyName(next.key);
      setTtl(next.ttl > 0 ? String(next.ttl) : "");
      const count = countRedisValueItems(v);
      setLoadedCount(count);
      const total = next.valueTotalLen ?? null;
      setValueTotalLen(total);
      setValueIsPartial(
        isRedisValuePagePartial(v, total, next.valueOffset, count),
      );
      setLoadedOffset(next.valueOffset);
      setOriginalValue(resolvedKind === v.kind ? v : KIND_DEFAULT[resolvedKind]);
      setOriginalLoadedCount(count);
    } catch (e) {
      toast.error("Failed to load Redis key", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, [connectionId, database, redisKey, isCreateMode]);

  const handleLoadMore = async () => {
    if (!record) return;
    setIsLoadingMore(true);
    try {
      const page = await api.redis.getKeyPage(
        connectionId,
        database,
        redisKey,
        loadedOffset,
        200,
      );
      const merged = mergeValues(value, page.value);
      setValue(merged);
      const newCount = countRedisValueItems(merged);
      setLoadedCount(newCount);
      setLoadedOffset(page.valueOffset);
      setValueIsPartial(
        isRedisValuePagePartial(
          page.value,
          page.valueTotalLen,
          page.valueOffset,
          newCount,
        ),
      );
    } catch (e) {
      toast.error("Failed to load more items", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsLoadingMore(false);
    }
  };

  const doSave = async (forceRename?: boolean) => {
    const normalizedKey = keyName.trim();
    if (!normalizedKey) throw new Error("Redis key cannot be empty");
    if (value.kind === "json") {
      if (jsonModuleMissing) {
        throw new Error("RedisJSON module is unavailable for this key. Saving is disabled.");
      }
      if (jsonValidationError) {
        throw new Error(`Invalid JSON: ${jsonValidationError}`);
      }
    }
    const parsedTtl = parseRedisTtlSeconds(ttl);
    if (!isCreateMode && normalizedKey !== redisKey) {
      try {
        await api.redis.renameKey(
          connectionId,
          database,
          redisKey,
          normalizedKey,
          forceRename,
        );
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        if (msg.includes("already exists") && !forceRename) {
          setPendingAction("force_rename");
          return;
        }
        throw e;
      }
    }
    const ttlOnly =
      !isCreateMode &&
      normalizedKey === redisKey &&
      isValueUnchanged(originalValue, value);

    if (isCreateMode) {
      await api.redis.setKey(connectionId, database, {
        key: normalizedKey,
        value,
        ttlSeconds: parsedTtl,
      });
    } else if (ttlOnly) {
      // Only TTL changed: avoid DEL + rebuild entirely
      await api.redis.setTtl(connectionId, database, normalizedKey, parsedTtl);
      toast.success("TTL updated");
      await load();
      return;
    } else if (value.kind === "json") {
      // JSON always full-replace (no incremental patch for arbitrary JSON)
      await api.redis.updateKey(connectionId, database, {
        key: normalizedKey,
        value,
        ttlSeconds: parsedTtl,
      });
    } else if (valueIsPartial) {
      // Partial load: use incremental patch to avoid overwriting unloaded data.
      // Three distinct TTL intents for patch_key:
      //   parsedTtl (> 0) — user set a value → EXPIRE
      //   0               — user cleared the field; key had a TTL → PERSIST
      //   null            — key had no TTL to begin with → leave unchanged
      const originalTtl = record?.ttl ?? -1;
      const patchTtlSeconds: number | null = ttl.trim()
        ? parsedTtl
        : originalTtl > 0
          ? 0
          : null;
      const patch = buildPatch(
        normalizedKey,
        patchTtlSeconds,
        originalValue,
        value,
        originalLoadedCount,
      );
      await api.redis.patchKey(connectionId, database, patch);
    } else {
      // All data loaded: safe to DEL + rebuild
      await api.redis.updateKey(connectionId, database, {
        key: normalizedKey,
        value,
        ttlSeconds: parsedTtl,
      });
    }
    toast.success("Redis key saved");
    if (normalizedKey !== redisKey) {
      onSavedKeyChange?.(normalizedKey);
    } else if (!isCreateMode) {
      await load();
    }
  };

  const handleApplyTtl = async () => {
    let parsedTtl: number | null;
    try {
      parsedTtl = parseRedisTtlSeconds(ttl);
    } catch (e) {
      toast.error("Invalid TTL", {
        description: e instanceof Error ? e.message : String(e),
      });
      return;
    }
    setIsSaving(true);
    try {
      await api.redis.setTtl(connectionId, database, redisKey, parsedTtl);
      toast.success("TTL updated");
      await load();
    } catch (e) {
      toast.error("Failed to update TTL", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsSaving(false);
    }
  };

  const handleSave = async () => {
    if (value.kind === "json" && jsonValidationError) {
      toast.error("Failed to save Redis key", {
        description: `Invalid JSON: ${jsonValidationError}`,
      });
      return;
    }
    if (jsonModuleMissing) {
      toast.error("Failed to save Redis key", {
        description: "RedisJSON module is unavailable for this key. Saving is disabled.",
      });
      return;
    }
    if (isCreateMode) {
      setIsSaving(true);
      try {
        await doSave();
      } catch (e) {
        toast.error("Failed to save Redis key", {
          description: e instanceof Error ? e.message : String(e),
        });
      } finally {
        setIsSaving(false);
      }
      return;
    }
    try {
      parseRedisTtlSeconds(ttl);
    } catch (e) {
      toast.error("Failed to save Redis key", {
        description: e instanceof Error ? e.message : String(e),
      });
      return;
    }
    // Skip overwrite dialog when no destructive change: patch mode or TTL-only
    const ttlOnly =
      keyName.trim() === redisKey && isValueUnchanged(originalValue, value);
    if (valueIsPartial || ttlOnly) {
      setIsSaving(true);
      try {
        await doSave();
      } catch (e) {
        toast.error("Failed to save Redis key", {
          description: e instanceof Error ? e.message : String(e),
        });
      } finally {
        setIsSaving(false);
      }
      return;
    }
    if (record?.isBinary) {
      setPendingAction("binary_overwrite");
    } else {
      setPendingAction("overwrite");
    }
  };

  const doDelete = async () => {
    await api.redis.deleteKey(connectionId, database, redisKey);
    toast.success("Redis key deleted");
    onDeleted?.();
  };

  const handleConfirm = async () => {
    setIsSaving(true);
    try {
      if (pendingAction === "delete") {
        await doDelete();
      } else if (pendingAction === "force_rename") {
        await doSave(true);
      } else {
        await doSave();
      }
    } catch (e) {
      toast.error("Operation failed", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setIsSaving(false);
      setPendingAction(null);
    }
  };

  const handleKindChange = (newKind: RedisKind) => {
    setValue(KIND_DEFAULT[newKind]);
  };

  const typeBadge = record ? TYPE_BADGE[record.value.kind] : null;

  return (
    <div className="h-full overflow-auto bg-background">
      <div className="mx-auto flex max-w-5xl flex-col gap-4 p-6">
        {/* Header */}
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2.5 min-w-0">
            <h2 className="text-xl font-semibold tracking-tight truncate">
              {isCreateMode ? "New Redis key" : redisKey}
            </h2>
            {typeBadge && (
              <span
                className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium border shrink-0 ${typeBadge.className}`}
              >
                {typeBadge.label}
              </span>
            )}
          </div>
          <div className="flex gap-2 shrink-0">
            <Button
              variant="outline"
              size="sm"
              onClick={() => void load()}
              disabled={isCreateMode || isLoading}
            >
              {isLoading ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <RefreshCw className="mr-2 h-4 w-4" />
              )}
              Refresh
            </Button>
            <Button
              variant="destructive"
              size="sm"
              onClick={() => setPendingAction("delete")}
              disabled={isCreateMode}
            >
              <Trash2 className="mr-2 h-4 w-4" />
              Delete
            </Button>
          </div>
        </div>

        {/* Metadata bar (view mode only) */}
        {!isCreateMode && record && (
          <div className="flex items-center gap-4 text-xs text-muted-foreground px-3 py-2 rounded-lg bg-muted/40 border">
            <span className="flex items-center gap-1.5">
              <Clock className="w-3.5 h-3.5" />
              TTL: {formatTtl(record.ttl)}
            </span>
            {valueTotalLen !== null && valueTotalLen > 0 && (
              <span className="flex items-center gap-1.5">
                <Hash className="w-3.5 h-3.5" />
                {valueTotalLen.toLocaleString()} total
              </span>
            )}
            <span className="text-muted-foreground/60">{database}</span>
          </div>
        )}

        {/* Edit form: key name / type / TTL */}
        <div className="grid gap-4 rounded-lg border bg-card p-4 md:grid-cols-[1fr_160px_160px]">
          <div className="space-y-2">
            <Label>Key</Label>
            <Input
              value={keyName}
              onChange={(e) => setKeyName(e.target.value)}
              placeholder="key name"
            />
          </div>
          <div className="space-y-2">
            <Label>Type</Label>
            {isCreateMode ? (
              <Select
                value={value.kind === "none" ? "string" : value.kind}
                onValueChange={(v) => handleKindChange(v as RedisKind)}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="string">string</SelectItem>
                  <SelectItem value="hash">hash</SelectItem>
                  <SelectItem value="list">list</SelectItem>
                  <SelectItem value="set">set</SelectItem>
                  <SelectItem value="zSet">zset</SelectItem>
                  <SelectItem value="stream">stream</SelectItem>
                  <SelectItem value="json">json</SelectItem>
                </SelectContent>
              </Select>
            ) : (
              <div className="flex h-9 items-center rounded-md border bg-muted/30 px-3 text-sm text-muted-foreground">
                {record?.keyType ?? value.kind}
              </div>
            )}
          </div>
          <div className="space-y-2">
            <Label>TTL (seconds)</Label>
            <div className="flex gap-1.5">
              <Input
                value={ttl}
                onChange={(e) => setTtl(e.target.value)}
                placeholder="persist"
                inputMode="numeric"
              />
              {!isCreateMode && (
                <Button
                  variant="outline"
                  size="sm"
                  className="shrink-0 px-2.5"
                  onClick={() => void handleApplyTtl()}
                  disabled={isSaving}
                  title="Apply TTL without modifying the value"
                >
                  Apply
                </Button>
              )}
            </div>
          </div>
        </div>

        {/* Value viewer */}
        <div className="space-y-2">
          <Label>Value</Label>

          {value.kind === "string" && (
            <RedisStringViewer
              value={value.value}
              onChange={(v) => setValue({ kind: "string", value: v })}
              isBinary={record?.isBinary}
              extra={record?.extra}
            />
          )}
          {value.kind === "hash" && (
            <RedisHashViewer
              value={value.value}
              onChange={(v) => setValue({ kind: "hash", value: v })}
            />
          )}
          {value.kind === "list" && (
            <RedisListViewer
              value={value.value}
              onChange={(v) => setValue({ kind: "list", value: v })}
            />
          )}
          {value.kind === "set" && (
            <RedisSetViewer
              value={value.value}
              onChange={(v) => setValue({ kind: "set", value: v })}
            />
          )}
          {value.kind === "zSet" && (
            <RedisZSetViewer
              value={value.value}
              onChange={(v) => setValue({ kind: "zSet", value: v })}
              extra={record?.extra}
            />
          )}
          {value.kind === "stream" && (
            <RedisStreamViewer
              connectionId={connectionId}
              database={database}
              redisKey={redisKey}
              value={value.value}
              onChange={(v) => setValue({ kind: "stream", value: v })}
              totalLen={valueTotalLen}
              extra={record?.extra}
              isCreateMode={isCreateMode}
            />
          )}
          {value.kind === "json" && (
            <RedisJsonViewer
              value={value.value}
              onChange={(v) => setValue({ kind: "json", value: v })}
              moduleMissing={record?.extra?.subtype === "json-module-missing"}
              readOnly={record?.extra?.subtype === "json-module-missing"}
            />
          )}
          {value.kind === "none" && (
            <div className="text-sm text-muted-foreground italic py-4">
              Key does not exist or type is unsupported.
            </div>
          )}

          {valueIsPartial && !isCreateMode && value.kind !== "stream" && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground pt-1">
              <span>
                Showing {loadedCount} of {valueTotalLen} items
              </span>
              <Button
                variant="outline"
                size="sm"
                onClick={() => void handleLoadMore()}
                disabled={isLoadingMore}
              >
                {isLoadingMore ? (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                ) : null}
                Load more
              </Button>
            </div>
          )}

        </div>

        {/* Save */}
        <div className="flex justify-end">
          <Button
            onClick={() => void handleSave()}
            disabled={isSaving || Boolean(jsonValidationError) || jsonModuleMissing}
          >
            {isSaving ? (
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            ) : (
              <Save className="mr-2 h-4 w-4" />
            )}
            Save
          </Button>
        </div>
      </div>

      <AlertDialog
        open={pendingAction !== null}
        onOpenChange={(open) => {
          if (!open) setPendingAction(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {pendingAction === "delete"
                ? "Delete this key?"
                : pendingAction === "binary_overwrite"
                  ? "Overwrite binary key?"
                  : pendingAction === "force_rename"
                    ? "Key already exists"
                    : "Overwrite key data?"}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {pendingAction === "delete"
                ? `"${redisKey}" will be permanently deleted. This cannot be undone.`
                : pendingAction === "binary_overwrite"
                  ? "This key contains binary data. Overwriting as text may corrupt the original bytes. This cannot be undone."
                  : pendingAction === "force_rename"
                    ? `Key "${keyName.trim()}" already exists. Force overwrite?`
                    : "This will replace the current value. This cannot be undone."}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={() => void handleConfirm()}>
              {pendingAction === "delete"
                ? "Delete"
                : pendingAction === "force_rename"
                  ? "Force overwrite"
                  : "Overwrite"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
