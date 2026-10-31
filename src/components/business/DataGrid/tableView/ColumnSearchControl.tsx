import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronLeft, ChevronRight, TextSearch, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import type { useColumnSearch } from "./hooks/useColumnSearch";

export function ColumnSearchControl({
  search,
}: {
  search: ReturnType<typeof useColumnSearch>;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const key = "tableView.columnSearch";

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant={search.query.trim() || open ? "secondary" : "ghost"}
          size="sm"
          className="h-6 gap-1 px-2 text-xs"
          aria-label={t(`${key}.title`)}
          title={t(`${key}.title`)}
        >
          <TextSearch className="h-3.5 w-3.5" />
          {t(`${key}.title`)}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-80 space-y-2 p-3">
        <div className="flex items-center gap-1">
          <Input
            className="h-8 min-w-0 text-xs"
            aria-label={t(`${key}.placeholder`)}
            placeholder={t(`${key}.placeholder`)}
            value={search.query}
            onChange={(event) => search.changeQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                search.navigate(event.shiftKey ? -1 : 1);
              }
            }}
          />
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8 shrink-0"
            disabled={!search.query}
            onClick={() => search.changeQuery("")}
            aria-label={t(`${key}.clear`)}
            title={t(`${key}.clear`)}
          >
            <X className="h-3.5 w-3.5" />
          </Button>
          {([-1, 1] as const).map((direction) => {
            const label = t(`${key}.${direction === -1 ? "previous" : "next"}`);
            const Icon = direction === -1 ? ChevronLeft : ChevronRight;
            return (
              <Button
                key={direction}
                variant="ghost"
                size="icon"
                className="h-8 w-8 shrink-0"
                disabled={!search.matches.length}
                onClick={() => search.navigate(direction)}
                aria-label={label}
                title={label}
              >
                <Icon className="h-3.5 w-3.5" />
              </Button>
            );
          })}
        </div>
        <div role="status" className="text-xs text-muted-foreground">
          {search.query.trim()
            ? search.matches.length
              ? t(`${key}.count`, {
                  current: search.activeIndex + 1,
                  total: search.matches.length,
                })
              : t(`${key}.noMatches`)
            : t(`${key}.hint`)}
        </div>
        {search.highlight.activeColumn !== null && (
          <div
            className="truncate font-mono text-xs"
            title={search.highlight.activeColumn}
          >
            {search.highlight.activeColumn}
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}
