import { z } from 'zod';
import { NOTES_MAX_LENGTH, WORK_ORDER_TITLE_MAX } from './constants';
import type { MetricType } from '../types';
import { METRIC_RANGES } from '../types';

// ── Auth ──────────────────────────────────────────────────────────────────────

export const loginSchema = z.object({
  username: z.string().min(1, 'Username is required'),
  password: z.string().min(1, 'Password is required'),
  captcha_answer: z.number().optional(),
  captcha_token:  z.string().optional(),
});

export type LoginFormValues = z.infer<typeof loginSchema>;

// ── Health Profile ────────────────────────────────────────────────────────────

export const healthProfileSchema = z.object({
  date_of_birth: z.string().regex(/^\d{4}-\d{2}-\d{2}$/, 'Must be YYYY-MM-DD').optional().or(z.literal('')),
  sex: z.enum(['male', 'female', 'other', 'prefer_not_to_say']).optional(),
  height_in:      z.coerce.number().min(12).max(120).optional(),
  weight_lbs:     z.coerce.number().min(10).max(1500).optional(),
  activity_level: z.enum([
    'sedentary',
    'lightly_active',
    'moderately_active',
    'very_active',
    'extra_active',
  ]).optional(),
  dietary_notes: z
    .string()
    .max(NOTES_MAX_LENGTH, `Max ${NOTES_MAX_LENGTH} characters`)
    .optional(),
  medical_notes: z
    .string()
    .max(NOTES_MAX_LENGTH, `Max ${NOTES_MAX_LENGTH} characters`)
    .optional(),
});

export type HealthProfileFormValues = z.infer<typeof healthProfileSchema>;

// ── Metric Entry ──────────────────────────────────────────────────────────────

export function metricEntrySchema(metricType: MetricType | '') {
  const range = metricType ? METRIC_RANGES[metricType] : { min: 0, max: 99999 };
  return z.object({
    member_id:   z.string().uuid('Must be a valid member ID'),
    metric_type: z.enum([
      'weight',
      'body_fat_percentage',
      'waist',
      'hip',
      'chest',
      'blood_glucose',
    ] as const),
    value: z.coerce
      .number({ invalid_type_error: 'Value must be a number' })
      .min(range.min, `Minimum value is ${range.min}`)
      .max(range.max, `Maximum value is ${range.max}`),
    entry_date: z
      .string()
      .regex(/^\d{4}-\d{2}-\d{2}$/, 'Must be YYYY-MM-DD'),
    notes: z.string().max(500, 'Max 500 characters').optional(),
  });
}

export type MetricEntryFormValues = ReturnType<typeof metricEntrySchema>['_type'];

// ── Goal ──────────────────────────────────────────────────────────────────────

export const createGoalSchema = z.object({
  member_id:       z.string().uuid('Must be a valid member ID'),
  title:           z.string().min(1, 'Title is required').max(200, 'Max 200 characters'),
  description:     z.string().max(1000, 'Max 1000 characters').optional(),
  goal_type:       z.enum(['fat_loss', 'muscle_gain', 'glucose_control'] as const),
  target_value:    z.coerce.number({ invalid_type_error: 'Required' }),
  target_date:     z.string().regex(/^\d{4}-\d{2}-\d{2}$/, 'Must be YYYY-MM-DD').optional().or(z.literal('')),
  baseline_value:  z.coerce.number().optional(),
  start_date:      z.string().regex(/^\d{4}-\d{2}-\d{2}$/, 'Must be YYYY-MM-DD').optional().or(z.literal('')),
});

export type CreateGoalFormValues = z.infer<typeof createGoalSchema>;

// ── Work Order ────────────────────────────────────────────────────────────────

export const createWorkOrderSchema = z.object({
  member_id:   z.string().uuid('Must be a valid member ID'),
  title:       z.string().min(1, 'Title is required').max(WORK_ORDER_TITLE_MAX, `Max ${WORK_ORDER_TITLE_MAX} characters`),
  description: z.string().max(2000, 'Max 2000 characters').optional(),
  priority:    z.enum(['low', 'medium', 'high', 'urgent'] as const).default('medium'),
  ticket_type: z.enum(['health_query', 'equipment', 'scheduling', 'nutrition', 'emergency'] as const),
  due_date:    z.string().regex(/^\d{4}-\d{2}-\d{2}$/, 'Must be YYYY-MM-DD').optional().or(z.literal('')),
});

export type CreateWorkOrderFormValues = z.infer<typeof createWorkOrderSchema>;

export const transitionWorkOrderSchema = z.object({
  new_status:        z.enum(['intake', 'triage', 'in_progress', 'waiting_on_member', 'resolved', 'closed'] as const),
  processing_notes:  z.string().max(2000, 'Max 2000 characters').optional(),
  assigned_to:       z.string().uuid().optional().or(z.literal('')),
});

export type TransitionWorkOrderFormValues = z.infer<typeof transitionWorkOrderSchema>;
