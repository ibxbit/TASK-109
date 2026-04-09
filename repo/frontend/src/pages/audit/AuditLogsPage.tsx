import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';
import { Search, ChevronLeft, ChevronRight } from 'lucide-react';
import * as auditApi from '../../services/api/audit';
import { Button } from '../../components/ui/Button';
import { Input } from '../../components/ui/Input';
import { Card } from '../../components/ui/Card';
import { EmptyState } from '../../components/ui/EmptyState';
import { Spinner } from '../../components/ui/Spinner';
import { formatDateTime } from '../../utils/formatters';
import type { AuditLogQueryParams } from '../../types';

const filterSchema = z.object({
  actor_id:    z.string().optional(),
  action:      z.string().optional(),
  entity_type: z.string().optional(),
  entity_id:   z.string().optional(),
  start_date:  z.string().regex(/^\d{4}-\d{2}-\d{2}$/).optional().or(z.literal('')),
  end_date:    z.string().regex(/^\d{4}-\d{2}-\d{2}$/).optional().or(z.literal('')),
});

type FilterFormValues = z.infer<typeof filterSchema>;

const ACTION_COLOUR: Record<string, string> = {
  LOGIN:          'bg-green-100 text-green-700',
  LOGIN_FAILED:   'bg-red-100 text-red-700',
  LOGOUT:         'bg-slate-100 text-slate-600',
  GOAL_CREATED:   'bg-blue-100 text-blue-700',
  GOAL_UPDATED:   'bg-blue-100 text-blue-700',
  METRIC_ENTRY_CREATED: 'bg-purple-100 text-purple-700',
  WORK_ORDER_CREATED:   'bg-orange-100 text-orange-700',
};

function actionClass(action: string): string {
  for (const [prefix, cls] of Object.entries(ACTION_COLOUR)) {
    if (action.startsWith(prefix)) return cls;
  }
  return 'bg-slate-100 text-slate-600';
}

export function AuditLogsPage() {
  const [page, setPage]     = useState(1);
  const [filters, setFilters] = useState<AuditLogQueryParams>({});

  const form = useForm<FilterFormValues>({ resolver: zodResolver(filterSchema) });

  const query = useQuery({
    queryKey: ['audit-logs', filters, page],
    queryFn: () =>
      auditApi.getAuditLogs({ ...filters, page, per_page: 50 }),
    staleTime: 10_000,
    retry: false,
  });

  function applyFilters(values: FilterFormValues) {
    setPage(1);
    setFilters({
      actor_id:    values.actor_id    || undefined,
      action:      values.action      || undefined,
      entity_type: values.entity_type || undefined,
      entity_id:   values.entity_id   || undefined,
      start_date:  values.start_date  || undefined,
      end_date:    values.end_date    || undefined,
    });
  }

  function clearFilters() {
    form.reset();
    setFilters({});
    setPage(1);
  }

  const data     = query.data;
  const logs     = data?.items ?? [];
  const total    = data?.total ?? 0;
  const perPage  = data?.per_page ?? 50;
  const lastPage = Math.max(1, Math.ceil(total / perPage));

  return (
    <div className="p-6 max-w-7xl mx-auto space-y-5">
      <div>
        <h1 className="text-xl font-bold text-slate-900">Audit Logs</h1>
        <p className="text-sm text-slate-500 mt-0.5">
          Immutable audit trail — every action performed in the system.
        </p>
      </div>

      {/* Filters */}
      <Card title="Filters">
        <form
          onSubmit={form.handleSubmit(applyFilters)}
          className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-3 items-end"
          noValidate
        >
          <Input
            label="Actor ID"
            placeholder="UUID"
            {...form.register('actor_id')}
          />
          <Input
            label="Action"
            placeholder="e.g. LOGIN_FAILED"
            {...form.register('action')}
          />
          <Input
            label="Entity type"
            placeholder="e.g. goal"
            {...form.register('entity_type')}
          />
          <Input
            label="Entity ID"
            placeholder="UUID"
            {...form.register('entity_id')}
          />
          <Input
            label="Start date"
            type="date"
            {...form.register('start_date')}
          />
          <Input
            label="End date"
            type="date"
            {...form.register('end_date')}
          />
          <div className="flex gap-2 col-span-full sm:col-span-1 lg:col-span-2">
            <Button type="submit" leftIcon={<Search size={14} />}>
              Search
            </Button>
            <Button type="button" variant="secondary" onClick={clearFilters}>
              Clear
            </Button>
          </div>
        </form>
      </Card>

      {/* Results */}
      {query.isLoading ? (
        <div className="flex justify-center py-16"><Spinner /></div>
      ) : query.isError ? (
        <div className="bg-red-50 border border-red-200 rounded-lg p-4 text-sm text-red-700">
          {(query.error as { message?: string }).message ?? 'Failed to load audit logs.'}
        </div>
      ) : logs.length === 0 ? (
        <EmptyState
          title="No audit log entries found"
          description="Adjust your filters or check back later."
        />
      ) : (
        <>
          <div className="text-xs text-slate-500">
            Showing {(page - 1) * perPage + 1}–{Math.min(page * perPage, total)} of {total} entries
          </div>
          <div className="overflow-x-auto rounded-lg border border-slate-200">
            <table className="w-full text-sm">
              <thead className="bg-slate-50 text-xs text-slate-500 uppercase tracking-wide">
                <tr>
                  {['Timestamp', 'Action', 'Entity Type', 'Entity ID', 'Actor ID', 'IP'].map(
                    (h) => (
                      <th key={h} className="px-4 py-3 text-left font-medium">
                        {h}
                      </th>
                    ),
                  )}
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100 bg-white">
                {logs.map((log) => (
                  <tr key={log.id} className="hover:bg-slate-50 transition-colors">
                    <td className="px-4 py-3 whitespace-nowrap text-slate-500 text-xs">
                      {formatDateTime(log.created_at)}
                    </td>
                    <td className="px-4 py-3 whitespace-nowrap">
                      <span
                        className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ${actionClass(log.action)}`}
                      >
                        {log.action}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-slate-600 whitespace-nowrap">
                      {log.entity_type ?? '—'}
                    </td>
                    <td className="px-4 py-3 font-mono text-xs text-slate-400 whitespace-nowrap">
                      {log.entity_id ? `${log.entity_id.slice(0, 8)}…` : '—'}
                    </td>
                    <td className="px-4 py-3 font-mono text-xs text-slate-400 whitespace-nowrap">
                      {log.actor_id ? `${log.actor_id.slice(0, 8)}…` : '—'}
                    </td>
                    <td className="px-4 py-3 text-xs text-slate-400">
                      {log.ip_address ?? '—'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Pagination */}
          <div className="flex items-center justify-between">
            <Button
              variant="secondary"
              size="sm"
              leftIcon={<ChevronLeft size={14} />}
              disabled={page === 1}
              onClick={() => setPage((p) => Math.max(1, p - 1))}
            >
              Previous
            </Button>
            <span className="text-xs text-slate-500">
              Page {page} of {lastPage}
            </span>
            <Button
              variant="secondary"
              size="sm"
              disabled={page === lastPage}
              onClick={() => setPage((p) => Math.min(lastPage, p + 1))}
            >
              Next
              <ChevronRight size={14} className="ml-1" />
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
