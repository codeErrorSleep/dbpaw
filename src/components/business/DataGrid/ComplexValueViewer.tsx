import { useState } from "react";
import { Copy, Check, ChevronRight, ChevronDown } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";

interface ComplexValueViewerProps {
  value: unknown;
  columnName: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type TabId = "json" | "tree" | "table";

// --- Tree View ---

function TreeNode({
  label,
  value,
  depth = 0,
}: {
  label: string;
  value: unknown;
  depth?: number;
}) {
  const [expanded, setExpanded] = useState(depth < 2);
  const isComplex = value !== null && value !== undefined && typeof value === "object";
  const isArr = Array.isArray(value);

  if (!isComplex) {
    const isNull = value === null;
    const isStr = typeof value === "string";
    const isBool = typeof value === "boolean";
    const isNum = typeof value === "number";
    return (
      <div
        className="flex items-baseline py-[2px] text-xs font-mono leading-5"
        style={{ paddingLeft: depth * 14 + 18 }}
      >
        <span className="text-blue-500 dark:text-blue-400 mr-1 shrink-0">{label}</span>
        <span className="text-muted-foreground mr-1 shrink-0">:</span>
        <span
          className={
            isNull
              ? "text-muted-foreground italic"
              : isStr
                ? "text-green-600 dark:text-green-400"
                : isBool || isNum
                  ? "text-orange-500 dark:text-orange-400"
                  : "text-foreground"
          }
        >
          {isNull ? "null" : isStr ? `"${value}"` : String(value)}
        </span>
      </div>
    );
  }

  const entries = isArr
    ? (value as unknown[]).map((v, i) => [String(i), v] as [string, unknown])
    : Object.entries(value as Record<string, unknown>);

  return (
    <div>
      <div
        className="flex items-center py-[2px] text-xs font-mono leading-5 cursor-pointer hover:bg-muted/60 rounded select-none"
        style={{ paddingLeft: depth * 14 }}
        onClick={() => setExpanded((v) => !v)}
      >
        <span className="w-[18px] shrink-0 text-muted-foreground flex items-center">
          {expanded ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
        </span>
        <span className="text-blue-500 dark:text-blue-400 mr-1">{label}</span>
        <span className="text-muted-foreground mr-1">:</span>
        <span className="text-muted-foreground">
          {isArr ? `[ ${entries.length} ]` : `{ ${entries.length} }`}
        </span>
      </div>
      {expanded &&
        entries.map(([k, v]) => (
          <TreeNode key={k} label={k} value={v} depth={depth + 1} />
        ))}
    </div>
  );
}

// --- Table View ---

function cellText(value: unknown): string {
  if (value === null || value === undefined) return "null";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function TableView({ value }: { value: unknown }) {
  if (Array.isArray(value)) {
    const arr = value as unknown[];
    const allObjects =
      arr.length > 0 &&
      arr.every((item) => item !== null && typeof item === "object" && !Array.isArray(item));

    const keys = allObjects
      ? Array.from(new Set(arr.flatMap((item) => Object.keys(item as object))))
      : null;

    return (
      <table className="text-xs font-mono w-full border-collapse">
        <thead>
          <tr className="border-b border-border bg-muted/60">
            {keys ? (
              keys.map((k) => (
                <th key={k} className="text-left px-3 py-1.5 text-muted-foreground font-medium">
                  {k}
                </th>
              ))
            ) : (
              <>
                <th className="text-left px-3 py-1.5 text-muted-foreground font-medium w-14">#</th>
                <th className="text-left px-3 py-1.5 text-muted-foreground font-medium">value</th>
              </>
            )}
          </tr>
        </thead>
        <tbody>
          {arr.map((row, i) => (
            <tr key={i} className="border-b border-border/40 hover:bg-muted/40 transition-colors">
              {keys ? (
                keys.map((k) => {
                  const v = (row as Record<string, unknown>)[k];
                  return (
                    <td
                      key={k}
                      className={`px-3 py-1.5 ${v === null ? "text-muted-foreground italic" : ""}`}
                    >
                      {cellText(v)}
                    </td>
                  );
                })
              ) : (
                <>
                  <td className="px-3 py-1.5 text-muted-foreground">{i}</td>
                  <td className={`px-3 py-1.5 ${row === null ? "text-muted-foreground italic" : ""}`}>
                    {cellText(row)}
                  </td>
                </>
              )}
            </tr>
          ))}
        </tbody>
      </table>
    );
  }

  if (value !== null && typeof value === "object") {
    return (
      <table className="text-xs font-mono w-full border-collapse">
        <thead>
          <tr className="border-b border-border bg-muted/60">
            <th className="text-left px-3 py-1.5 text-muted-foreground font-medium w-2/5">key</th>
            <th className="text-left px-3 py-1.5 text-muted-foreground font-medium">value</th>
          </tr>
        </thead>
        <tbody>
          {Object.entries(value as Record<string, unknown>).map(([k, v]) => (
            <tr key={k} className="border-b border-border/40 hover:bg-muted/40 transition-colors">
              <td className="px-3 py-1.5 text-blue-500 dark:text-blue-400">{k}</td>
              <td className={`px-3 py-1.5 ${v === null ? "text-muted-foreground italic" : ""}`}>
                {cellText(v)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    );
  }

  return null;
}

// --- Main Component ---

const TABS: { id: TabId; label: string }[] = [
  { id: "json", label: "JSON" },
  { id: "tree", label: "Tree" },
  { id: "table", label: "Table" },
];

export function ComplexValueViewer({
  value,
  columnName,
  open,
  onOpenChange,
}: ComplexValueViewerProps) {
  const [activeTab, setActiveTab] = useState<TabId>("json");
  const [copied, setCopied] = useState(false);
  const formatted = JSON.stringify(value, null, 2);
  const typeLabel = Array.isArray(value) ? "array" : "object";

  const handleCopy = () => {
    navigator.clipboard.writeText(formatted).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="p-0 gap-0 max-w-2xl flex flex-col overflow-hidden h-[520px]">
        <DialogTitle className="sr-only">{columnName}</DialogTitle>

        {/* Header */}
        <div className="flex items-center gap-2 px-4 h-11 border-b shrink-0 pr-12">
          <span className="font-mono text-sm font-medium truncate min-w-0">{columnName}</span>
          <span className="text-[10px] uppercase tracking-wide text-muted-foreground bg-muted px-1.5 py-0.5 rounded shrink-0">
            {typeLabel}
          </span>
          <Button
            variant="ghost"
            size="sm"
            onClick={handleCopy}
            className="ml-auto h-7 gap-1.5 text-xs shrink-0"
          >
            {copied ? <Check size={11} /> : <Copy size={11} />}
            {copied ? "Copied" : "Copy JSON"}
          </Button>
        </div>

        {/* Custom Tab Bar */}
        <div className="flex border-b shrink-0 bg-muted/40">
          {TABS.map((tab) => {
            const active = activeTab === tab.id;
            return (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={[
                  "relative flex-1 px-5 h-9 text-xs font-medium transition-colors",
                  "border-r border-r-border/40 last:border-r-0",
                  active
                    ? "bg-background text-foreground"
                    : "bg-transparent text-muted-foreground hover:text-foreground hover:bg-muted/60",
                ]
                  .filter(Boolean)
                  .join(" ")}
              >
                {tab.label}
                {active && (
                  <span className="absolute bottom-0 left-0 right-0 h-0.5 bg-accent" />
                )}
              </button>
            );
          })}
        </div>

        {/* Tab Content */}
        <div className="flex-1 min-h-0 overflow-hidden">
          {activeTab === "json" && (
            <ScrollArea className="h-full">
              <pre className="p-4 text-xs font-mono leading-5 whitespace-pre text-foreground">
                {formatted}
              </pre>
            </ScrollArea>
          )}
          {activeTab === "tree" && (
            <ScrollArea className="h-full">
              <div className="p-3">
                <TreeNode label="root" value={value} depth={0} />
              </div>
            </ScrollArea>
          )}
          {activeTab === "table" && (
            <ScrollArea className="h-full">
              <TableView value={value} />
            </ScrollArea>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
