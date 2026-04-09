import { describe, it, expect } from 'vitest';
import { loginSchema, createGoalSchema, createWorkOrderSchema, metricEntrySchema } from '../../utils/validators';

describe('loginSchema', () => {
  it('passes valid credentials', () => {
    const result = loginSchema.safeParse({ username: 'admin', password: 'secret' });
    expect(result.success).toBe(true);
  });

  it('rejects empty username', () => {
    const result = loginSchema.safeParse({ username: '', password: 'secret' });
    expect(result.success).toBe(false);
    expect(result.error?.issues[0].path[0]).toBe('username');
  });

  it('rejects empty password', () => {
    const result = loginSchema.safeParse({ username: 'admin', password: '' });
    expect(result.success).toBe(false);
    expect(result.error?.issues[0].path[0]).toBe('password');
  });
});

describe('metricEntrySchema', () => {
  it('passes valid weight entry', () => {
    const schema = metricEntrySchema('weight');
    const result = schema.safeParse({
      member_id:   '11111111-1111-1111-1111-111111111111',
      metric_type: 'weight',
      value:       180,
      entry_date:  '2024-03-15',
    });
    expect(result.success).toBe(true);
  });

  it('rejects weight below minimum (10)', () => {
    const schema = metricEntrySchema('weight');
    const result = schema.safeParse({
      member_id:   '11111111-1111-1111-1111-111111111111',
      metric_type: 'weight',
      value:       5,
      entry_date:  '2024-03-15',
    });
    expect(result.success).toBe(false);
  });

  it('rejects blood_glucose above maximum (600)', () => {
    const schema = metricEntrySchema('blood_glucose');
    const result = schema.safeParse({
      member_id:   '11111111-1111-1111-1111-111111111111',
      metric_type: 'blood_glucose',
      value:       700,
      entry_date:  '2024-03-15',
    });
    expect(result.success).toBe(false);
  });

  it('rejects invalid date format', () => {
    const schema = metricEntrySchema('weight');
    const result = schema.safeParse({
      member_id:   '11111111-1111-1111-1111-111111111111',
      metric_type: 'weight',
      value:       180,
      entry_date:  '15/03/2024',
    });
    expect(result.success).toBe(false);
  });
});

describe('createGoalSchema', () => {
  it('passes valid goal', () => {
    const result = createGoalSchema.safeParse({
      member_id:    '11111111-1111-1111-1111-111111111111',
      title:        'Lose 10 lbs',
      goal_type:    'fat_loss',
      target_value: 25.0,
    });
    expect(result.success).toBe(true);
  });

  it('rejects invalid member UUID', () => {
    const result = createGoalSchema.safeParse({
      member_id:    'not-a-uuid',
      title:        'Lose 10 lbs',
      goal_type:    'fat_loss',
      target_value: 25.0,
    });
    expect(result.success).toBe(false);
    expect(result.error?.issues[0].path[0]).toBe('member_id');
  });

  it('rejects missing title', () => {
    const result = createGoalSchema.safeParse({
      member_id:    '11111111-1111-1111-1111-111111111111',
      title:        '',
      goal_type:    'fat_loss',
      target_value: 25.0,
    });
    expect(result.success).toBe(false);
  });
});

describe('createWorkOrderSchema', () => {
  it('passes valid work order', () => {
    const result = createWorkOrderSchema.safeParse({
      member_id:   '11111111-1111-1111-1111-111111111111',
      title:       'Need help with diet plan',
      ticket_type: 'nutrition',
      priority:    'medium',
    });
    expect(result.success).toBe(true);
  });

  it('rejects title exceeding 300 characters', () => {
    const result = createWorkOrderSchema.safeParse({
      member_id:   '11111111-1111-1111-1111-111111111111',
      title:       'A'.repeat(301),
      ticket_type: 'nutrition',
    });
    expect(result.success).toBe(false);
  });
});
