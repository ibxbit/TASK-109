import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  ReferenceLine,
} from 'recharts';
import { format, parseISO } from 'date-fns';
import type { MetricEntry, MetricType } from '../../types';
import { METRIC_UNITS } from '../../types';
import { METRIC_TYPE_LABELS } from '../../utils/constants';
import { EmptyState } from '../ui/EmptyState';

interface MetricChartProps {
  entries:     MetricEntry[];
  metricType:  MetricType;
  targetValue?: number;
  height?:     number;
}

export function MetricChart({
  entries,
  metricType,
  targetValue,
  height = 240,
}: MetricChartProps) {
  if (entries.length === 0) {
    return (
      <EmptyState
        title="No data yet"
        description="Metric entries will appear here once recorded."
      />
    );
  }

  const data = [...entries]
    .sort((a, b) => a.entry_date.localeCompare(b.entry_date))
    .map((e) => ({
      date:  e.entry_date,
      value: e.value,
    }));

  const unit  = METRIC_UNITS[metricType];
  const label = METRIC_TYPE_LABELS[metricType] ?? metricType;

  return (
    <div>
      <p className="text-xs text-slate-500 mb-2">
        {label} ({unit})
      </p>
      <ResponsiveContainer width="100%" height={height}>
        <LineChart data={data} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="#f1f5f9" />
          <XAxis
            dataKey="date"
            tick={{ fontSize: 11, fill: '#64748b' }}
            tickFormatter={(d: string) => {
              try { return format(parseISO(d), 'MMM d'); }
              catch { return d; }
            }}
          />
          <YAxis
            tick={{ fontSize: 11, fill: '#64748b' }}
            tickFormatter={(v: number) => String(v)}
            width={45}
          />
          <Tooltip
            formatter={(value: number) => [`${value} ${unit}`, label]}
            labelFormatter={(d: string) => {
              try { return format(parseISO(d), 'MMM d, yyyy'); }
              catch { return d; }
            }}
            contentStyle={{
              fontSize: 12,
              borderRadius: 6,
              border: '1px solid #e2e8f0',
            }}
          />
          <Line
            type="monotone"
            dataKey="value"
            stroke="#2563eb"
            strokeWidth={2}
            dot={{ r: 3, fill: '#2563eb' }}
            activeDot={{ r: 5 }}
          />
          {targetValue !== undefined && (
            <ReferenceLine
              y={targetValue}
              stroke="#16a34a"
              strokeDasharray="4 4"
              label={{
                value: `Target: ${targetValue} ${unit}`,
                fontSize: 11,
                fill: '#16a34a',
                position: 'insideTopRight',
              }}
            />
          )}
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
