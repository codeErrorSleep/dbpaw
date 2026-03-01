import { useMemo, useState } from "react";
import { Check, ChevronsUpDown, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { cn } from "@/components/ui/utils";

export interface SelectedTableRef {
  schema: string;
  name: string;
}

export interface TableSelectorProps {
  tables: SelectedTableRef[];
  value: SelectedTableRef[];
  onChange: (next: SelectedTableRef[]) => void;
  disabled?: boolean;
}

function toKey(t: SelectedTableRef) {
  return `${t.schema}.${t.name}`;
}

export function TableSelector({
  tables,
  value,
  onChange,
  disabled,
}: TableSelectorProps) {
  const [open, setOpen] = useState(false);
  const hasSelection = value.length > 0;
  const selected = useMemo(() => new Set(value.map(toKey)), [value]);
  const byKey = useMemo(() => {
    const map = new Map<string, SelectedTableRef>();
    for (const t of tables) map.set(toKey(t), t);
    return map;
  }, [tables]);

  const toggle = (key: string) => {
    const next = new Set(selected);
    if (next.has(key)) {
      next.delete(key);
      onChange(value.filter((t) => toKey(t) !== key));
      return;
    }
    next.add(key);
    const item = byKey.get(key);
    if (!item) return;
    onChange([...value, item]);
  };

  const label =
    value.length === 0
      ? "Select table schema (no data)"
      : `Schema: ${value.length} selected`;

  return (
    <div className="min-w-0">
      <Popover open={open} onOpenChange={setOpen}>
        <div className="relative min-w-0">
          <PopoverTrigger asChild>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className={cn(
                "h-8 w-full min-w-0 justify-start border-border/60 bg-muted/20 pr-9 text-xs",
                hasSelection && "pr-14",
              )}
              disabled={disabled || tables.length === 0}
              aria-label="Only schema,no data"
            >
              <span className="truncate">{label}</span>
            </Button>
          </PopoverTrigger>
          <div className="pointer-events-none absolute inset-y-0 right-1 flex items-center gap-0.5">
            {hasSelection ? (
              <button
                type="button"
                className="pointer-events-auto inline-flex h-6 w-6 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-muted/60 hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                disabled={disabled}
                onMouseDown={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                }}
                onClick={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  onChange([]);
                }}
                title="Clear table selection"
                aria-label="Clear table selection"
              >
                <X className="h-3.5 w-3.5" />
              </button>
            ) : null}
            <ChevronsUpDown className="h-4 w-4 shrink-0 opacity-60" />
          </div>
        </div>
        <PopoverContent align="start" className="w-[320px] p-0">
          <Command>
            <CommandInput placeholder="Search tables..." />
            <CommandList>
              <CommandEmpty>No tables found</CommandEmpty>
              <CommandGroup heading="Tables">
                {tables.map((t) => {
                  const key = toKey(t);
                  const checked = selected.has(key);
                  return (
                    <CommandItem
                      key={key}
                      value={key}
                      onSelect={() => toggle(key)}
                      className="flex items-center gap-2"
                    >
                      <Check
                        className={cn(
                          "h-4 w-4",
                          checked ? "opacity-100" : "opacity-0",
                        )}
                      />
                      <span className="truncate">
                        {t.schema}.{t.name}
                      </span>
                    </CommandItem>
                  );
                })}
              </CommandGroup>
            </CommandList>
          </Command>
        </PopoverContent>
      </Popover>
    </div>
  );
}
