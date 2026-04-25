import { useCallback, useEffect, useState } from "react";
import { Loader2, Plus, RefreshCw, Search, Terminal } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { api } from "@/services/api";
import type { RedisKeyInfo } from "@/services/api";
import { toast } from "sonner";
import { cn } from "@/components/ui/utils";
import { RedisKeyView } from "./RedisKeyView";
import { isRedisClusterDatabaseList } from "./redis-utils";

const SCAN_LIMIT = 200;

const TYPE_COLORS: Record<string, string> = {
  string:
    "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400",
  hash: "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
  list: "bg-orange-100 text-orange-700 dark:bg-orange-900/30 dark:text-orange-400",
  set: "bg-violet-100 text-violet-700 dark:bg-violet-900/30 dark:text-violet-400",
  zset: "bg-rose-100 text-rose-700 dark:bg-rose-900/30 dark:text-rose-400",
};

function formatTtlShort(ttl: number): string {
  if (ttl <= -2) return "exp";
  if (ttl === -1) return "";
  if (ttl < 60) return `${ttl}s`;
  if (ttl < 3600) return `${Math.floor(ttl / 60)}m`;
  return `${Math.floor(ttl / 3600)}h`;
}

interface Props {
  connectionId: number;
  database: string;
  onOpenConsole?: () => void;
}

type DetailState =
  | { mode: "none" }
  | { mode: "new" }
  | { mode: "view"; key: string };

