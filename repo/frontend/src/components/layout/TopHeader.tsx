import { useState } from 'react';
import { Bell, LogOut, ChevronDown } from 'lucide-react';
import { useAuth } from '../../hooks/useAuth';
import { useNotifications } from '../../hooks/useNotifications';
import { ROLE_LABELS } from '../../utils/constants';
import { formatRelativeTime } from '../../utils/formatters';
import { Button } from '../ui/Button';

export function TopHeader() {
  const { user, logout } = useAuth();
  const { notifications, unreadCount, markRead, markAllRead } = useNotifications();
  const [notifOpen, setNotifOpen] = useState(false);

  return (
    <header className="h-14 shrink-0 bg-white border-b border-slate-200 flex items-center justify-end px-6 gap-4">
      {/* Notification Bell */}
      <div className="relative">
        <button
          aria-label={`Notifications (${unreadCount} unread)`}
          onClick={() => setNotifOpen((v) => !v)}
          className="relative p-2 rounded-md text-slate-500 hover:bg-slate-100 transition-colors"
        >
          <Bell size={20} />
          {unreadCount > 0 && (
            <span className="absolute top-1 right-1 h-4 w-4 rounded-full bg-red-500 text-white text-[10px] flex items-center justify-center leading-none">
              {unreadCount > 9 ? '9+' : unreadCount}
            </span>
          )}
        </button>

        {notifOpen && (
          <div className="absolute right-0 top-10 z-50 w-80 bg-white rounded-lg shadow-lg border border-slate-200 animate-slide-in">
            <div className="flex items-center justify-between px-4 py-3 border-b border-slate-100">
              <span className="text-sm font-semibold text-slate-700">
                Notifications
              </span>
              {unreadCount > 0 && (
                <button
                  onClick={() => { markAllRead(); }}
                  className="text-xs text-brand-600 hover:text-brand-700"
                >
                  Mark all read
                </button>
              )}
            </div>
            <div className="max-h-80 overflow-y-auto divide-y divide-slate-100">
              {notifications.length === 0 ? (
                <p className="px-4 py-6 text-sm text-slate-400 text-center">
                  No notifications
                </p>
              ) : (
                notifications.slice(0, 10).map((n) => (
                  <div
                    key={n.id}
                    onClick={() => markRead(n.id)}
                    className={[
                      'px-4 py-3 cursor-pointer hover:bg-slate-50 transition-colors',
                      !n.is_read ? 'bg-blue-50' : '',
                    ].join(' ')}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <p className="text-xs font-medium text-slate-700 leading-snug">
                        {n.title}
                      </p>
                      {!n.is_read && (
                        <span className="shrink-0 h-2 w-2 mt-1 rounded-full bg-brand-500" />
                      )}
                    </div>
                    <p className="text-xs text-slate-500 mt-0.5 line-clamp-2">
                      {n.body}
                    </p>
                    <p className="text-[10px] text-slate-400 mt-1">
                      {formatRelativeTime(n.created_at)}
                    </p>
                  </div>
                ))
              )}
            </div>
          </div>
        )}
      </div>

      {/* User Menu */}
      <div className="flex items-center gap-2 text-sm text-slate-700">
        <span className="hidden sm:block font-medium">{user?.username}</span>
        <span className="hidden sm:block text-slate-400 text-xs">
          {ROLE_LABELS[user?.role_id ?? ''] ?? ''}
        </span>
        <ChevronDown size={14} className="text-slate-400" />
      </div>

      <Button
        variant="ghost"
        size="sm"
        onClick={logout}
        leftIcon={<LogOut size={15} />}
        aria-label="Sign out"
      >
        Sign out
      </Button>
    </header>
  );
}
