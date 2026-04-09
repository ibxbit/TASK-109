import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as notifApi from '../services/api/notifications';

const QUERY_KEY = ['notifications'];

/**
 * Hook for in-app notification data and actions.
 * Polls every 60 seconds for new notifications.
 */
export function useNotifications() {
  const qc = useQueryClient();

  const query = useQuery({
    queryKey: QUERY_KEY,
    queryFn:  () => notifApi.getNotifications({ limit: 50 }),
    refetchInterval: 60_000,
    staleTime:        30_000,
  });

  const unreadCount = query.data?.filter((n) => !n.is_read).length ?? 0;

  const markRead = useMutation({
    mutationFn: (id: string) => notifApi.markNotificationRead(id),
    onSuccess:  () => qc.invalidateQueries({ queryKey: QUERY_KEY }),
  });

  const markAllRead = useMutation({
    mutationFn: () => notifApi.markAllNotificationsRead(),
    onSuccess:  () => qc.invalidateQueries({ queryKey: QUERY_KEY }),
  });

  return {
    notifications:   query.data ?? [],
    isLoading:       query.isLoading,
    unreadCount,
    markRead:        (id: string) => markRead.mutate(id),
    markAllRead:     () => markAllRead.mutate(),
  };
}
