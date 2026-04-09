import apiClient from './client';
import type {
  Notification,
  NotificationSubscription,
  NotificationSchedule,
  NotificationEventType,
} from '../../types';

// ── POST /notifications ───────────────────────────────────────────────────────

export async function createNotification(payload: {
  user_id: string;
  title: string;
  body: string;
  event_type: NotificationEventType;
  entity_type?: string;
  entity_id?: string;
}): Promise<{ id: string }> {
  const { data } = await apiClient.post<{ id: string }>(
    '/notifications',
    payload,
  );
  return data;
}

// ── GET /notifications ────────────────────────────────────────────────────────

export async function getNotifications(params?: {
  is_read?: boolean;
  event_type?: NotificationEventType;
  limit?: number;
  offset?: number;
}): Promise<Notification[]> {
  const { data } = await apiClient.get<Notification[]>('/notifications', {
    params,
  });
  return data;
}

// ── POST /notifications/:id/read ─────────────────────────────────────────────

export async function markNotificationRead(id: string): Promise<void> {
  await apiClient.post(`/notifications/${id}/read`);
}

// ── POST /notifications/read-all ─────────────────────────────────────────────

export async function markAllNotificationsRead(): Promise<{ marked_count: number }> {
  const { data } = await apiClient.post<{ marked_count: number }>(
    '/notifications/read-all',
  );
  return data;
}

// ── GET /notifications/subscriptions ─────────────────────────────────────────

export async function getSubscriptions(): Promise<NotificationSubscription[]> {
  const { data } = await apiClient.get<NotificationSubscription[]>(
    '/notifications/subscriptions',
  );
  return data;
}

// ── PATCH /notifications/subscriptions/:event_type ───────────────────────────

export async function updateSubscription(
  eventType: NotificationEventType,
  is_subscribed: boolean,
): Promise<NotificationSubscription> {
  const { data } = await apiClient.patch<NotificationSubscription>(
    `/notifications/subscriptions/${eventType}`,
    { is_subscribed },
  );
  return data;
}

// ── POST /notifications/schedules ────────────────────────────────────────────

export async function createSchedule(payload: {
  template_id: string;
  label: string;
  fire_hour: number;
  tz_offset_minutes: number;
}): Promise<NotificationSchedule> {
  const { data } = await apiClient.post<NotificationSchedule>(
    '/notifications/schedules',
    payload,
  );
  return data;
}

// ── GET /notifications/schedules ─────────────────────────────────────────────

export async function getSchedules(): Promise<NotificationSchedule[]> {
  const { data } = await apiClient.get<NotificationSchedule[]>(
    '/notifications/schedules',
  );
  return data;
}

// ── DELETE /notifications/schedules/:id ──────────────────────────────────────

export async function deleteSchedule(id: string): Promise<void> {
  await apiClient.delete(`/notifications/schedules/${id}`);
}
