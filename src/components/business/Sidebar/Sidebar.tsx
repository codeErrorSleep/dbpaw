import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ConnectionList } from "./ConnectionList";
import { SavedQueriesList } from "./SavedQueriesList";
import { ConnectionForm, SavedQuery } from "@/services/api";

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
  onSelectSavedQuery: (query: SavedQuery) => void;
}

export function Sidebar({
  onTableSelect,
  onConnect,
  onCreateQuery,
  onSelectSavedQuery,
}: SidebarProps) {
  return (
    <div className="h-full flex flex-col bg-background border-r border-border">
        <Tabs defaultValue="connections" className="h-full flex flex-col">
            <TabsList className="w-full grid grid-cols-2">
                <TabsTrigger value="connections">Connections</TabsTrigger>
                <TabsTrigger value="queries">Queries</TabsTrigger>
            </TabsList>
            <div className="flex-1 overflow-hidden mt-2">
                <TabsContent value="connections" className="h-full m-0 border-0">
                    <ConnectionList
                        onTableSelect={onTableSelect}
                        onConnect={onConnect}
                        onCreateQuery={onCreateQuery}
                    />
                </TabsContent>
                <TabsContent value="queries" className="h-full m-0 border-0">
                    <SavedQueriesList onSelectQuery={onSelectSavedQuery} />
                </TabsContent>
            </div>
        </Tabs>
    </div>
  );
}
