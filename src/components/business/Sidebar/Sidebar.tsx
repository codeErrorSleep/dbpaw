import { useEffect, useState } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ConnectionList } from "./ConnectionList";
import { SavedQueriesList } from "./SavedQueriesList";
import { ConnectionForm, SavedQuery } from "@/services/api";

interface ActiveTableTarget {
  connectionId: number;
  database: string;
  table: string;
  schema?: string;
}

interface SidebarProps {
  onTableSelect?: (
    connection: string,
    database: string,
    table: string,
    connectionId: number,
    driver: string,
  ) => void;
  onConnect?: (form: ConnectionForm) => void;
  onCreateQuery?: (
    connectionId: number,
    databaseName: string,
    driver: string,
  ) => void;
  onExportTable?: (
    ctx: {
      connectionId: number;
      database: string;
      schema: string;
      table: string;
      driver: string;
    },
    format: "csv" | "json" | "sql",
    filePath: string,
  ) => void;
  onSelectSavedQuery: (query: SavedQuery) => void;
  lastUpdated?: number;
  activeTableTarget?: ActiveTableTarget;
}

export function Sidebar({
  onTableSelect,
  onConnect,
  onCreateQuery,
  onExportTable,
  onSelectSavedQuery,
  lastUpdated,
  activeTableTarget,
}: SidebarProps) {
  const [sidebarTab, setSidebarTab] = useState<"connections" | "queries">(
    "connections",
  );

  useEffect(() => {
    if (!activeTableTarget) return;
    setSidebarTab("connections");
  }, [activeTableTarget]);

  return (
    <div className="h-full flex flex-col bg-background border-r border-border">
      <Tabs
        value={sidebarTab}
        onValueChange={(value) =>
          setSidebarTab(value as "connections" | "queries")
        }
        className="h-full flex flex-col"
      >
        <TabsList className="w-full grid grid-cols-2 overflow-hidden">
          <TabsTrigger value="connections" className="min-w-0 truncate">
            Connections
          </TabsTrigger>
          <TabsTrigger value="queries" className="min-w-0 truncate">
            Queries
          </TabsTrigger>
        </TabsList>
        <div className="flex-1 overflow-hidden mt-2">
          <TabsContent value="connections" className="h-full m-0 border-0">
            <ConnectionList
              onTableSelect={onTableSelect}
              onConnect={onConnect}
              onCreateQuery={onCreateQuery}
              onExportTable={onExportTable}
              activeTableTarget={activeTableTarget}
            />
          </TabsContent>
          <TabsContent value="queries" className="h-full m-0 border-0">
            <SavedQueriesList
              onSelectQuery={onSelectSavedQuery}
              lastUpdated={lastUpdated}
            />
          </TabsContent>
        </div>
      </Tabs>
    </div>
  );
}
