import type { ComponentProps } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Copy } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { cn } from "@/components/ui/utils";

export interface AIMarkdownMessageProps {
  content: string;
  className?: string;
}

function CodeBlock({
  inline,
  className,
  children,
  ...props
}: ComponentProps<"code"> & { inline?: boolean }) {
  const rawCode = String(children ?? "").replace(/\n$/, "");

  if (inline) {
    return (
      <code
        className={cn(
          "rounded bg-muted px-1.5 py-0.5 font-mono text-[0.85em] text-foreground",
          className,
        )}
        {...props}
      >
        {children}
      </code>
    );
  }

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(rawCode);
      toast.success("Copied");
    } catch {
      toast.error("Copy failed", {
        description: "Please check clipboard permissions",
      });
    }
  };

  return (
    <div className="group relative my-3">
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="absolute top-2 right-2 z-10 h-7 rounded-md px-2 text-xs opacity-0 transition-opacity group-hover:opacity-100"
        onClick={handleCopy}
      >
        <Copy className="h-3.5 w-3.5" />
        Copy
      </Button>
      <pre className="w-full min-w-0 max-w-full overflow-x-auto rounded-lg border border-border/70 bg-muted/40 p-3 text-sm leading-6">
        <code
          className={cn("block whitespace-pre-wrap break-words font-mono text-foreground", className)}
          {...props}
        >
          {rawCode}
        </code>
      </pre>
    </div>
  );
}

export function AIMarkdownMessage({ content, className }: AIMarkdownMessageProps) {
  return (
    <div
      className={cn(
        "w-full min-w-0 max-w-full overflow-hidden text-sm leading-7 text-foreground",
        className,
      )}
    >
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          h1: ({ children }) => <h1 className="text-lg">{children}</h1>,
          h2: ({ children }) => <h2 className="text-base">{children}</h2>,
          h3: ({ children }) => <h3 className="text-sm">{children}</h3>,
          p: ({ children }) => <p className="break-words">{children}</p>,
          ul: ({ children }) => <ul className="list-disc break-words pl-6">{children}</ul>,
          ol: ({ children }) => <ol className="list-decimal break-words pl-6">{children}</ol>,
          li: ({ children }) => <li className="break-words">{children}</li>,
          blockquote: ({ children }) => (
            <blockquote className="my-3 break-words border-l-2 border-border pl-3 text-muted-foreground">
              {children}
            </blockquote>
          ),
          table: ({ children }) => (
            <div className="my-3 w-full min-w-0 overflow-x-auto">
              <table className="w-full min-w-0 table-fixed border-collapse text-left text-xs">
                {children}
              </table>
            </div>
          ),
          th: ({ children }) => (
            <th className="break-words border border-border/70 bg-muted/50 px-2 py-1.5 font-semibold">
              {children}
            </th>
          ),
          td: ({ children }) => (
            <td className="break-words border border-border/70 px-2 py-1.5 align-top">
              {children}
            </td>
          ),
          code: CodeBlock,
          a: ({ children, href }) => (
            <a
              href={href}
              className="text-primary underline underline-offset-2"
              target="_blank"
              rel="noreferrer"
            >
              {children}
            </a>
          ),
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
