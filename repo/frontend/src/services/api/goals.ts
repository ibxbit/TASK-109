import apiClient from './client';
import type {
  Goal,
  CreateGoalRequest,
  UpdateGoalRequest,
  GoalStatus,
} from '../../types';

// ── POST /goals ───────────────────────────────────────────────────────────────

export async function createGoal(payload: CreateGoalRequest): Promise<Goal> {
  const { data } = await apiClient.post<Goal>('/goals', payload);
  return data;
}

// ── GET /goals ────────────────────────────────────────────────────────────────

export async function getGoals(params: {
  member_id?: string;
  status?: GoalStatus;
}): Promise<Goal[]> {
  const { data } = await apiClient.get<Goal[]>('/goals', { params });
  return data;
}

// ── PUT /goals/:id ────────────────────────────────────────────────────────────

export async function updateGoal(
  goalId: string,
  payload: UpdateGoalRequest,
): Promise<Goal> {
  const { data } = await apiClient.put<Goal>(`/goals/${goalId}`, payload);
  return data;
}
