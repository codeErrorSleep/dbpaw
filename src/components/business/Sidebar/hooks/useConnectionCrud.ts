import { useState } from "react";
import { api } from "@/services/api";
import type { SavedConnection } from "@/services/api";
import type { Connection, DatabaseInfo } from "../connection-list/types";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import {
  mergeConnections,
  sanitizeConnectionErrorMessage,
  buildFormFromConnection,
} from "../connection-list/helpers";
import { errorMessage } from "@/lib/errors";

export const mapSavedConnection = (
  c: SavedConnection,
  fallbackName: string,
): Connection => ({
  id: String(c.id),
  name: c.name || fallbackName,
  type: (c.dbType as Connection["type"]) || "postgres",
  host: c.host || "",
  port: String(c.port || ""),
  database: c.database || "",
  username: c.username || "",
  ssl: c.ssl || false,
  sslMode: c.sslMode || "require",
  sslCaCert: c.sslCaCert || "",
  filePath: c.filePath || "",
  sshEnabled: c.sshEnabled || false,
  sshHost: c.sshHost || "",
  sshPort: c.sshPort || 22,
  sshUsername: c.sshUsername || "root",
  sshPassword: c.sshPassword || "",
  sshKeyPath: c.sshKeyPath || "",
  proxy: c.proxy ?? undefined,
  mode: c.mode || undefined,
  seedNodes: c.seedNodes || [],
  sentinels: c.sentinels || [],
  connectTimeoutMs: c.connectTimeoutMs || undefined,
  serviceName: c.serviceName || undefined,
  sentinelPassword: c.sentinelPassword || "",
  authMode: c.authMode || "none",
  apiKeyId: c.apiKeyId || "",
  apiKeySecret: c.apiKeySecret || "",
  apiKeyEncoded: c.apiKeyEncoded || "",
  cloudId: c.cloudId || "",
  authSource: c.authSource || "",
  isConnected: false,
  connectState: "idle",
  connectError: undefined,
  databases: [],
});

