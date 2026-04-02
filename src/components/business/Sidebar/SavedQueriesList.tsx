import { useEffect, useState } from "react";
import { api, SavedQuery, Driver } from "@/services/api";
import { Button } from "@/components/ui/button";
import { RefreshCw, Trash2, Edit3, Search, Plus } from "lucide-react";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useTranslation } from "react-i18next";
import { getConnectionIcon } from "@/lib/driver-registry";

interface SavedQueriesListProps {
  onSelectQuery: (query: SavedQuery) => void;
  onCreateQuery?: (
    connectionId: number,
    databaseName: string,
    driver: string,
  ) => void;
  lastUpdated?: number;
}

interface ConnectionOption {
  id: number;
  name: string;
  dbType: Driver;
  database?: string;
}

const DEFAULT_DATABASE_VALUE = "__default__";

export function SavedQueriesList({
  onSelectQuery,
  onCreateQuery,
  lastUpdated,
}: SavedQueriesListProps) {
  const { t } = useTranslation();
  const [queries, setQueries] = useState<SavedQuery[]>([]);
  const [connectionOptions, setConnectionOptions] = useState<
    ConnectionOption[]
  >([]);
  const [connections, setConnections] = useState<Record<number, string>>({});
  const [connectionTypes, setConnectionTypes] = useState<
    Record<number, Driver>
  >({});
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [selectedConnectionId, setSelectedConnectionId] = useState("");
  const [databaseOptions, setDatabaseOptions] = useState<string[]>([]);
  const [selectedDatabase, setSelectedDatabase] = useState(
    DEFAULT_DATABASE_VALUE,
  );
  const [loadingDatabases, setLoadingDatabases] = useState(false);
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
      const normalizedConnections: ConnectionOption[] = [];
      connectionsData.forEach((c: any) => {
        const id = Number(c.id);
        if (!Number.isFinite(id)) return;
        const name =
          typeof c.name === "string" && c.name.trim().length > 0
            ? c.name
            : `#${id}`;
        const dbType = (c.dbType || c.driver || "postgres") as Driver;
        const database =
          typeof c.database === "string" && c.database.trim().length > 0
            ? c.database
            : undefined;
        connMap[id] = name;
        connTypeMap[id] = dbType;
        normalizedConnections.push({ id, name, dbType, database });
      });
      setConnections(connMap);
      setConnectionTypes(connTypeMap);
      setConnectionOptions(normalizedConnections);
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

  const loadDatabasesForConnection = async (connectionId: number) => {
    setLoadingDatabases(true);
    try {
      const data = await api.metadata.listDatabasesById(connectionId);
      const normalized = Array.from(
        new Set(
          data
            .map((item) => (typeof item === "string" ? item.trim() : ""))
            .filter((item) => item.length > 0),
        ),
      );
      setDatabaseOptions(normalized);
    } catch (error) {
      console.error("Failed to load databases for query creation:", error);
      setDatabaseOptions([]);
    } finally {
      setLoadingDatabases(false);
    }
  };

  const handleConnectionChange = (value: string) => {
    setSelectedConnectionId(value);
    setSelectedDatabase(DEFAULT_DATABASE_VALUE);
    const connectionId = Number(value);
    if (!Number.isFinite(connectionId)) {
      setDatabaseOptions([]);
      return;
    }
    void loadDatabasesForConnection(connectionId);
  };

  const handleCreateDialogOpenChange = (open: boolean) => {
    setCreateDialogOpen(open);
    if (open) return;
    setSelectedConnectionId("");
    setSelectedDatabase(DEFAULT_DATABASE_VALUE);
    setDatabaseOptions([]);
    setLoadingDatabases(false);
  };

  const handleCreate = () => {
    if (!onCreateQuery || !selectedConnectionId) return;
    const connectionId = Number(selectedConnectionId);
    const connection = connectionOptions.find(
      (item) => item.id === connectionId,
    );
    if (!connection) return;
    const explicitDatabase =
      selectedDatabase === DEFAULT_DATABASE_VALUE ? "" : selectedDatabase;
    const resolvedDatabase = explicitDatabase || connection.database || "";
    onCreateQuery(connection.id, resolvedDatabase, connection.dbType);
    handleCreateDialogOpenChange(false);
  };

  const filteredQueries = queries.filter((q) =>
    q.name.toLowerCase().includes(searchTerm.toLowerCase()),
  );

  return (
    <div className="h-full flex flex-col bg-background border-r border-border">
      <div className="px-2 py-1 border-b border-border flex items-center justify-between h-8">
        <h2 className="font-semibold text-sm">{t("sidebar.queries.title")}</h2>
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            className="h-6 px-1.5 gap-1"
            onClick={() => handleCreateDialogOpenChange(true)}
            disabled={!onCreateQuery}
          >
            <Plus className="w-3.5 h-3.5" />
            <span className="text-xs">{t("sidebar.queries.newQuery")}</span>
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="h-6 w-6 p-0"
            onClick={fetchQueriesAndConnections}
          >
            <RefreshCw className="w-3.5 h-3.5" />
          </Button>
        </div>
      </div>

      <div className="p-2 border-b border-border">
        <div className="relative">
          <Search className="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder={t("sidebar.queries.searchPlaceholder")}
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
            {t("sidebar.queries.empty")}
          </div>
        )}
      </div>

      <Dialog
        open={createDialogOpen}
        onOpenChange={handleCreateDialogOpenChange}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {t("sidebar.queries.newQueryDialog.title")}
            </DialogTitle>
          </DialogHeader>
          <div className="grid gap-4 py-2">
            <div className="grid gap-2">
              <Label htmlFor="query-create-connection">
                {t("sidebar.queries.newQueryDialog.connection")}
              </Label>
              <Select
                value={selectedConnectionId}
                onValueChange={handleConnectionChange}
              >
                <SelectTrigger id="query-create-connection">
                  <SelectValue
                    placeholder={t(
                      "sidebar.queries.newQueryDialog.connectionPlaceholder",
                    )}
                  />
                </SelectTrigger>
                <SelectContent>
                  {connectionOptions.map((connection) => (
                    <SelectItem
                      key={connection.id}
                      value={String(connection.id)}
                      className="text-sm"
                    >
                      {connection.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="grid gap-2">
              <Label htmlFor="query-create-database">
                {t("sidebar.queries.newQueryDialog.databaseOptional")}
              </Label>
              <Select
                value={selectedDatabase}
                onValueChange={setSelectedDatabase}
                disabled={!selectedConnectionId || loadingDatabases}
              >
                <SelectTrigger id="query-create-database">
                  <SelectValue
                    placeholder={t(
                      "sidebar.queries.newQueryDialog.databasePlaceholder",
                    )}
                  />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={DEFAULT_DATABASE_VALUE}>
                    {t("sidebar.queries.newQueryDialog.databaseDefault")}
                  </SelectItem>
                  {databaseOptions.map((database) => (
                    <SelectItem key={database} value={database}>
                      {database}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => handleCreateDialogOpenChange(false)}
            >
              {t("sidebar.queries.newQueryDialog.cancel")}
            </Button>
            <Button
              type="button"
              onClick={handleCreate}
              disabled={!selectedConnectionId}
            >
              {t("sidebar.queries.newQueryDialog.confirm")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

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
            {t("sidebar.queries.open")}
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
            {t("sidebar.queries.delete")}
          </button>
        </div>
      )}
    </div>
  );
}
