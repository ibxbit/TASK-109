import { InboxIcon } from 'lucide-react';
import type { ReactNode } from 'react';

interface EmptyStateProps {
  title:       string;
  description?: string;
  action?:      ReactNode;
  icon?:        ReactNode;
}

export function EmptyState({
  title,
  description,
  action,
  icon,
}: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center py-16 px-8 text-center gap-3">
      <div className="text-slate-300">
        {icon ?? <InboxIcon size={48} />}
      </div>
      <h3 className="text-sm font-semibold text-slate-700">{title}</h3>
      {description && (
        <p className="text-sm text-slate-500 max-w-xs">{description}</p>
      )}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}
