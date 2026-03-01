import { AIMarkdownMessage } from "../AIMarkdownMessage";

export interface ChatTypingIndicatorProps {
  streamingContent: string;
  streamStatus: string;
}

export function ChatTypingIndicator({
  streamingContent,
  streamStatus,
}: ChatTypingIndicatorProps) {
  const hasStreamingText = Boolean(
    streamingContent?.trim() || streamStatus?.trim(),
  );

  if (hasStreamingText) {
    return (
      <div className="min-w-0 w-full max-w-full">
        <AIMarkdownMessage content={streamingContent || streamStatus} />
      </div>
    );
  }

  return (
    <div className="min-w-0 w-full max-w-full">
      <div className="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-muted/25 px-3 py-2 text-sm text-muted-foreground">
        <span>Thinking</span>
        <span className="inline-flex items-end gap-1 leading-none">
          <span className="animate-bounce">●</span>
          <span className="animate-bounce delay-100">●</span>
          <span className="animate-bounce delay-200">●</span>
        </span>
      </div>
    </div>
  );
}
