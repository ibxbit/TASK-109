import apiClient from './client';
import type {
  WorkflowTemplate,
  WorkflowNode,
  WorkflowInstance,
  StartWorkflowRequest,
  WorkflowActionRequest,
} from '../../types';

// ── POST /workflows/templates ─────────────────────────────────────────────────

export async function createWorkflowTemplate(payload: {
  name: string;
  description?: string;
  business_type: string;
  org_unit_id: string;
  risk_tier: string;
  amount_tier?: string;
}): Promise<WorkflowTemplate> {
  const { data } = await apiClient.post<WorkflowTemplate>(
    '/workflows/templates',
    payload,
  );
  return data;
}

// ── POST /workflows/templates/:id/nodes ──────────────────────────────────────

export async function addWorkflowNode(
  templateId: string,
  payload: {
    name: string;
    node_order: number;
    is_parallel?: boolean;
    action_type: string;
  },
): Promise<WorkflowNode> {
  const { data } = await apiClient.post<WorkflowNode>(
    `/workflows/templates/${templateId}/nodes`,
    payload,
  );
  return data;
}

// ── POST /workflows/instances ─────────────────────────────────────────────────

export async function startWorkflowInstance(
  payload: StartWorkflowRequest,
): Promise<WorkflowInstance> {
  const { data } = await apiClient.post<WorkflowInstance>(
    '/workflows/instances',
    payload,
  );
  return data;
}

// ── GET /workflows/instances/:id ─────────────────────────────────────────────

export async function getWorkflowInstance(
  instanceId: string,
): Promise<WorkflowInstance> {
  const { data } = await apiClient.get<WorkflowInstance>(
    `/workflows/instances/${instanceId}`,
  );
  return data;
}

// ── POST /workflows/instances/:id/actions ────────────────────────────────────

export async function takeWorkflowAction(
  instanceId: string,
  payload: WorkflowActionRequest,
): Promise<WorkflowInstance> {
  const { data } = await apiClient.post<WorkflowInstance>(
    `/workflows/instances/${instanceId}/actions`,
    payload,
  );
  return data;
}

// ── GET /workflows/instances (list — not in backend spec but needed for UI) ──
// The backend returns instances when queried via the approvals flow.
// We list by fetching instances where current user is an approver.
// If the backend exposes a list endpoint, update this accordingly.

export async function listWorkflowInstances(): Promise<WorkflowInstance[]> {
  // The backend does not expose a list endpoint; return empty until implemented.
  // In a real deployment, add GET /workflows/instances to the Rust backend.
  return [];
}

// ── GET /workflows/templates ──────────────────────────────────────────────────
// Similarly there is no list endpoint for templates in the backend spec.
// Placeholder until the backend exposes it.

export async function listWorkflowTemplates(): Promise<WorkflowTemplate[]> {
  return [];
}
