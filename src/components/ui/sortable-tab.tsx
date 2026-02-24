import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { ReactNode } from "react";

interface SortableTabProps {
  id: string;
  children: ReactNode;
}

export function SortableTab({ id, children }: SortableTabProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id });

  const style = {
    transform: CSS.Translate.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
    zIndex: isDragging ? 50 : "auto",
    position: isDragging ? "relative" : undefined,
  };

  return (
    <div
      ref={setNodeRef}
      style={style as any}
      {...attributes}
      {...listeners}
      className="h-full flex items-center outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
    >
      {children}
    </div>
  );
}
