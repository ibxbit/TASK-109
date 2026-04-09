import apiClient from './client';
import type {
  MetricEntry,
  MetricSummary,
  CreateMetricEntryRequest,
  MetricQueryParams,
} from '../../types';

// ── POST /metrics ─────────────────────────────────────────────────────────────

export async function createMetricEntry(
  payload: CreateMetricEntryRequest,
): Promise<MetricEntry> {
  const { data } = await apiClient.post<MetricEntry>('/metrics', payload);
  return data;
}

// ── GET /metrics ──────────────────────────────────────────────────────────────

export async function getMetrics(
  params: MetricQueryParams,
): Promise<MetricEntry[]> {
  const { data } = await apiClient.get<MetricEntry[]>('/metrics', { params });
  return data;
}

// ── GET /metrics/summary ──────────────────────────────────────────────────────

export async function getMetricsSummary(params: {
  member_id: string;
  range?: string;
  start?: string;
  end?: string;
}): Promise<MetricSummary[]> {
  const { data } = await apiClient.get<MetricSummary[]>('/metrics/summary', {
    params,
  });
  return data;
}
