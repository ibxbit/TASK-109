import { useState } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  PieChart,
  Pie,
  Cell,
  Legend,
} from 'recharts';
import { Download, RefreshCw } from 'lucide-react';
import * as analyticsApi from '../../services/api/analytics';
import { Button } from '../../components/ui/Button';
import { Input } from '../../components/ui/Input';
import { Card } from '../../components/ui/Card';
import { Spinner } from '../../components/ui/Spinner';
import { toast } from '../../components/ui/Toast';
import { snakeToTitle } from '../../utils/formatters';

const CHART_COLORS = ['#2563eb', '#16a34a', '#d97706', '#dc2626', '#7c3aed', '#0891b2'];

const querySchema = z.object({
  start_date:  z.string().regex(/^\d{4}-\d{2}-\d{2}$/, 'YYYY-MM-DD required'),
  end_date:    z.string().regex(/^\d{4}-\d{2}-\d{2}$/, 'YYYY-MM-DD required'),
  org_unit_id: z.string().optional(),
});

type QueryFormValues = z.infer<typeof querySchema>;

function defaultDates() {
  const end   = new Date();
  const start = new Date();
  start.setDate(start.getDate() - 30);
  return {
    start_date: start.toISOString().slice(0, 10),
    end_date:   end.toISOString().slice(0, 10),
  };
}

export function AnalyticsPage() {
  const defaults = defaultDates();
  const [params, setParams] = useState<QueryFormValues>({
    ...defaults,
  });

  const form = useForm<QueryFormValues>({
    resolver: zodResolver(querySchema),
    defaultValues: params,
  });

  const query = useQuery({
    queryKey: ['analytics', params],
    queryFn: () =>
      analyticsApi.getAnalytics({
        start_date:  params.start_date,
        end_date:    params.end_date,
        org_unit_id: params.org_unit_id,
      }),
    retry: false,
  });

  const exportMutation = useMutation({
    mutationFn: () =>
      analyticsApi.exportAndDownload({
        start_date:  params.start_date,
        end_date:    params.end_date,
        org_unit_id: params.org_unit_id,
      }),
    onSuccess: () => toast.success('Export downloaded.'),
    onError: (err: unknown) =>
      toast.error((err as { message?: string }).message ?? 'Export failed.'),
  });

  function onSearch(values: QueryFormValues) {
    setParams(values);
  }

  const report = query.data;

  function toChartData(obj: Record<string, number> = {}) {
    return Object.entries(obj).map(([name, value]) => ({
      name:  snakeToTitle(name),
      value,
    }));
  }

  return (
    <div className="p-6 max-w-6xl mx-auto space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold text-slate-900">Analytics & Reports</h1>
          <p className="text-sm text-slate-500 mt-0.5">
            Program-level metrics and export capabilities.
          </p>
        </div>
        <Button
          variant="secondary"
          leftIcon={<Download size={15} />}
          loading={exportMutation.isPending}
          disabled={exportMutation.isPending || !report}
          onClick={() => exportMutation.mutate()}
        >
          Export XLSX
        </Button>
      </div>

      {/* Date range filter */}
      <Card title="Filter">
        <form
          onSubmit={form.handleSubmit(onSearch)}
          className="flex items-end gap-4 flex-wrap"
          noValidate
        >
          <Input
            label="Start date"
            type="date"
            required
            error={form.formState.errors.start_date?.message}
            {...form.register('start_date')}
          />
          <Input
            label="End date"
            type="date"
            required
            error={form.formState.errors.end_date?.message}
            {...form.register('end_date')}
          />
          <Input
            label="Org unit ID (optional)"
            placeholder="UUID"
            {...form.register('org_unit_id')}
          />
          <Button type="submit" leftIcon={<RefreshCw size={14} />}>
            Run report
          </Button>
        </form>
      </Card>

      {/* Results */}
      {query.isLoading && (
        <div className="flex justify-center py-16"><Spinner /></div>
      )}

      {query.isError && (
        <div className="bg-red-50 border border-red-200 rounded-lg p-4 text-sm text-red-700">
          {(query.error as { message?: string }).message ?? 'Failed to load analytics.'}
        </div>
      )}

      {report && (
        <div className="space-y-5">
          {/* Summary stats */}
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
            <StatCard label="Total Members" value={report.member_count} />
            <StatCard
              label="Date Range"
              value={`${params.start_date} → ${params.end_date}`}
            />
            <StatCard
              label="Org Unit"
              value={params.org_unit_id ? params.org_unit_id.slice(0, 8) + '…' : 'All'}
            />
          </div>

          {/* Charts */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-5">
            {Object.entries(report.metrics).map(([key, data]) => {
              const chartData = toChartData(data as Record<string, number>);
              if (chartData.length === 0) return null;
              return (
                <Card key={key} title={snakeToTitle(key)}>
                  {chartData.length <= 6 ? (
                    <PieChartPanel data={chartData} />
                  ) : (
                    <BarChartPanel data={chartData} />
                  )}
                </Card>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

// ── Stat card ─────────────────────────────────────────────────────────────────

function StatCard({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="bg-white rounded-lg border border-slate-200 shadow-sm px-5 py-4">
      <p className="text-xs text-slate-500">{label}</p>
      <p className="text-2xl font-bold text-slate-800 mt-1 truncate">{value}</p>
    </div>
  );
}

// ── Chart panels ──────────────────────────────────────────────────────────────

function BarChartPanel({ data }: { data: { name: string; value: number }[] }) {
  return (
    <ResponsiveContainer width="100%" height={200}>
      <BarChart data={data} margin={{ top: 4, right: 4, left: 0, bottom: 4 }}>
        <CartesianGrid strokeDasharray="3 3" stroke="#f1f5f9" />
        <XAxis dataKey="name" tick={{ fontSize: 11 }} />
        <YAxis tick={{ fontSize: 11 }} width={35} />
        <Tooltip contentStyle={{ fontSize: 12, borderRadius: 6 }} />
        <Bar dataKey="value" fill="#2563eb" radius={[3, 3, 0, 0]} />
      </BarChart>
    </ResponsiveContainer>
  );
}

function PieChartPanel({ data }: { data: { name: string; value: number }[] }) {
  return (
    <ResponsiveContainer width="100%" height={200}>
      <PieChart>
        <Pie
          data={data}
          dataKey="value"
          nameKey="name"
          cx="50%"
          cy="50%"
          outerRadius={70}
          label={({ name, percent }: { name: string; percent: number }) =>
            `${name}: ${(percent * 100).toFixed(0)}%`
          }
          labelLine={false}
        >
          {data.map((_, i) => (
            <Cell
              key={i}
              fill={CHART_COLORS[i % CHART_COLORS.length]}
            />
          ))}
        </Pie>
        <Legend iconSize={10} wrapperStyle={{ fontSize: 11 }} />
        <Tooltip contentStyle={{ fontSize: 12, borderRadius: 6 }} />
      </PieChart>
    </ResponsiveContainer>
  );
}
