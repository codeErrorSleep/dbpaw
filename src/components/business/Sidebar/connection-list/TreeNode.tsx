import type { ReactNode, MouseEvent } from "react";
import { ChevronRight, ChevronDown } from "lucide-react";

export interface TreeNodeProps {
  level: number;
  children: ReactNode;
  icon: ReactNode;
  label: string;
  isSelected?: boolean;
  isExpanded?: boolean;
  onToggle?: () => void;
  canToggle?: boolean;
  forceShowToggle?: boolean;
  hideToggle?: boolean;
  toggleOnRowClick?: boolean;
  onDoubleClick?: () => void;
  onContextMenu?: (e: MouseEvent) => void;
  leadingIndicator?: ReactNode;
  statusIndicator?: ReactNode;
  actions?: ReactNode;
}

export function TreeNode({
  level,
  children,
  icon,
  label,
  isSelected = false,
  isExpanded,
  onToggle,
  canToggle = true,
  forceShowToggle = false,
  hideToggle = false,
  toggleOnRowClick = true,
  onDoubleClick,
  onContextMenu,
  leadingIndicator,
  statusIndicator,
  actions,
}: TreeNodeProps) {
  const hasChildren = children !== null && children !== undefined;
  const showToggle = !hideToggle && (forceShowToggle || hasChildren);

  return (
    <div>
      <div
        className={`flex items-center gap-1 px-2 py-1 cursor-pointer group select-none ${
          isSelected ? "bg-accent text-accent-foreground" : "hover:bg-accent"
        }`}
        style={{ paddingLeft: `${level * 12 + 8}px` }}
        onClick={toggleOnRowClick ? onToggle : undefined}
        onDoubleClick={onDoubleClick}
        onContextMenu={onContextMenu}
      >
        {leadingIndicator ? (
          <span className="inline-flex w-4 items-center justify-center shrink-0">
            {leadingIndicator}
          </span>
        ) : showToggle ? (
          <button
            type="button"
            className={`text-muted-foreground ${!canToggle ? "opacity-50 cursor-not-allowed" : ""}`}
            onClick={(e) => {
              e.stopPropagation();
              if (!canToggle) return;
              onToggle?.();
            }}
            disabled={!canToggle}
            aria-label={isExpanded ? "Collapse" : "Expand"}
          >
            {isExpanded ? (
              <ChevronDown className="w-4 h-4" />
            ) : (
              <ChevronRight className="w-4 h-4" />
            )}
          </button>
        ) : (
          <span className="w-4" />
        )}
        <span className="text-muted-foreground">{icon}</span>
        <span className="flex-1 text-sm truncate">{label}</span>
        {statusIndicator}
        {actions && (
          <span className="opacity-0 group-hover:opacity-100">{actions}</span>
        )}
      </div>
      {isExpanded && children}
    </div>
  );
}
