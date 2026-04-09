import apiClient from './client';
import type {
  WorkOrder,
  CreateWorkOrderRequest,
  TransitionWorkOrderRequest,
  WorkOrderStatus,
} from '../../types';

// ── POST /work-orders ─────────────────────────────────────────────────────────

export async function createWorkOrder(
  payload: CreateWorkOrderRequest,
): Promise<WorkOrder> {
  const { data } = await apiClient.post<WorkOrder>('/work-orders', payload);
  return data;
}

// ── GET /work-orders ──────────────────────────────────────────────────────────

export async function getWorkOrders(params: {
  member_id?:   string;
  assigned_to?: string;
  status?:      WorkOrderStatus;
}): Promise<WorkOrder[]> {
  const { data } = await apiClient.get<WorkOrder[]>('/work-orders', { params });
  return data;
}

// ── PATCH /work-orders/:id/transition ────────────────────────────────────────

export async function transitionWorkOrder(
  workOrderId: string,
  payload: TransitionWorkOrderRequest,
): Promise<WorkOrder> {
  const { data } = await apiClient.patch<WorkOrder>(
    `/work-orders/${workOrderId}/transition`,
    payload,
  );
  return data;
}
