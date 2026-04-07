import { useEffect, useState } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ConnectionList } from "./ConnectionList";
import { SavedQueriesList } from "./SavedQueriesList";
import { ConnectionForm, SavedQuery } from "@/services/api";
import { useTranslation } from "react-i18next";

interface ActiveTableTarget {
  connectionId: number;
  database: string;
  table: string;
  schema?: string;
}

interface SidebarRevealRequest extends ActiveTableTarget {
  id: number;
}

interface SidebarProps {
  onTableSelect?: (
    connection: string,
    database: string,
    table: string,
    connectionId: number,
    driver: string,
    schema?: string,
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
  sidebarRevealRequest?: SidebarRevealRequest;
  layoutMode?: "tabs" | "tree";
}

export function Sidebar({
  onTableSelect,
  onConnect,
  onCreateQuery,
  onExportTable,
  onSelectSavedQuery,
  lastUpdated,
  activeTableTarget,
  sidebarRevealRequest,
  layoutMode = "tabs",
}: SidebarProps) {
  const { t } = useTranslation();
  const [sidebarTab, setSidebarTab] = useState<"connections" | "queries">(
    "connections",
  );

  useEffect(() => {
    if (!sidebarRevealRequest) return;
    setSidebarTab("connections");
  }, [sidebarRevealRequest]);

  if (layoutMode === "tree") {
    return (
      <div className="h-full flex flex-col bg-background border-r border-border">
        <ConnectionList
          onTableSelect={onTableSelect}
          onConnect={onConnect}
          onCreateQuery={onCreateQuery}
          onExportTable={onExportTable}
          activeTableTarget={activeTableTarget}
          sidebarRevealRequest={sidebarRevealRequest}
          onSelectSavedQuery={onSelectSavedQuery}
          lastUpdated={lastUpdated}
          showSavedQueriesInTree
        />
      </div>
    );
  }

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
          <TabsTrigger value="connections" className="min-w-0 max-w-none truncate">
            {t("sidebar.tabs.connections")}
          </TabsTrigger>
          <TabsTrigger value="queries" className="min-w-0 max-w-none truncate">
            {t("sidebar.tabs.queries")}
          </TabsTrigger>
        </TabsList>
        <div className="flex-1 overflow-hidden mt-2">
          <TabsContent
            value="connections"
            forceMount
            className="h-full m-0 border-0"
          >
            <ConnectionList
              onTableSelect={onTableSelect}
              onConnect={onConnect}
              onCreateQuery={onCreateQuery}
              onExportTable={onExportTable}
              activeTableTarget={activeTableTarget}
              sidebarRevealRequest={sidebarRevealRequest}
            />
          </TabsContent>
          <TabsContent
            value="queries"
            forceMount
            className="h-full m-0 border-0"
          >
            <SavedQueriesList
              onSelectQuery={onSelectSavedQuery}
              onCreateQuery={onCreateQuery}
              lastUpdated={lastUpdated}
            />
          </TabsContent>
        </div>
      </Tabs>
    </div>
  );
}
