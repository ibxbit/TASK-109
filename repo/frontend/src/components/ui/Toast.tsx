/**
 * Minimal toast notification system.
 * Usage: call `toast.success('Saved!')` etc. from anywhere.
 *
 * This is a lightweight solution without an external library.
 * It uses a global event bus pattern to keep the bundle small.
 */

import React, { useEffect, useState, useCallback } from 'react';
import { CheckCircle, XCircle, AlertCircle, Info, X } from 'lucide-react';

type ToastType = 'success' | 'error' | 'warning' | 'info';

interface ToastItem {
  id:      string;
  type:    ToastType;
  message: string;
}

// ── Global event bus ──────────────────────────────────────────────────────────

const listeners: ((item: ToastItem) => void)[] = [];

function emit(item: ToastItem) {
  listeners.forEach((l) => l(item));
}

function makeId() {
  return Math.random().toString(36).slice(2);
}

export const toast = {
  success: (message: string) => emit({ id: makeId(), type: 'success', message }),
  error:   (message: string) => emit({ id: makeId(), type: 'error',   message }),
  warning: (message: string) => emit({ id: makeId(), type: 'warning', message }),
  info:    (message: string) => emit({ id: makeId(), type: 'info',    message }),
};

// ── ToastContainer — mount once in the app root ───────────────────────────────

const ICONS: Record<ToastType, React.ReactNode> = {
  success: <CheckCircle size={18} className="text-green-500 shrink-0" />,
  error:   <XCircle     size={18} className="text-red-500 shrink-0"   />,
  warning: <AlertCircle size={18} className="text-yellow-500 shrink-0"/>,
  info:    <Info        size={18} className="text-blue-500 shrink-0"  />,
};

const BG: Record<ToastType, string> = {
  success: 'border-green-200  bg-green-50',
  error:   'border-red-200    bg-red-50',
  warning: 'border-yellow-200 bg-yellow-50',
  info:    'border-blue-200   bg-blue-50',
};

export function ToastContainer() {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  useEffect(() => {
    const handler = (item: ToastItem) => {
      setToasts((prev) => [...prev, item]);
      setTimeout(() => {
        setToasts((prev) => prev.filter((t) => t.id !== item.id));
      }, 5000);
    };
    listeners.push(handler);
    return () => {
      const idx = listeners.indexOf(handler);
      if (idx !== -1) listeners.splice(idx, 1);
    };
  }, []);

  const dismiss = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  return (
    <div
      aria-live="polite"
      className="fixed bottom-4 right-4 z-[100] flex flex-col gap-2 w-80"
    >
      {toasts.map((t) => (
        <div
          key={t.id}
          role="alert"
          className={[
            'flex items-start gap-3 rounded-lg border px-4 py-3 shadow-lg',
            'animate-slide-in text-sm text-slate-800',
            BG[t.type],
          ].join(' ')}
        >
          {ICONS[t.type]}
          <span className="flex-1">{t.message}</span>
          <button
            onClick={() => dismiss(t.id)}
            aria-label="Dismiss"
            className="text-slate-400 hover:text-slate-600 shrink-0"
          >
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
