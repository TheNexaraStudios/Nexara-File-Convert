import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import "./EmptyState.css";

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  description: string;
  action?: ReactNode;
}

export function EmptyState({ icon: Icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="empty-state anim-fade-in">
      <div className="empty-state__icon">
        <Icon size={26} strokeWidth={1.6} />
      </div>
      <h3>{title}</h3>
      <p>{description}</p>
      {action}
    </div>
  );
}
