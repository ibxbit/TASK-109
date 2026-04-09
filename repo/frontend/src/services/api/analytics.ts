import apiClient from './client';
import type { AnalyticsReport, ExportRequest } from '../../types';

// ── GET /analytics ────────────────────────────────────────────────────────────

export async function getAnalytics(params: {
  org_unit_id?: string;
  start_date: string;
  end_date: string;
}): Promise<AnalyticsReport> {
  const { data } = await apiClient.get<AnalyticsReport>('/analytics', {
    params,
  });
  return data;
}

// ── POST /analytics/export ────────────────────────────────────────────────────

export async function requestExport(
  payload: ExportRequest,
): Promise<{ file_path: string }> {
  const { data } = await apiClient.post<{ file_path: string }>(
    '/analytics/export',
    payload,
  );
  return data;
}

// ── GET /analytics/export/:file_id/download ──────────────────────────────────
// Returns a blob — trigger a browser download via an <a> element.

export async function downloadExport(fileId: string): Promise<Blob> {
  const { data } = await apiClient.get<Blob>(
    `/analytics/export/${fileId}/download`,
    { responseType: 'blob' },
  );
  return data;
}

/**
 * Helper: request an export and immediately trigger a browser download.
 * Returns the file_id on success.
 */
export async function exportAndDownload(payload: ExportRequest): Promise<string> {
  const { file_path } = await requestExport(payload);
  // file_path is like "exports/abc123.xlsx" — extract the file_id segment
  const fileId = file_path.split('/').pop() ?? file_path;
  const cleanId = fileId.replace(/\.xlsx$/, '');
  const blob = await downloadExport(cleanId);

  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `vitalpath-report-${payload.start_date}-to-${payload.end_date}.xlsx`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);

  return cleanId;
}
