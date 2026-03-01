import type { KeyboardEventHandler } from "react";
import { Send } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/components/ui/utils";
import type { AIProviderConfig } from "@/services/api";
import { TableSelector, type SelectedTableRef } from "./TableSelector";

export interface ChatComposerProps {
  input: string;
  onInputChange: (value: string) => void;
  onKeyDown: KeyboardEventHandler<HTMLTextAreaElement>;
  onSend: () => void;
  isLoading: boolean;
  providers: AIProviderConfig[];
  selectedProviderId: string;
  onProviderChange: (value: string) => void;
  availableTables: SelectedTableRef[];
  selectedTables: SelectedTableRef[];
  onSelectedTablesChange: (next: SelectedTableRef[]) => void;
}

export function ChatComposer({
  input,
  onInputChange,
  onKeyDown,
  onSend,
  isLoading,
  providers,
  selectedProviderId,
  onProviderChange,
  availableTables,
  selectedTables,
  onSelectedTablesChange,
}: ChatComposerProps) {
  const isSendDisabled = !input.trim() || isLoading || !selectedProviderId;

  return (
    <div className="shrink-0 min-w-0 border-t border-border/60 px-3 py-2.5">
      <div className="min-w-0 rounded-xl border border-border/70 bg-background/95 px-2 py-1.5 transition-[box-shadow,border-color,background-color] duration-200 focus-within:border-primary/45 focus-within:shadow-[0_0_0_1px_color-mix(in_srgb,var(--color-primary)_55%,transparent),0_0_20px_color-mix(in_srgb,var(--color-primary)_20%,transparent)]">
        <Textarea
          placeholder="Describe SQL to generate or optimize..."
          value={input}
          onChange={(e) => onInputChange(e.target.value)}
          onKeyDown={onKeyDown}
          className="min-h-[76px] w-full min-w-0 resize-none border-0 bg-transparent px-2 py-1 shadow-none focus-visible:ring-0"
          rows={3}
        />
        <div className="mt-1.5 flex min-w-0 flex-wrap items-center gap-2 px-1 pb-0.5">
          <div className="order-1 min-w-[150px] flex-1 basis-[180px] sm:min-w-0 sm:basis-0">
            <Select value={selectedProviderId} onValueChange={onProviderChange}>
              <SelectTrigger className="h-8 w-full min-w-0 border-border/60 bg-muted/30 text-xs">
                <SelectValue placeholder="Select AI provider" />
              </SelectTrigger>
              <SelectContent align="start">
                {providers.map((p) => (
                  <SelectItem key={p.id} value={String(p.id)}>
                    {p.name} / {p.model}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          {availableTables.length ? (
            <div className="order-3 min-w-0 basis-full sm:order-2 sm:flex-1 sm:basis-0">
              <TableSelector
                tables={availableTables}
                value={selectedTables}
                onChange={onSelectedTablesChange}
                disabled={isLoading}
              />
            </div>
          ) : null}
          <Button
            type="button"
            onClick={onSend}
            disabled={isSendDisabled}
            size="icon"
            className={cn(
              "order-2 h-8 w-8 shrink-0 rounded-md transition-all sm:order-3",
              isLoading &&
                "cursor-wait bg-gradient-to-br from-primary/90 to-primary/65 text-primary-foreground",
              !isLoading &&
                !isSendDisabled &&
                "bg-gradient-to-br from-primary to-primary/75 text-primary-foreground shadow-sm hover:brightness-110",
              isSendDisabled && "bg-muted text-muted-foreground opacity-70",
            )}
            title="Send"
            aria-label="Send message"
          >
            <Send className={cn("h-4 w-4", isLoading && "animate-pulse")} />
          </Button>
        </div>
      </div>
    </div>
  );
}
