import React from 'react';

interface TextareaProps extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?:    string;
  error?:    string;
  hint?:     string;
  required?: boolean;
  maxChars?: number;
  currentLength?: number;
}

export const Textarea = React.forwardRef<HTMLTextAreaElement, TextareaProps>(
  (
    {
      label,
      error,
      hint,
      required,
      maxChars,
      currentLength,
      id,
      className = '',
      ...props
    },
    ref,
  ) => {
    const taId = id ?? `textarea-${label?.toLowerCase().replace(/\s+/g, '-')}`;
    const charsLeft = maxChars !== undefined && currentLength !== undefined
      ? maxChars - currentLength
      : null;

    return (
      <div className="space-y-1">
        {label && (
          <div className="flex justify-between items-baseline">
            <label
              htmlFor={taId}
              className="block text-sm font-medium text-slate-700"
            >
              {label}
              {required && <span className="text-red-500 ml-0.5">*</span>}
            </label>
            {charsLeft !== null && (
              <span
                className={`text-xs ${charsLeft < 50 ? 'text-orange-500' : 'text-slate-400'}`}
              >
                {charsLeft} left
              </span>
            )}
          </div>
        )}
        <textarea
          ref={ref}
          id={taId}
          aria-invalid={!!error}
          rows={4}
          className={[
            'block w-full rounded-md border px-3 py-2 text-sm shadow-sm resize-y',
            'placeholder:text-slate-400',
            'focus:outline-none focus:ring-2 focus:ring-brand-500 focus:border-brand-500',
            'disabled:bg-slate-50 disabled:text-slate-400 disabled:cursor-not-allowed',
            'transition-colors duration-150',
            error ? 'border-red-400' : 'border-slate-300',
            className,
          ].join(' ')}
          {...props}
        />
        {error && (
          <p role="alert" className="text-xs text-red-600">
            {error}
          </p>
        )}
        {hint && !error && (
          <p className="text-xs text-slate-500">{hint}</p>
        )}
      </div>
    );
  },
);

Textarea.displayName = 'Textarea';
