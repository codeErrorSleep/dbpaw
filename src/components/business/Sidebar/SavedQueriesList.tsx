import { useEffect, useState } from "react";
import { api, SavedQuery, Driver } from "@/services/api";
import { Button } from "@/components/ui/button";
import {
  Database,
  RefreshCw,
  Trash2,
  Edit3,
  Search,
  Server,
} from "lucide-react";
import { Input } from "@/components/ui/input";
import { siMysql, siPostgresql, siSqlite, type SimpleIcon } from "simple-icons";

const renderSimpleIcon = (icon: SimpleIcon) => (
  <svg
    viewBox="0 0 24 24"
    width="16"
    height="16"
    aria-hidden="true"
    className="shrink-0"
    role="img"
  >
    <path d={icon.path} fill="currentColor" />
  </svg>
);

const getConnectionIcon = (driver?: Driver): React.ReactNode => {
  const normalized = String(driver || "")
    .trim()
    .toLowerCase();

  switch (normalized) {
    case "postgres":
    case "postgresql":
    case "pgsql":
      return renderSimpleIcon(siPostgresql);
    case "mysql":
    case "mariadb":
      return renderSimpleIcon(siMysql);
    case "sqlite":
    case "sqlite3":
      return renderSimpleIcon(siSqlite);
    case "clickhouse":
    case "mssql":
      return <Database className="w-4 h-4" />;
    default:
      return <Server className="w-4 h-4" />;
  }
};

interface SavedQueriesListProps {
  onSelectQuery: (query: SavedQuery) => void;
  lastUpdated?: number;
}

export function SavedQueriesList({
  onSelectQuery,
  lastUpdated,
}: SavedQueriesListProps) {
  const [queries, setQueries] = useState<SavedQuery[]>([]);
  const [connections, setConnections] = useState<Record<number, string>>({});
  const [connectionTypes, setConnectionTypes] = useState<
    Record<number, Driver>
  >({});
  const [searchTerm, setSearchTerm] = useState("");
  const [contextMenu, setContextMenu] = useState<{
    visible: boolean;
    x: number;
    y: number;
    queryId: number | null;
  }>({ visible: false, x: 0, y: 0, queryId: null });

  useEffect(() => {
    fetchQueriesAndConnections();
  }, [lastUpdated]);

  const fetchQueriesAndConnections = async () => {
    try {
      const [queriesData, connectionsData] = await Promise.all([
        api.queries.list(),
        api.connections.list(),
      ]);
      setQueries(queriesData);

      const connMap: Record<number, string> = {};
      const connTypeMap: Record<number, Driver> = {};
      connectionsData.forEach((c: any) => {
        connMap[c.id] = c.name;
        connTypeMap[c.id] = c.dbType;
      });
      setConnections(connMap);
      setConnectionTypes(connTypeMap);
    } catch (error) {
      console.error("Failed to fetch saved queries or connections:", error);
    }
  };

  const fetchQueries = async () => {
    try {
      const data = await api.queries.list();
      setQueries(data);
    } catch (error) {
      console.error("Failed to fetch saved queries:", error);
    }
  };

  const handleDelete = async (id: number) => {
    try {
      await api.queries.delete(id);
      fetchQueries();
    } catch (error) {
      console.error("Failed to delete query:", error);
    }
  };

  const filteredQueries = queries.filter((q) =>
    q.name.toLowerCase().includes(searchTerm.toLowerCase()),
  );

  return (
    <div className="h-full flex flex-col bg-background border-r border-border">
      <div className="px-2 py-1 border-b border-border flex items-center justify-between h-8">
        <h2 className="font-semibold text-sm">Saved Queries</h2>
        <Button
          variant="ghost"
          size="sm"
          className="h-6 w-6 p-0"
          onClick={fetchQueriesAndConnections}
        >
          <RefreshCw className="w-3.5 h-3.5" />
        </Button>
      </div>

      <div className="p-2 border-b border-border">
        <div className="relative">
          <Search className="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search queries..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="pl-8"
          />
        </div>
      </div>

      <div
        className="flex-1 overflow-auto"
        onClick={() => setContextMenu((prev) => ({ ...prev, visible: false }))}
      >
        {filteredQueries.map((query) => (
          <div
            key={query.id}
            className="flex items-center gap-2 px-3 py-1.5 hover:bg-accent cursor-pointer group select-none text-sm"
            onDoubleClick={() => onSelectQuery(query)}
            onContextMenu={(e) => {
              e.preventDefault();
              e.stopPropagation();
              setContextMenu({
                visible: true,
                x: e.clientX,
                y: e.clientY,
                queryId: query.id,
              });
            }}
          >
            {getConnectionIcon(
              query.connectionId
                ? connectionTypes[query.connectionId]
                : undefined,
            )}
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <span className="truncate font-medium">{query.name}</span>
                {query.connectionId && connections[query.connectionId] && (
                  <span className="text-xs text-muted-foreground truncate">
                    ({connections[query.connectionId]})
                  </span>
                )}
              </div>
              {query.description && (
                <div className="truncate text-xs text-muted-foreground">
                  {query.description}
                </div>
              )}
            </div>
          </div>
        ))}
        {filteredQueries.length === 0 && (
          <div className="p-4 text-center text-sm text-muted-foreground">
            No saved queries found.
          </div>
        )}
      </div>

      {contextMenu.visible && (
        <div
          className="fixed z-50 min-w-[140px] bg-popover border border-border rounded-md shadow-lg py-1"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          <button
            className="w-full px-3 py-2 text-left text-sm hover:bg-accent flex items-center gap-2"
            onClick={() => {
              if (contextMenu.queryId) {
                const query = queries.find((q) => q.id === contextMenu.queryId);
                if (query) onSelectQuery(query);
              }
              setContextMenu((prev) => ({ ...prev, visible: false }));
            }}
          >
            <Edit3 className="w-4 h-4" />
            Open
          </button>
          <div className="h-px bg-border my-1" />
          <button
            className="w-full px-3 py-2 text-left text-sm hover:bg-accent text-destructive flex items-center gap-2"
            onClick={() => {
              if (contextMenu.queryId) handleDelete(contextMenu.queryId);
              setContextMenu((prev) => ({ ...prev, visible: false }));
            }}
          >
            <Trash2 className="w-4 h-4" />
            Delete
          </button>
        </div>
      )}
    </div>
  );
}
