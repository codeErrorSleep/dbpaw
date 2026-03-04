import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import { Sparkles, Plus } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import {
  api,
  AIConversation,
  AIMessage,
  AIProviderConfig,
  isTauri,
} from "@/services/api";
import { isModKey } from "@/lib/keyboard";
import { toast } from "sonner";
import { AIHistoryPopover } from "./AIHistoryPopover";
import { ChatComposer } from "./chat/ChatComposer";
import { ChatMessageList } from "./chat/ChatMessageList";
import { type SelectedTableRef } from "./chat/TableSelector";
import { useTranslation } from "react-i18next";

interface AISidebarProps {
  connectionId?: number;
  database?: string;
  schemaOverview?: {
    tables: {
      schema: string;
      name: string;
      columns: { name: string; type: string }[];
    }[];
  };
}

interface AiChunkPayload {
  requestId: string;
  conversationId: number;
  chunk: string;
}

interface AiDonePayload {
  requestId: string;
  conversationId: number;
}

interface AiStartedPayload {
  requestId: string;
  conversationId: number;
  model: string;
}

interface AiErrorPayload {
  requestId: string;
  conversationId?: number;
  error: string;
}

export function AISidebar({
  connectionId,
  database,
  schemaOverview,
}: AISidebarProps) {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<AIProviderConfig[]>([]);
  const [selectedProviderId, setSelectedProviderId] = useState<string>("");
  const [conversations, setConversations] = useState<AIConversation[]>([]);
  const [activeConversationId, setActiveConversationId] = useState<
    number | null
  >(null);
  const [messages, setMessages] = useState<AIMessage[]>([]);
  const [input, setInput] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [streamingContent, setStreamingContent] = useState("");
  const [streamStatus, setStreamStatus] = useState("");
  const [selectedTables, setSelectedTables] = useState<SelectedTableRef[]>([]);
  const [availableTables, setAvailableTables] = useState<SelectedTableRef[]>(
    [],
  );

  const requestIdRef = useRef<string>("");
  const errorNotifiedRef = useRef(false);
  const streamQueueRef = useRef<string>("");
  const streamDrainTimerRef = useRef<ReturnType<typeof setInterval> | null>(
    null,
  );
  const streamFinalizeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );
  const isLoadingRef = useRef(false);
  const activeConversationIdRef = useRef<number | null>(null);
  const reloadConversationsRef = useRef<() => Promise<void>>(async () => {});
  const loadConversationRef = useRef<(conversationId: number) => Promise<void>>(
    async () => {},
  );

  const sortedConversations = useMemo(
    () =>
      [...conversations].sort((a, b) => (a.updatedAt < b.updatedAt ? 1 : -1)),
    [conversations],
  );

  const reloadProviders = async () => {
    try {
      const list = await api.ai.providers.list();
      const available = list.filter((p) => p.enabled);
      setProviders(available);
      const defaultProvider =
        available.find((p) => p.isDefault) || available[0];
      setSelectedProviderId(defaultProvider ? String(defaultProvider.id) : "");
    } catch (e) {
      console.error("Failed to load AI providers", e);
      setProviders([]);
    }
  };

  const reloadConversations = async () => {
    try {
      const list = await api.ai.conversations.list({ connectionId, database });
      setConversations(list);
      if (!activeConversationIdRef.current && list.length > 0) {
        setActiveConversationId(list[0].id);
      }
    } catch (e) {
      console.error("Failed to load AI conversations", e);
      setConversations([]);
    }
  };

  const loadConversation = async (conversationId: number) => {
    try {
      const detail = await api.ai.conversations.get(conversationId);
      setMessages(detail.messages);
      setActiveConversationId(conversationId);
      const hasAssistantReply = detail.messages.some(
        (m) => m.role === "assistant",
      );
      if (isLoadingRef.current && hasAssistantReply) {
        setIsLoading(false);
        setStreamStatus("");
        setStreamingContent("");
        streamQueueRef.current = "";
      }
    } catch (e) {
      toast.error(t("aiSidebar.errors.loadConversation"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  useEffect(() => {
    reloadProviders();
    reloadConversations();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connectionId, database]);

  useEffect(() => {
    setSelectedTables([]);
  }, [connectionId, database]);

  useEffect(() => {
    if (!connectionId) {
      setAvailableTables([]);
      return;
    }
    let cancelled = false;
    api.metadata
      .listTables(connectionId, database)
      .then((items) => {
        if (cancelled) return;
        const tables = items.map((t) => ({ schema: t.schema, name: t.name }));
        setAvailableTables(tables);
      })
      .catch((e) => {
        if (cancelled) return;
        console.error("Failed to load tables for AI selector", e);
        const fallback =
          schemaOverview?.tables?.map((t) => ({
            schema: t.schema,
            name: t.name,
          })) || [];
        setAvailableTables(fallback);
      });
    return () => {
      cancelled = true;
    };
  }, [connectionId, database, schemaOverview]);

  useEffect(() => {
    isLoadingRef.current = isLoading;
  }, [isLoading]);

  useEffect(() => {
    activeConversationIdRef.current = activeConversationId;
  }, [activeConversationId]);

  useEffect(() => {
    reloadConversationsRef.current = reloadConversations;
    loadConversationRef.current = loadConversation;
  });

  useEffect(() => {
    if (!activeConversationId) {
      setMessages([]);
      return;
    }
    void loadConversation(activeConversationId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeConversationId]);

  useEffect(() => {
    if (!isTauri()) return;

    let disposed = false;
    const unlistenFns: Array<() => void> = [];

    const registerListener = <T,>(
      event: string,
      handler: (evt: { payload: T }) => void,
    ) => {
      void listen<T>(event, handler)
        .then((unlisten) => {
          if (disposed) {
            unlisten();
            return;
          }
          unlistenFns.push(unlisten);
        })
        .catch((error) => {
          console.error(`Failed to register listener for ${event}`, error);
        });
    };

    registerListener<AiStartedPayload>("ai/started", (evt) => {
      if (evt.payload.requestId !== requestIdRef.current) return;
      setStreamStatus(
        t("aiSidebar.status.requestSent", { model: evt.payload.model }),
      );
    });

    registerListener<AiChunkPayload>("ai/chunk", (evt) => {
      if (evt.payload.requestId !== requestIdRef.current) return;
      setStreamStatus(t("aiSidebar.status.receiving"));
      streamQueueRef.current += evt.payload.chunk;
    });

    registerListener<AiDonePayload>("ai/done", (evt) => {
      if (evt.payload.requestId !== requestIdRef.current) return;
      setStreamStatus(t("aiSidebar.status.finalizing"));
      setActiveConversationId(evt.payload.conversationId);
      void reloadConversationsRef.current();
      void loadConversationRef.current(evt.payload.conversationId);

      const finish = () => {
        if (streamQueueRef.current.length > 0) {
          streamFinalizeTimerRef.current = setTimeout(finish, 20);
          return;
        }
        if (streamFinalizeTimerRef.current) {
          clearTimeout(streamFinalizeTimerRef.current);
          streamFinalizeTimerRef.current = null;
        }
        setIsLoading(false);
        setStreamingContent("");
        setStreamStatus("");
      };
      finish();
    });

    registerListener<AiErrorPayload>("ai/error", (evt) => {
      if (evt.payload.requestId !== requestIdRef.current) return;
      setIsLoading(false);
      setStreamingContent("");
      setStreamStatus("");
      streamQueueRef.current = "";
      if (streamFinalizeTimerRef.current) {
        clearTimeout(streamFinalizeTimerRef.current);
        streamFinalizeTimerRef.current = null;
      }
      errorNotifiedRef.current = true;
      toast.error(t("aiSidebar.errors.requestFailed"), {
        id: "ai-request-error",
        description: evt.payload.error,
      });
    });

    return () => {
      disposed = true;
      for (const unlisten of unlistenFns) {
        unlisten();
      }
      if (streamFinalizeTimerRef.current) {
        clearTimeout(streamFinalizeTimerRef.current);
        streamFinalizeTimerRef.current = null;
      }
    };
  }, [t]);

  useEffect(() => {
    if (!isLoading) {
      if (streamDrainTimerRef.current) {
        clearInterval(streamDrainTimerRef.current);
        streamDrainTimerRef.current = null;
      }
      return;
    }

    if (!streamDrainTimerRef.current) {
      streamDrainTimerRef.current = setInterval(() => {
        if (!streamQueueRef.current) return;
        const take = Math.min(2, streamQueueRef.current.length);
        const next = streamQueueRef.current.slice(0, take);
        streamQueueRef.current = streamQueueRef.current.slice(take);
        setStreamingContent((prev) => prev + next);
      }, 16);
    }

    return () => {
      if (streamDrainTimerRef.current) {
        clearInterval(streamDrainTimerRef.current);
        streamDrainTimerRef.current = null;
      }
    };
  }, [isLoading]);

  const handleSend = async () => {
    const text = input.trim();
    if (!text || isLoading) return;

    if (!selectedProviderId) {
      toast.error(t("aiSidebar.errors.providerMissing"));
      return;
    }

    const requestId = `ai-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    requestIdRef.current = requestId;
    errorNotifiedRef.current = false;

    const optimisticUserMsg: AIMessage = {
      id: Date.now(),
      conversationId: activeConversationId || 0,
      role: "user",
      content: text,
      createdAt: new Date().toISOString(),
    };

    setMessages((prev) => [...prev, optimisticUserMsg]);
    setInput("");
    setIsLoading(true);
    setStreamingContent("");
    setStreamStatus(t("aiSidebar.status.sending"));
    streamQueueRef.current = "";
    if (streamFinalizeTimerRef.current) {
      clearTimeout(streamFinalizeTimerRef.current);
      streamFinalizeTimerRef.current = null;
    }

    const request = {
      requestId,
      providerId: Number(selectedProviderId),
      conversationId: activeConversationId || undefined,
      scenario: "sql_generate",
      input: text,
      title: text.slice(0, 36),
      connectionId,
      database,
      schemaOverview,
      selectedTables,
    };

    try {
      let conversationIdToRefresh: number | null = null;
      if (activeConversationId) {
        const done = await api.ai.chat.continue(request);
        conversationIdToRefresh = done.conversationId;
      } else {
        const started = await api.ai.chat.start(request);
        setActiveConversationId(started.conversationId);
        conversationIdToRefresh = started.conversationId;
      }

      if (conversationIdToRefresh !== null) {
        await reloadConversations();
        await loadConversation(conversationIdToRefresh);
      }

      if (!isTauri() || requestIdRef.current === requestId) {
        setIsLoading(false);
        setStreamingContent("");
        setStreamStatus("");
        streamQueueRef.current = "";
        if (streamFinalizeTimerRef.current) {
          clearTimeout(streamFinalizeTimerRef.current);
          streamFinalizeTimerRef.current = null;
        }
      }
    } catch (e) {
      setIsLoading(false);
      setStreamingContent("");
      setStreamStatus("");
      streamQueueRef.current = "";
      if (streamFinalizeTimerRef.current) {
        clearTimeout(streamFinalizeTimerRef.current);
        streamFinalizeTimerRef.current = null;
      }
      if (!errorNotifiedRef.current) {
        toast.error(t("aiSidebar.errors.sendFailed"), {
          id: "ai-request-error",
          description: e instanceof Error ? e.message : String(e),
        });
      }
    }
  };

  const handleDeleteConversation = async (conversationId: number) => {
    try {
      await api.ai.conversations.delete(conversationId);
      if (activeConversationId === conversationId) {
        setActiveConversationId(null);
        setMessages([]);
      }
      await reloadConversations();
    } catch (e) {
      toast.error(t("aiSidebar.errors.deleteConversation"), {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key !== "Enter") return;
    if (e.shiftKey && !isModKey(e)) return;
    e.preventDefault();
    void handleSend();
  };

  const handleNewConversation = () => {
    if (isLoading) return;
    setActiveConversationId(null);
    setMessages([]);
    setStreamingContent("");
    setStreamStatus("");
    streamQueueRef.current = "";
  };

  return (
    <div className="flex h-full min-h-0 w-full min-w-0 flex-col overflow-hidden border-l border-border/80 bg-background">
      <div className="flex h-9 shrink-0 min-w-0 items-center justify-between border-b border-border/70 px-3">
        <div className="flex min-w-0 items-center gap-2">
          <Sparkles className="h-4 w-4 text-primary" />
          <h2 className="truncate text-sm font-semibold text-foreground">
            {t("aiSidebar.title")}
          </h2>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <AIHistoryPopover
            conversations={sortedConversations}
            activeConversationId={activeConversationId}
            onSelect={(conversationId) =>
              setActiveConversationId(conversationId)
            }
            onDelete={(conversationId) => {
              void handleDeleteConversation(conversationId);
            }}
            disabled={isLoading}
          />
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="h-7 w-7 rounded-md"
            title={t("aiSidebar.newChat")}
            aria-label={t("aiSidebar.startNewChat")}
            onClick={handleNewConversation}
            disabled={isLoading}
          >
            <Plus className="h-4 w-4" />
          </Button>
        </div>
      </div>

      <ChatMessageList
        messages={messages}
        isLoading={isLoading}
        streamingContent={streamingContent}
        streamStatus={streamStatus}
      />

      <ChatComposer
        input={input}
        onInputChange={setInput}
        onKeyDown={handleKeyDown}
        onSend={() => {
          void handleSend();
        }}
        isLoading={isLoading}
        providers={providers}
        selectedProviderId={selectedProviderId}
        onProviderChange={setSelectedProviderId}
        availableTables={availableTables}
        selectedTables={selectedTables}
        onSelectedTablesChange={setSelectedTables}
      />
    </div>
  );
}
