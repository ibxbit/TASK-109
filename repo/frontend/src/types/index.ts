// ─────────────────────────────────────────────────────────────────────────────
// VitalPath Frontend — Shared TypeScript Types
// These mirror the backend Rust structs / serde-JSON shapes exactly.
// ─────────────────────────────────────────────────────────────────────────────

// ── Auth ─────────────────────────────────────────────────────────────────────

export type Role = 'administrator' | 'care_coach' | 'approver' | 'member';

export const ROLE_IDS: Record<string, Role> = {
  '00000000-0000-0000-0000-000000000001': 'administrator',
  '00000000-0000-0000-0000-000000000002': 'care_coach',
  '00000000-0000-0000-0000-000000000003': 'approver',
  '00000000-0000-0000-0000-000000000004': 'member',
};

export interface UserPublic {
  id: string;
  username: string;
  role_id: string;
  org_unit_id: string | null;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface LoginRequest {
  username: string;
  password: string;
  captcha_token?: string;
  captcha_answer?: number;
}

export interface LoginSuccess {
  token: string;
  user: UserPublic;
  expires_at: string;
}

export interface CaptchaChallenge {
  captcha_required: true;
  captcha_token: string;
  question: string;
}

export interface LockedResponse {
  locked: true;
  locked_until: string;
  message: string;
}

export type LoginResponse = LoginSuccess | CaptchaChallenge | LockedResponse;

// ── Health Profile ────────────────────────────────────────────────────────────

export type Sex = 'male' | 'female' | 'other' | 'prefer_not_to_say';
export type ActivityLevel =
  | 'sedentary'
  | 'lightly_active'
  | 'moderately_active'
  | 'very_active'
  | 'extra_active';

export interface HealthProfile {
  id: string;
  member_id: string;
  date_of_birth: string | null;
  sex: Sex | null;
  height_in: number | null;
  weight_lbs: number | null;
  activity_level: ActivityLevel | null;
  dietary_notes: string | null;
  medical_notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateHealthProfileRequest {
  member_id: string;
  date_of_birth?: string;
  sex?: Sex;
  height_in?: number;
  weight_lbs?: number;
  activity_level?: ActivityLevel;
  dietary_notes?: string;
  medical_notes?: string;
}

export interface UpdateHealthProfileRequest {
  date_of_birth?: string;
  sex?: Sex;
  height_in?: number;
  weight_lbs?: number;
  activity_level?: ActivityLevel;
  dietary_notes?: string;
  medical_notes?: string;
}

// ── Metrics ───────────────────────────────────────────────────────────────────

export type MetricType =
  | 'weight'
  | 'body_fat_percentage'
  | 'waist'
  | 'hip'
  | 'chest'
  | 'blood_glucose';

export const METRIC_UNITS: Record<MetricType, string> = {
  weight:               'lbs',
  body_fat_percentage:  '%',
  waist:                'in',
  hip:                  'in',
  chest:                'in',
  blood_glucose:        'mg/dL',
};

export const METRIC_RANGES: Record<MetricType, { min: number; max: number }> = {
  weight:               { min: 10,  max: 1500 },
  body_fat_percentage:  { min: 1,   max: 70   },
  waist:                { min: 10,  max: 120  },
  hip:                  { min: 10,  max: 120  },
  chest:                { min: 10,  max: 120  },
  blood_glucose:        { min: 50,  max: 600  },
};

export interface MetricEntry {
  id: string;
  member_id: string;
  metric_type: MetricType;
  value: number;
  entry_date: string;
  recorded_by: string | null;
  notes: string | null;
  created_at: string;
}

export interface CreateMetricEntryRequest {
  member_id: string;
  metric_type: MetricType;
  value: number;
  entry_date: string;
  notes?: string;
}

export interface MetricSummary {
  metric_type: MetricType;
  count: number;
  first_value: number | null;
  latest_value: number | null;
  min_value: number | null;
  max_value: number | null;
  avg_value: number | null;
  change: number | null;
  change_pct: number | null;
}

export type MetricRange = '7d' | '30d' | '90d' | 'all';

export interface MetricQueryParams {
  member_id: string;
  metric_type?: MetricType;
  range?: MetricRange;
  start?: string;
  end?: string;
}

// ── Goals ─────────────────────────────────────────────────────────────────────

export type GoalType = 'fat_loss' | 'muscle_gain' | 'glucose_control';
export type GoalStatus = 'active' | 'paused' | 'completed' | 'cancelled';

export interface Goal {
  id: string;
  member_id: string;
  title: string;
  description: string | null;
  goal_type: GoalType;
  status: GoalStatus;
  target_value: number;
  target_date: string | null;
  baseline_value: number | null;
  start_date: string;
  assigned_by: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateGoalRequest {
  member_id: string;
  title: string;
  description?: string;
  goal_type: GoalType;
  target_value: number;
  target_date?: string;
  baseline_value?: number;
  start_date?: string;
}

export interface UpdateGoalRequest {
  title?: string;
  description?: string;
  status?: GoalStatus;
  target_value?: number;
  target_date?: string;
}

// ── Work Orders ───────────────────────────────────────────────────────────────

export type WorkOrderStatus =
  | 'intake'
  | 'triage'
  | 'in_progress'
  | 'waiting_on_member'
  | 'resolved'
  | 'closed';

export type WorkOrderPriority = 'low' | 'medium' | 'high' | 'urgent';
export type TicketType =
  | 'health_query'
  | 'equipment'
  | 'scheduling'
  | 'nutrition'
  | 'emergency';

export const WORK_ORDER_STATUS_FLOW: WorkOrderStatus[] = [
  'intake',
  'triage',
  'in_progress',
  'waiting_on_member',
  'resolved',
  'closed',
];

export interface WorkOrder {
  id: string;
  member_id: string;
  title: string;
  description: string | null;
  priority: WorkOrderPriority;
  status: WorkOrderStatus;
  ticket_type: TicketType;
  assigned_to: string | null;
  routed_to_org_unit_id: string | null;
  created_by: string;
  due_date: string | null;
  processing_notes: string | null;
  resolved_at: string | null;
  closed_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateWorkOrderRequest {
  member_id: string;
  title: string;
  description?: string;
  priority?: WorkOrderPriority;
  ticket_type: TicketType;
  due_date?: string;
}

export interface TransitionWorkOrderRequest {
  new_status: WorkOrderStatus;
  processing_notes?: string;
  assigned_to?: string;
}

// ── Workflows ─────────────────────────────────────────────────────────────────

export type WorkflowStatus = 'pending' | 'approved' | 'rejected' | 'completed';
export type ApprovalStatus = 'pending' | 'approved' | 'rejected';
export type RiskTier = 'low' | 'medium' | 'high';

export interface WorkflowTemplate {
  id: string;
  name: string;
  description: string | null;
  business_type: string;
  org_unit_id: string;
  risk_tier: RiskTier;
  amount_tier: RiskTier | null;
  is_active: boolean;
  created_at: string;
}

export interface WorkflowNode {
  id: string;
  template_id: string;
  name: string;
  node_order: number;
  is_parallel: boolean;
  action_type: string;
  created_at: string;
}

export interface Approval {
  id: string;
  approver_id: string;
  assignee_id: string | null;
  node_id: string;
  status: ApprovalStatus;
  sla_deadline: string;
  sla_breached: boolean;
  decision_at: string | null;
  created_at: string;
}

export interface WorkflowInstance {
  id: string;
  template_id: string;
  initiated_by: string;
  status: WorkflowStatus;
  current_stage: number;
  approvals: Approval[];
  created_at: string;
}

export interface StartWorkflowRequest {
  template_id: string;
}

export interface WorkflowActionRequest {
  action: 'approve' | 'reject' | 'reassign';
  approval_id: string;
  comment?: string;
  reassign_to?: string;
}

// ── Notifications ─────────────────────────────────────────────────────────────

export type NotificationEventType =
  | 'manual'
  | 'goal_completed'
  | 'metric_milestone'
  | 'health_alert'
  | 'work_order_update';

export interface Notification {
  id: string;
  user_id: string;
  title: string;
  body: string;
  event_type: NotificationEventType;
  template_id: string | null;
  entity_type: string | null;
  entity_id: string | null;
  is_read: boolean;
  read_at: string | null;
  created_at: string;
}

export interface NotificationSubscription {
  id: string;
  user_id: string;
  event_type: NotificationEventType;
  is_subscribed: boolean;
  created_at: string;
  updated_at: string;
}

export interface NotificationSchedule {
  id: string;
  user_id: string;
  template_id: string;
  label: string;
  fire_hour: number;
  tz_offset_minutes: number;
  is_active: boolean;
  last_fired_at: string | null;
  next_fire_at: string;
  created_at: string;
}

// ── Analytics ─────────────────────────────────────────────────────────────────

export interface AnalyticsMetrics {
  attendance:   Record<string, number>;
  cancellation: Record<string, number>;
  conversion:   Record<string, number>;
  distribution: Record<string, number>;
  popularity:   Record<string, number>;
}

export interface AnalyticsReport {
  query_params: {
    org_unit_id: string | null;
    start_date: string;
    end_date: string;
  };
  member_count: number;
  metrics: AnalyticsMetrics;
}

export interface ExportRequest {
  org_unit_id?: string;
  start_date: string;
  end_date: string;
  format?: 'xlsx';
}

// ── Audit Logs ────────────────────────────────────────────────────────────────

export interface AuditLog {
  id: string;
  actor_id: string | null;
  action: string;
  entity_type: string | null;
  entity_id: string | null;
  reason_code: string | null;
  old_value: unknown;
  new_value: unknown;
  ip_address: string | null;
  user_agent: string | null;
  created_at: string;
}

export interface AuditLogPage {
  items: AuditLog[];
  total: number;
  page: number;
  per_page: number;
}

export interface AuditLogQueryParams {
  actor_id?: string;
  action?: string;
  entity_type?: string;
  entity_id?: string;
  start_date?: string;
  end_date?: string;
  page?: number;
  per_page?: number;
}

// ── API Error ─────────────────────────────────────────────────────────────────

export interface ApiError {
  status: number;
  message: string;
  code?: string;
}

// ── Pagination helpers ────────────────────────────────────────────────────────

export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  per_page: number;
}