export function RedisBrowserView({ connectionId, database, onOpenConsole }: Props) {
  const [pattern, setPattern] = useState("");
  const [keys, setKeys] = useState<RedisKeyInfo[]>([]);
  const [cursor, setCursor] = useState("0");
  const [isPartial, setIsPartial] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [detail, setDetail] = useState<DetailState>({ mode: "none" });
  const [isClusterMode, setIsClusterMode] = useState(false);
  const [requiresPattern, setRequiresPattern] = useState(false);

  // scan never touches detail — callers decide what happens to selection
  const scan = useCallback(
    async (pat: string, cur: string, append: boolean) => {
      if (isClusterMode && !pat.trim()) {
        setKeys([]);
        setCursor("0");
        setIsPartial(false);
        setRequiresPattern(true);
        return;
      }
      setIsLoading(true);
      try {
        const res = await api.redis.scanKeys({
          id: connectionId,
          database,
          cursor: cur,
          pattern: pat.trim() || undefined,
          limit: SCAN_LIMIT,
        });
        setKeys((prev) => (append ? [...prev, ...res.keys] : res.keys));
        setCursor(res.cursor);
        setIsPartial(res.isPartial);
        setRequiresPattern(false);
      } catch (e) {
        toast.error("Failed to scan keys", {
          description: e instanceof Error ? e.message : String(e),
        });
      } finally {
        setIsLoading(false);
      }
    },
    [connectionId, database, isClusterMode],
  );

  useEffect(() => {
    let cancelled = false;
    const init = async () => {
      try {
        const databases = await api.redis.listDatabases(connectionId);
        if (cancelled) return;
        const clusterMode = isRedisClusterDatabaseList(databases);
        setIsClusterMode(clusterMode);
        setRequiresPattern(clusterMode);
        if (!clusterMode) {
          await scan("", "0", false);
        } else {
          setKeys([]);
          setCursor("0");
          setIsPartial(false);
        }
      } catch (e) {
        toast.error("Failed to load Redis databases", {
          description: e instanceof Error ? e.message : String(e),
        });
      }
    };
    void init();
    return () => {
      cancelled = true;
    };
  }, [connectionId, scan]);

  const handleSearch = () => {
    setDetail({ mode: "none" });
    void scan(pattern, "0", false);
  };

  const handleLoadMore = () => void scan(pattern, cursor, true);

  const handleSelectKey = (key: string) => setDetail({ mode: "view", key });

  const handleNewKey = () => setDetail({ mode: "new" });

  const handleKeyDeleted = () => {
    if (detail.mode === "view") {
      setKeys((prev) => prev.filter((k) => k.key !== detail.key));
    }
    setDetail({ mode: "none" });
    void scan(pattern, "0", false);
  };

  const handleKeySaved = (newKey: string) => {
    if (detail.mode === "new") {
      setDetail({ mode: "view", key: newKey });
      void scan(pattern, "0", false);
    } else if (detail.mode === "view" && newKey !== detail.key) {
      setDetail({ mode: "view", key: newKey });
      void scan(pattern, "0", false);
    } else {
      void scan(pattern, "0", false);
    }
  };

  const selectedKey = detail.mode === "view" ? detail.key : null;

  return (
    <ResizablePanelGroup direction="horizontal" className="h-full">
      {/* Left: key browser */}
      <ResizablePanel defaultSize={30} minSize={18} maxSize={50}>
        <div className="h-full flex flex-col border-r">
          {/* Search */}
          <div className="p-3 border-b space-y-2 shrink-0">
            <div className="flex gap-1.5">
              <div className="relative flex-1">
                <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground pointer-events-none" />
                <Input
                  className="h-7 pl-7 text-xs font-mono"
                  placeholder="Pattern (user:* or *)"
                  value={pattern}
                  onChange={(e) => setPattern(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleSearch();
                  }}
                />
              </div>
              <Button
                variant="outline"
                size="sm"
                className="h-7 w-7 p-0 shrink-0"
                onClick={handleSearch}
                disabled={isLoading}
                title="Search / Refresh"
              >
                {isLoading ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <RefreshCw className="w-3.5 h-3.5" />
                )}
              </Button>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-xs text-muted-foreground">
                {keys.length} keys{isPartial ? "+" : ""}
              </span>
              <div className="flex items-center gap-1.5">
                <Button
                  variant="outline"
                  size="sm"
                  className="h-6 px-2 text-xs"
                  onClick={onOpenConsole}
                  title="Open Console"
                >
                  <Terminal className="w-3 h-3 mr-1" />
                  Console
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-6 px-2 text-xs"
                  onClick={handleNewKey}
                >
                  <Plus className="w-3 h-3 mr-1" />
                  New key
                </Button>
              </div>
            </div>
          </div>

          {/* Key list */}
          <div className="flex-1 overflow-y-auto">
            {keys.length === 0 && !isLoading && (
              <div className="p-6 text-center text-xs text-muted-foreground">
                {requiresPattern
                  ? "Redis Cluster browsing requires a search pattern"
                  : "No keys found"}
              </div>
            )}

            {keys.map((k) => {
              const ttlLabel = formatTtlShort(k.ttl);
              return (
                <div
                  key={k.key}
                  className={cn(
                    "flex items-center gap-2 px-3 py-1.5 cursor-pointer hover:bg-muted/50 border-b border-border/30 text-xs",
                    selectedKey === k.key && "bg-accent/50",
                  )}
                  onClick={() => handleSelectKey(k.key)}
                >
                  <span
                    className={cn(
                      "shrink-0 px-1.5 py-0.5 rounded text-[10px] font-medium",
                      TYPE_COLORS[k.keyType] ??
                        "bg-muted text-muted-foreground",
                    )}
                  >
                    {k.keyType}
                  </span>
                  <span
                    className="flex-1 truncate font-mono text-foreground"
                    title={k.key}
                  >
                    {k.key}
                  </span>
                  {ttlLabel && (
                    <span
                      className={cn(
                        "shrink-0 text-[10px] tabular-nums",
                        k.ttl <= -2
                          ? "text-destructive"
                          : "text-muted-foreground",
                      )}
                    >
                      {ttlLabel}
                    </span>
                  )}
                </div>
              );
            })}

            {isPartial && !isLoading && (
              <div className="p-2 text-center">
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-xs h-6"
                  onClick={handleLoadMore}
                >
                  Load more
                </Button>
              </div>
            )}

            {requiresPattern && (
              <div className="p-3 text-xs text-muted-foreground border-t">
                Enter a pattern like <span className="font-mono">user:*</span>{" "}
                before browsing cluster keys. Full-cluster wildcard scans are
                blocked.
              </div>
            )}
          </div>
        </div>
      </ResizablePanel>

      <ResizableHandle />

      {/* Right: key detail */}
      <ResizablePanel defaultSize={70} minSize={50}>
        {detail.mode === "none" ? (
          <div className="h-full flex flex-col items-center justify-center gap-2 text-muted-foreground">
            <p className="text-sm">Select a key to view and edit</p>
            <Button variant="outline" size="sm" onClick={handleNewKey}>
              <Plus className="w-4 h-4 mr-2" />
              New key
            </Button>
          </div>
        ) : (
          <RedisKeyView
            key={detail.mode === "new" ? "__new__" : detail.key}
            connectionId={connectionId}
            database={database}
            redisKey={detail.mode === "new" ? "" : detail.key}
            onDeleted={handleKeyDeleted}
            onSavedKeyChange={handleKeySaved}
          />
        )}
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
