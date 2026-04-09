import { describe, it, expect } from 'vitest';
import {
  formatDate,
  formatDateTime,
  formatMetricValue,
  formatChange,
  formatPct,
  truncate,
  snakeToTitle,
  slaColourClass,
} from '../../utils/formatters';

describe('formatDate', () => {
  it('formats a valid ISO date string', () => {
    expect(formatDate('2024-03-15T00:00:00Z')).toBe('Mar 15, 2024');
  });

  it('returns N/A for null', () => {
    expect(formatDate(null)).toBe('N/A');
  });

  it('returns N/A for undefined', () => {
    expect(formatDate(undefined)).toBe('N/A');
  });

  it('returns N/A for an invalid string', () => {
    expect(formatDate('not-a-date')).toBe('N/A');
  });
});

describe('formatDateTime', () => {
  it('includes date and time components', () => {
    const result = formatDateTime('2024-03-15T14:30:00Z');
    // Date portion must be present (time zone may shift hour display)
    expect(result).toMatch(/Mar \d+, 2024/);
    // Time portion must be present (HH:MM format)
    expect(result).toMatch(/\d{2}:\d{2}/);
  });
});

describe('formatMetricValue', () => {
  it('formats integer value', () => {
    expect(formatMetricValue(180, 'lbs')).toBe('180 lbs');
  });

  it('formats float value to 1 decimal', () => {
    expect(formatMetricValue(180.5, 'lbs')).toBe('180.5 lbs');
  });

  it('returns N/A for null', () => {
    expect(formatMetricValue(null, 'lbs')).toBe('N/A');
  });
});

describe('formatChange', () => {
  it('shows + prefix for positive change', () => {
    expect(formatChange(5.2)).toBe('+5.2');
  });

  it('shows − prefix for negative change', () => {
    expect(formatChange(-3.1)).toBe('-3.1');
  });

  it('returns em dash for null', () => {
    expect(formatChange(null)).toBe('—');
  });
});

describe('formatPct', () => {
  it('adds % suffix', () => {
    expect(formatPct(12.5)).toBe('+12.5%');
  });

  it('returns em dash for undefined', () => {
    expect(formatPct(undefined)).toBe('—');
  });
});

describe('truncate', () => {
  it('returns the original string if under limit', () => {
    expect(truncate('Hello', 10)).toBe('Hello');
  });

  it('truncates and appends ellipsis', () => {
    expect(truncate('Hello World', 5)).toBe('Hello…');
  });

  it('returns empty string for null', () => {
    expect(truncate(null, 10)).toBe('');
  });
});

describe('snakeToTitle', () => {
  it('converts snake_case to Title Case', () => {
    expect(snakeToTitle('body_fat_percentage')).toBe('Body Fat Percentage');
  });

  it('handles single word', () => {
    expect(snakeToTitle('weight')).toBe('Weight');
  });
});

describe('slaColourClass', () => {
  it('returns red for breached SLA (past deadline)', () => {
    const past = new Date(Date.now() - 3_600_000).toISOString();
    expect(slaColourClass(past)).toContain('text-red-600');
  });

  it('returns green for SLA with plenty of time', () => {
    const future = new Date(Date.now() + 48 * 3_600_000).toISOString();
    expect(slaColourClass(future)).toContain('text-green-600');
  });

  it('returns slate for null', () => {
    expect(slaColourClass(null)).toContain('text-slate-500');
  });
});
