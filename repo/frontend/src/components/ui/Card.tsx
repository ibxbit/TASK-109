interface CardProps {
  children:   React.ReactNode;
  className?: string;
  title?:     string;
  actions?:   React.ReactNode;
}

export function Card({ children, className = '', title, actions }: CardProps) {
  return (
    <div
      className={[
        'bg-white rounded-lg border border-slate-200 shadow-sm',
        className,
      ].join(' ')}
    >
      {(title || actions) && (
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-200">
          {title && (
            <h3 className="text-sm font-semibold text-slate-800">{title}</h3>
          )}
          {actions && <div className="flex items-center gap-2">{actions}</div>}
        </div>
      )}
      <div className="p-5">{children}</div>
    </div>
  );
}
