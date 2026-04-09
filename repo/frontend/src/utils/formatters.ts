import { format, formatDistanceToNow, parseISO, isValid } from 'date-fns';

/**
 * Format an ISO-8601 datetime string to a human-readable date.
 * Returns 'N/A' if the input is null/undefined/invalid.
 */
export function formatDate(iso: string | null | undefined): string {
  if (!iso) return 'N/A';
  const d = parseISO(iso);
  return isValid(d) ? format(d, 'MMM d, yyyy') : 'N/A';
}

/**
 * Format an ISO-8601 datetime string to date + time.
 */
export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return 'N/A';
  const d = parseISO(iso);
  return isValid(d) ? format(d, 'MMM d, yyyy HH:mm') : 'N/A';
}

/**
 * Return a relative time string, e.g. "2 hours ago".
 */
export function formatRelativeTime(iso: string | null | undefined): string {
  if (!iso) return 'N/A';
  const d = parseISO(iso);
  return isValid(d) ? formatDistanceToNow(d, { addSuffix: true }) : 'N/A';
}

/**
 * Format a numeric metric value with its unit.
 * e.g. formatMetricValue(182.5, 'lbs') → '182.5 lbs'
 */
export function formatMetricValue(
  value: number | null | undefined,
  unit: string,
): string {
  if (value === null || value === undefined) return 'N/A';
  return `${value % 1 === 0 ? value : value.toFixed(1)} ${unit}`;
}

/**
 * Format a percentage change with a ± prefix and fixed decimal places.
 */
export function formatChange(change: number | null | undefined): string {
  if (change === null || change === undefined) return '—';
  const sign = change >= 0 ? '+' : '';
  return `${sign}${change.toFixed(1)}`;
}

/**
 * Format a percentage value (0–100) with a % suffix.
 */
export function formatPct(pct: number | null | undefined): string {
  if (pct === null || pct === undefined) return '—';
  const sign = pct >= 0 ? '+' : '';
  return `${sign}${pct.toFixed(1)}%`;
}

/**
 * Truncate a string to maxLen characters, appending '…' if truncated.
 */
export function truncate(str: string | null | undefined, maxLen: number): string {
  if (!str) return '';
  return str.length > maxLen ? `${str.slice(0, maxLen)}…` : str;
}

/**
 * Convert a snake_case identifier to a Title Case label.
 * e.g. 'body_fat_percentage' → 'Body Fat Percentage'
 */
export function snakeToTitle(str: string): string {
  return str
    .split('_')
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ');
}

/**
 * Return a CSS colour class based on SLA deadline imminence.
 */
export function slaColourClass(deadline: string | null | undefined): string {
  if (!deadline) return 'text-slate-500';
  const d = parseISO(deadline);
  if (!isValid(d)) return 'text-slate-500';
  const hoursLeft = (d.getTime() - Date.now()) / 3_600_000;
  if (hoursLeft < 0)   return 'text-red-600 font-semibold';
  if (hoursLeft < 8)   return 'text-orange-500 font-semibold';
  if (hoursLeft < 24)  return 'text-yellow-600';
  return 'text-green-600';
}
