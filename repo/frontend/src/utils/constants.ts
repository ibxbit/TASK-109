// ─────────────────────────────────────────────────────────────────────────────
// Application-wide constants
// ─────────────────────────────────────────────────────────────────────────────

export const APP_NAME = import.meta.env.VITE_APP_NAME ?? 'VitalPath Health Ops';

/** Auth token key in localStorage */
export const TOKEN_KEY = 'vp_token';
/** Stored user JSON key in localStorage */
export const USER_KEY = 'vp_user';

/** Role UUID → display name mapping */
export const ROLE_LABELS: Record<string, string> = {
  '00000000-0000-0000-0000-000000000001': 'Administrator',
  '00000000-0000-0000-0000-000000000002': 'Care Coach',
  '00000000-0000-0000-0000-000000000003': 'Approver',
  '00000000-0000-0000-0000-000000000004': 'Member',
};

/** Role UUIDs */
export const ROLE_IDS = {
  ADMINISTRATOR: '00000000-0000-0000-0000-000000000001',
  CARE_COACH:    '00000000-0000-0000-0000-000000000002',
  APPROVER:      '00000000-0000-0000-0000-000000000003',
  MEMBER:        '00000000-0000-0000-0000-000000000004',
} as const;

/** Max characters for dietary_notes / medical_notes (backend validated at DB) */
export const NOTES_MAX_LENGTH = 1000;

/** Max title length for work orders */
export const WORK_ORDER_TITLE_MAX = 300;

/** Metric type display labels */
export const METRIC_TYPE_LABELS: Record<string, string> = {
  weight:               'Weight',
  body_fat_percentage:  'Body Fat %',
  waist:                'Waist',
  hip:                  'Hip',
  chest:                'Chest',
  blood_glucose:        'Blood Glucose',
};

/** Goal type display labels */
export const GOAL_TYPE_LABELS: Record<string, string> = {
  fat_loss:        'Fat Loss',
  muscle_gain:     'Muscle Gain',
  glucose_control: 'Glucose Control',
};

/** Work-order status display labels */
export const WORK_ORDER_STATUS_LABELS: Record<string, string> = {
  intake:            'Intake',
  triage:            'Triage',
  in_progress:       'In Progress',
  waiting_on_member: 'Waiting on Member',
  resolved:          'Resolved',
  closed:            'Closed',
};

/** Work-order priority badge colours (Tailwind classes) */
export const PRIORITY_CLASSES: Record<string, string> = {
  low:    'bg-slate-100 text-slate-700',
  medium: 'bg-blue-100 text-blue-700',
  high:   'bg-orange-100 text-orange-700',
  urgent: 'bg-red-100 text-red-700',
};

/** Workflow instance status labels */
export const WORKFLOW_STATUS_LABELS: Record<string, string> = {
  pending:   'Pending',
  approved:  'Approved',
  rejected:  'Rejected',
  completed: 'Completed',
};

/** Notification event type labels */
export const EVENT_TYPE_LABELS: Record<string, string> = {
  manual:            'Manual',
  goal_completed:    'Goal Completed',
  metric_milestone:  'Metric Milestone',
  health_alert:      'Health Alert',
  work_order_update: 'Work Order Update',
};

/** SLA: 48 business hours shown in the UI */
export const SLA_BUSINESS_HOURS = 48;