export function useConnectionCrud(params: {
  setExpandedConnections: (fn: (prev: Set<string>) => Set<string>) => void;
  setExpandedDatabases: (fn: (prev: Set<string>) => Set<string>) => void;
  setExpandedSchemas: (fn: (prev: Set<string>) => Set<string>) => void;
  setExpandedTables: (fn: (prev: Set<string>) => Set<string>) => void;
  listDatabases: (connection: Connection) => Promise<string[]>;
}) {
  const { t } = useTranslation();
  const {
    setExpandedConnections,
    setExpandedDatabases,
    setExpandedSchemas,
    setExpandedTables,
    listDatabases,
  } = params;

  const [connections, setConnections] = useState<Connection[]>([]);
  const [isLoadingConnections, setIsLoadingConnections] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [deleteTargetConnectionId, setDeleteTargetConnectionId] = useState<
    string | null
  >(null);

  const fetchConnections = async () => {
    setIsLoadingConnections(true);
    try {
      const conns = await api.connections.list();
      const mapped = conns.map((c) =>
        mapSavedConnection(c, t("common.unknown")),
      );
      setConnections((prev) => mergeConnections(mapped, prev));
    } catch (e) {
      const message = errorMessage(e);
      console.error("listConnections failed", message);
      toast.error(t("connection.toast.loadConnectionsFailed"), {
        description: message,
      });
    } finally {
      setIsLoadingConnections(false);
    }
  };

  const fetchAndSetDatabases = async (
    connectionId: string,
  ): Promise<boolean> => {
    try {
      const current = connections.find((conn) => conn.id === connectionId);
      if (!current) return false;

      let databases: DatabaseInfo[];
      if (current.type === "redis") {
        const redisDbs = await api.redis.listDatabases(Number(current.id));
        databases = redisDbs.map((db) => ({
          name: db.name,
          schemas: [],
          tables: [],
          routines: [],
          redisKeyCount: db.keyCount,
        }));
      } else {
        const dbNames = await listDatabases(current);
        databases = dbNames.map((name) => ({
          name,
          schemas: [],
          tables: [],
          routines: [],
        }));
      }

      setConnections((prev) =>
        prev.map((conn) => {
          if (conn.id !== connectionId) return conn;
          return {
            ...conn,
            isConnected: true,
            connectState: "success" as const,
            connectError: undefined,
            databases,
          };
        }),
      );
      return true;
    } catch (e) {
      const message = errorMessage(e);
      const sanitizedMessage = sanitizeConnectionErrorMessage(message);
      console.error("listDatabasesById failed", message);
      setConnections((prev) =>
        prev.map((conn) => {
          if (conn.id !== connectionId) return conn;
          return {
            ...conn,
            isConnected: false,
            connectState: "error" as const,
            connectError: sanitizedMessage || message,
            databases: [],
          };
        }),
      );
      toast.error(t("connection.toast.loadDatabasesFailed"), {
        description: sanitizedMessage || message,
      });
      return false;
    }
  };

  const connectConnection = async (
    connectionId: string,
    options?: { resetTree?: boolean },
  ) => {
    const target = connections.find((conn) => conn.id === connectionId);
    if (!target || target.connectState === "connecting") return;

    if (options?.resetTree) {
      setExpandedConnections((prev) => {
        const next = new Set(prev);
        next.delete(connectionId);
        return next;
      });
      setExpandedDatabases((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
      setExpandedSchemas((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
      setExpandedTables((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
    }

    setConnections((prev) =>
      prev.map((conn) => {
        if (conn.id !== connectionId) return conn;
        return {
          ...conn,
          isConnected: false,
          connectState: "connecting" as const,
          connectError: undefined,
          databases: options?.resetTree ? [] : conn.databases,
        };
      }),
    );

    const ok = await fetchAndSetDatabases(connectionId);
    if (ok) {
      setExpandedConnections((prev) => {
        const next = new Set(prev);
        next.add(connectionId);
        return next;
      });
      return;
    }

    setExpandedConnections((prev) => {
      const next = new Set(prev);
      next.delete(connectionId);
      return next;
    });
  };

  const clearConnectionTreeCache = (connectionId: string) => {
    setConnections((prev) =>
      prev.map((conn) =>
        conn.id === connectionId ? { ...conn, databases: [] } : conn,
      ),
    );
    setExpandedDatabases(
      (prev) =>
        new Set([...prev].filter((key) => !key.startsWith(`${connectionId}-`))),
    );
    setExpandedSchemas(
      (prev) =>
        new Set([...prev].filter((key) => !key.startsWith(`${connectionId}-`))),
    );
    setExpandedTables(
      (prev) =>
        new Set([...prev].filter((key) => !key.startsWith(`${connectionId}-`))),
    );
  };

  const handleReconnect = async (connectionId: string) => {
    await connectConnection(connectionId, { resetTree: true });
  };

  const buildDuplicateConnectionName = (sourceName: string) => {
    const baseName = `${sourceName}-${t("connection.menu.copy")}`;
    let candidate = baseName;
    let counter = 2;
    while (connections.some((conn) => conn.name === candidate)) {
      candidate = `${baseName}-${counter}`;
      counter += 1;
    }
    return candidate;
  };

  const handleDuplicateConnection = async (connectionId: string) => {
    const source = connections.find((conn) => conn.id === connectionId);
    if (!source) return;

    const duplicateName = buildDuplicateConnectionName(
      source.name || t("common.unknown"),
    );
    const duplicateForm = buildFormFromConnection(source, {
      name: duplicateName,
    });

    try {
      const res = await api.connections.create(duplicateForm);
      setConnections((prev) => [
        mapSavedConnection(res, t("common.unknown")),
        ...prev,
      ]);
      toast.success(t("connection.toast.duplicateSuccess"));
    } catch (e) {
      toast.error(t("connection.toast.duplicateFailed"), {
        description: errorMessage(e),
      });
    }
  };

  const handleDeleteConnection = async (connectionId: string) => {
    setIsDeleting(true);
    try {
      await api.connections.delete(Number(connectionId));
      setConnections((prev) => prev.filter((conn) => conn.id !== connectionId));
      setExpandedConnections((prev) => {
        const next = new Set(prev);
        next.delete(connectionId);
        return next;
      });
      setExpandedDatabases((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
      setExpandedSchemas((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
      setExpandedTables((prev) => {
        const next = new Set(
          [...prev].filter((key) => !key.startsWith(`${connectionId}-`)),
        );
        return next;
      });
      setDeleteTargetConnectionId(null);
    } catch (e) {
      console.error(
        "deleteConnection failed",
        errorMessage(e),
      );
    } finally {
      setIsDeleting(false);
    }
  };

  return {
    connections,
    setConnections,
    isLoadingConnections,
    isDeleting,
    setIsDeleting,
    deleteTargetConnectionId,
    setDeleteTargetConnectionId,
    fetchConnections,
    connectConnection,
    fetchAndSetDatabases,
    clearConnectionTreeCache,
    handleReconnect,
    handleDuplicateConnection,
    handleDeleteConnection,
    buildDuplicateConnectionName,
  };
}
