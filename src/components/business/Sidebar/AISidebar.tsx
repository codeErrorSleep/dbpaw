import { useEffect, useMemo, useRef, useState } from "react";
import { Send, Sparkles, Trash2, Plus } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  api,
  AIConversation,
  AIMessage,
  AIProviderConfig,
  AIProviderType,
  isTauri,
} from "@/services/api";
import { isModKey } from "@/lib/keyboard";
import { toast } from "sonner";

interface AISidebarProps {
  connectionId?: number;
  database?: string;
  schemaOverview?: {
    tables: { schema: string; name: string; columns: { name: string; type: string }[] }[];
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

const isAIProviderType = (value: string): value is AIProviderType =>
  value === "openai" || value === "kimi" || value === "glm";

export function AISidebar({ connectionId, database, schemaOverview }: AISidebarProps) {
  const [providers, setProviders] = useState<AIProviderConfig[]>([]);
  const [selectedProviderId, setSelectedProviderId] = useState<string>("");
  const [conversations, setConversations] = useState<AIConversation[]>([]);
  const [activeConversationId, setActiveConversationId] = useState<number | null>(null);
  const [messages, setMessages] = useState<AIMessage[]>([]);
  const [input, setInput] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [streamingContent, setStreamingContent] = useState("");
  const [streamStatus, setStreamStatus] = useState("");

  const requestIdRef = useRef<string>("");
  const errorNotifiedRef = useRef(false);

  const sortedConversations = useMemo(
    () => [...conversations].sort((a, b) => (a.updatedAt < b.updatedAt ? 1 : -1)),
    [conversations],
  );

  const reloadProviders = async () => {
    try {
      const list = await api.ai.providers.list();
      const available = list.filter(
        (p) => p.enabled && isAIProviderType(p.providerType),
      );
      setProviders(available);
      const defaultProvider = available.find((p) => p.isDefault) || available[0];
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
      if (!activeConversationId && list.length > 0) {
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
    } catch (e) {
      toast.error("Failed to load conversation", {
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
    if (!activeConversationId) {
      setMessages([]);
      return;
    }
    loadConversation(activeConversationId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeConversationId]);

  useEffect(() => {
    if (!isTauri()) return;

    let unlistenChunk: (() => void) | undefined;
    let unlistenStarted: (() => void) | undefined;
    let unlistenDone: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;

    listen<AiStartedPayload>("ai.started", (evt) => {
      if (evt.payload.requestId !== requestIdRef.current) return;
      setStreamStatus(`Request sent (${evt.payload.model}), waiting for first token...`);
    }).then((f) => (unlistenStarted = f));

    listen<AiChunkPayload>("ai.chunk", (evt) => {
      if (evt.payload.requestId !== requestIdRef.current) return;
      setStreamStatus("Receiving response...");
      setStreamingContent((prev) => prev + evt.payload.chunk);
    }).then((f) => (unlistenChunk = f));

    listen<AiDonePayload>("ai.done", (evt) => {
      if (evt.payload.requestId !== requestIdRef.current) return;
      setIsLoading(false);
      setStreamingContent("");
      setStreamStatus("");
      setActiveConversationId(evt.payload.conversationId);
      reloadConversations();
      loadConversation(evt.payload.conversationId);
    }).then((f) => (unlistenDone = f));

    listen<AiErrorPayload>("ai.error", (evt) => {
      if (evt.payload.requestId !== requestIdRef.current) return;
      setIsLoading(false);
      setStreamingContent("");
      setStreamStatus("");
      errorNotifiedRef.current = true;
      toast.error("AI request failed", {
        id: "ai-request-error",
        description: evt.payload.error,
      });
    }).then((f) => (unlistenError = f));

    return () => {
      if (unlistenChunk) unlistenChunk();
      if (unlistenStarted) unlistenStarted();
      if (unlistenDone) unlistenDone();
      if (unlistenError) unlistenError();
    };
  }, [activeConversationId]);

  const handleSend = async () => {
    const text = input.trim();
    if (!text || isLoading) return;

    if (!selectedProviderId) {
      toast.error("Please configure and select an AI provider in Settings.");
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
    setStreamStatus("Sending request...");

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
    };

    try {
      if (activeConversationId) {
        await api.ai.chat.continue(request);
      } else {
        const started = await api.ai.chat.start(request);
        setActiveConversationId(started.conversationId);
      }

      if (!isTauri()) {
        setIsLoading(false);
        setStreamingContent("");
        setStreamStatus("");
        reloadConversations();
        if (activeConversationId) {
          loadConversation(activeConversationId);
        }
      }
    } catch (e) {
      setIsLoading(false);
      setStreamingContent("");
      setStreamStatus("");
      if (!errorNotifiedRef.current) {
        toast.error("Failed to send AI message", {
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
      reloadConversations();
    } catch (e) {
      toast.error("Failed to delete conversation", {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key !== "Enter") return;
    if (e.shiftKey && !isModKey(e)) return;
    e.preventDefault();
    handleSend();
  };

  return (
    <div className="h-full min-h-0 flex flex-col overflow-hidden bg-background border-l border-border">
      <div className="px-3 py-2 border-b border-border flex items-center gap-2 h-10 shrink-0">
        <Sparkles className="w-4 h-4 text-primary" />
        <h2 className="font-semibold text-sm text-foreground">AI Assistant</h2>
      </div>

      <div className="p-3 border-b border-border space-y-2 shrink-0">
        <Select value={selectedProviderId} onValueChange={setSelectedProviderId}>
          <SelectTrigger className="h-8">
            <SelectValue placeholder="Select AI provider" />
          </SelectTrigger>
          <SelectContent>
            {providers.map((p) => (
              <SelectItem key={p.id} value={String(p.id)}>
                {p.name} ({p.model})
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            className="h-7"
            onClick={() => {
              setActiveConversationId(null);
              setMessages([]);
            }}
          >
            <Plus className="w-3 h-3 mr-1" /> New
          </Button>
        </div>
      </div>

      <ScrollArea className="h-28 border-b border-border shrink-0">
        <div className="p-2 space-y-1">
          {sortedConversations.map((c) => (
            <div
              key={c.id}
              className={`group flex items-center justify-between rounded px-2 py-1 text-xs cursor-pointer ${
                activeConversationId === c.id
                  ? "bg-accent text-accent-foreground"
                  : "text-muted-foreground hover:bg-muted/60"
              }`}
              onClick={() => setActiveConversationId(c.id)}
            >
              <span className="truncate">{c.title}</span>
              <Button
                variant="ghost"
                size="sm"
                className="h-5 w-5 p-0 opacity-0 group-hover:opacity-100"
                onClick={(e) => {
                  e.stopPropagation();
                  handleDeleteConversation(c.id);
                }}
              >
                <Trash2 className="w-3 h-3" />
              </Button>
            </div>
          ))}
        </div>
      </ScrollArea>

      <div className="flex-1 min-h-0 overflow-hidden">
        <ScrollArea className="h-full">
          <div className="p-4 space-y-4">
            {messages.map((message) => (
              <div
                key={`${message.id}-${message.createdAt}`}
                className={`flex ${message.role === "user" ? "justify-end" : "justify-start"}`}
              >
                <div
                  className={`max-w-[85%] rounded-lg px-4 py-2 ${
                    message.role === "user"
                      ? "bg-primary text-primary-foreground"
                      : "bg-muted text-foreground border border-border"
                  }`}
                >
                  <div className="text-sm whitespace-pre-wrap">{message.content}</div>
                </div>
              </div>
            ))}

            {isLoading && (
              <div className="flex justify-start">
                <div className="bg-muted text-foreground border border-border rounded-lg px-4 py-2 max-w-[85%]">
                  <div className="text-sm whitespace-pre-wrap">
                    {streamingContent || streamStatus || "Thinking..."}
                  </div>
                </div>
              </div>
            )}
          </div>
        </ScrollArea>
      </div>

      <div className="p-4 border-t border-border bg-background shrink-0">
        <div className="flex gap-2">
          <Textarea
            placeholder="Describe SQL to generate or optimize..."
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            className="resize-none bg-background"
            rows={3}
          />
          <Button
            onClick={handleSend}
            disabled={!input.trim() || isLoading || !selectedProviderId}
            className="self-end"
          >
            <Send className="w-4 h-4" />
          </Button>
        </div>
      </div>
    </div>
  );
}
