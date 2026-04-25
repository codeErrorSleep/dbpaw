import { useState } from "react";
import { Braces, AlertTriangle } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

interface Props {
  value: string;
  onChange: (v: string) => void;
  isBinary?: boolean;
}

function tryParseJson(s: string): unknown | null {
  const trimmed = s.trim();
  if (!trimmed) return null;
  try {
    return JSON.parse(trimmed);
  } catch {
    return null;
  }
}

export function RedisStringViewer({ value, onChange, isBinary }: Props) {
  const [formatted, setFormatted] = useState(false);
  const [editAsText, setEditAsText] = useState(false);
  const parsed = tryParseJson(value);
  const isJson = parsed !== null && !isBinary;

  const displayValue =
    formatted && isJson ? JSON.stringify(parsed, null, 2) : value;

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between text-sm text-muted-foreground">
        <span>{value.length} chars</span>
        <div className="flex items-center gap-2">
          {isJson && (
            <Badge variant="secondary" className="text-xs">
              JSON
            </Badge>
          )}
          {isBinary && (
            <Badge variant="destructive" className="text-xs">
              Binary
            </Badge>
          )}
          {isJson && (
            <Button
              variant="outline"
              size="sm"
              className="h-6 px-2 text-xs"
              onClick={() => setFormatted((f) => !f)}
            >
              <Braces className="w-3 h-3 mr-1" />
              {formatted ? "Raw" : "Beautify"}
            </Button>
          )}
          {isBinary && (
            <Button
              variant="outline"
              size="sm"
              className="h-6 px-2 text-xs"
              onClick={() => setEditAsText((e) => !e)}
            >
              {editAsText ? "Base64" : "Edit as text"}
            </Button>
          )}
        </div>
      </div>
      {isBinary && editAsText && (
        <div className="flex items-center gap-2 text-xs text-amber-600 bg-amber-50 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-900 rounded px-3 py-2">
          <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
          <span>
            This value contains binary data. Editing as text may corrupt the
            original bytes.
          </span>
        </div>
      )}
      <Textarea
        className="min-h-[320px] font-mono text-sm"
        value={displayValue}
        onChange={(e) => {
          setFormatted(false);
          onChange(e.target.value);
        }}
        placeholder="String value"
        readOnly={isBinary && !editAsText}
      />
    </div>
  );
}
