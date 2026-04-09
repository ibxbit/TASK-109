import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Bell, BellOff } from 'lucide-react';
import * as notifApi from '../../services/api/notifications';
import { Button } from '../../components/ui/Button';
import { Card } from '../../components/ui/Card';
import { Badge } from '../../components/ui/Badge';
import { EmptyState } from '../../components/ui/EmptyState';
import { Spinner } from '../../components/ui/Spinner';
import { toast } from '../../components/ui/Toast';
import { formatRelativeTime } from '../../utils/formatters';
import { EVENT_TYPE_LABELS } from '../../utils/constants';
import type { NotificationEventType } from '../../types';

const ALL_EVENT_TYPES: NotificationEventType[] = [
  'manual',
  'goal_completed',
  'metric_milestone',
  'health_alert',
  'work_order_update',
];

export function NotificationsPage() {
  const qc = useQueryClient();

  const notifQuery = useQuery({
    queryKey: ['notifications-page'],
    queryFn:  () => notifApi.getNotifications({ limit: 100 }),
  });

  const subsQuery = useQuery({
    queryKey: ['subscriptions'],
    queryFn:  () => notifApi.getSubscriptions(),
  });

  const markAllMutation = useMutation({
    mutationFn: () => notifApi.markAllNotificationsRead(),
    onSuccess: (data) => {
      qc.invalidateQueries({ queryKey: ['notifications-page'] });
      qc.invalidateQueries({ queryKey: ['notifications'] });
      toast.success(`Marked ${data.marked_count} notifications as read.`);
    },
  });

  const markOneMutation = useMutation({
    mutationFn: (id: string) => notifApi.markNotificationRead(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['notifications-page'] });
      qc.invalidateQueries({ queryKey: ['notifications'] });
    },
  });

  const toggleSubMutation = useMutation({
    mutationFn: ({ eventType, is_subscribed }: { eventType: NotificationEventType; is_subscribed: boolean }) =>
      notifApi.updateSubscription(eventType, is_subscribed),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['subscriptions'] });
      toast.success('Subscription updated.');
    },
    onError: (err: unknown) => {
      toast.error((err as { message?: string }).message ?? 'Failed to update.');
    },
  });

  const notifications = notifQuery.data ?? [];
  const unread = notifications.filter((n) => !n.is_read).length;

  return (
    <div className="p-6 max-w-4xl mx-auto space-y-6">
      <div>
        <h1 className="text-xl font-bold text-slate-900">Notifications</h1>
        <p className="text-sm text-slate-500 mt-0.5">
          In-app notifications and subscription preferences.
        </p>
      </div>

      {/* Subscription preferences */}
      <Card
        title="Notification Preferences"
        actions={
          subsQuery.isLoading ? <Spinner size="sm" /> : null
        }
      >
        <div className="space-y-2">
          {ALL_EVENT_TYPES.map((eventType) => {
            const sub = subsQuery.data?.find((s) => s.event_type === eventType);
            const isSubscribed = sub?.is_subscribed ?? true;
            return (
              <div
                key={eventType}
                className="flex items-center justify-between py-2 border-b border-slate-100 last:border-0"
              >
                <div>
                  <p className="text-sm text-slate-700">
                    {EVENT_TYPE_LABELS[eventType] ?? eventType}
                  </p>
                </div>
                <button
                  onClick={() =>
                    toggleSubMutation.mutate({ eventType, is_subscribed: !isSubscribed })
                  }
                  disabled={toggleSubMutation.isPending}
                  aria-label={isSubscribed ? 'Unsubscribe' : 'Subscribe'}
                  className="flex items-center gap-1.5 text-xs font-medium transition-colors"
                >
                  {isSubscribed ? (
                    <>
                      <Bell size={14} className="text-green-500" />
                      <span className="text-green-600">On</span>
                    </>
                  ) : (
                    <>
                      <BellOff size={14} className="text-slate-400" />
                      <span className="text-slate-400">Off</span>
                    </>
                  )}
                </button>
              </div>
            );
          })}
        </div>
      </Card>

      {/* Notifications list */}
      <Card
        title={`Notifications${unread > 0 ? ` (${unread} unread)` : ''}`}
        actions={
          unread > 0 ? (
            <Button
              size="sm"
              variant="secondary"
              loading={markAllMutation.isPending}
              onClick={() => markAllMutation.mutate()}
            >
              Mark all read
            </Button>
          ) : null
        }
      >
        {notifQuery.isLoading ? (
          <div className="flex justify-center py-10"><Spinner /></div>
        ) : notifications.length === 0 ? (
          <EmptyState
            icon={<Bell size={40} />}
            title="No notifications"
            description="You're all caught up!"
          />
        ) : (
          <div className="divide-y divide-slate-100">
            {notifications.map((n) => (
              <div
                key={n.id}
                className={[
                  'flex items-start gap-3 py-4',
                  !n.is_read ? 'bg-blue-50/50' : '',
                ].join(' ')}
              >
                {!n.is_read && (
                  <span className="mt-1.5 h-2 w-2 rounded-full bg-brand-500 shrink-0" />
                )}
                {n.is_read && <span className="mt-1.5 h-2 w-2 shrink-0" />}
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <p className="text-sm font-medium text-slate-800">{n.title}</p>
                    <Badge variant="default">
                      {EVENT_TYPE_LABELS[n.event_type] ?? n.event_type}
                    </Badge>
                  </div>
                  <p className="text-sm text-slate-600 mt-0.5">{n.body}</p>
                  <p className="text-xs text-slate-400 mt-1">
                    {formatRelativeTime(n.created_at)}
                  </p>
                </div>
                {!n.is_read && (
                  <button
                    onClick={() => markOneMutation.mutate(n.id)}
                    className="text-slate-400 hover:text-slate-600 shrink-0"
                    aria-label="Mark as read"
                  >
                    <Bell size={15} />
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}
