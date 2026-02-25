import { useState } from "react";
import { formatDistanceToNow } from "date-fns";
import { History, MessageSquare, Trash2 } from "lucide-react";
import type { AIConversation } from "@/services/api";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/components/ui/utils";

export interface AIHistoryPopoverProps {
  conversations: AIConversation[];
  activeConversationId: number | null;
  onSelect: (conversationId: number) => void;
  onDelete: (conversationId: number) => void;
  disabled?: boolean;
}

export function AIHistoryPopover({
  conversations,
  activeConversationId,
  onSelect,
  onDelete,
  disabled,
}: AIHistoryPopoverProps) {
  const [open, setOpen] = useState(false);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="h-7 w-7 rounded-md"
          disabled={disabled}
          title="History"
          aria-label="Open conversation history"
        >
          <History className="h-4 w-4" />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[320px] p-2">
        <div className="px-2 py-1 text-xs font-medium text-muted-foreground">
          Conversation History
        </div>
        {conversations.length === 0 ? (
          <div className="px-2 py-8 text-center text-sm text-muted-foreground">
            No conversation yet
          </div>
        ) : (
          <ScrollArea className="max-h-[60vh]">
            <div className="space-y-1 pr-2">
              {conversations.map((conversation) => (
                <div
                  key={conversation.id}
                  className={cn(
                    "group flex cursor-pointer items-start gap-2 rounded-md border border-transparent px-2 py-2 transition-colors",
                    activeConversationId === conversation.id
                      ? "bg-accent text-accent-foreground"
                      : "hover:bg-muted/60",
                  )}
                  onClick={() => {
                    onSelect(conversation.id);
                    setOpen(false);
                  }}
                >
                  <MessageSquare className="mt-0.5 h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-xs font-medium">{conversation.title}</div>
                    <div className="text-[11px] text-muted-foreground">
                      {formatDistanceToNow(new Date(conversation.updatedAt), {
                        addSuffix: true,
                      })}
                    </div>
                  </div>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="h-6 w-6 shrink-0 rounded-md opacity-0 transition-opacity group-hover:opacity-100"
                    onClick={(event) => {
                      event.stopPropagation();
                      onDelete(conversation.id);
                    }}
                    title="Delete conversation"
                    aria-label={`Delete conversation ${conversation.title}`}
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </Button>
                </div>
              ))}
            </div>
          </ScrollArea>
        )}
      </PopoverContent>
    </Popover>
  );
}
