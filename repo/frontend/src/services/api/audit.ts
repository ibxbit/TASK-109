import apiClient from './client';
import type { AuditLogPage, AuditLogQueryParams } from '../../types';

// ── GET /audit-logs ───────────────────────────────────────────────────────────

export async function getAuditLogs(
  params: AuditLogQueryParams = {},
): Promise<AuditLogPage> {
  const { data } = await apiClient.get<AuditLogPage>('/audit-logs', {
    params,
  });
  return data;
}
