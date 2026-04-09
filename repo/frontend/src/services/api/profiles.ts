import apiClient from './client';
import type {
  HealthProfile,
  CreateHealthProfileRequest,
  UpdateHealthProfileRequest,
} from '../../types';

// ── POST /profile ─────────────────────────────────────────────────────────────

export async function createProfile(
  payload: CreateHealthProfileRequest,
): Promise<HealthProfile> {
  const { data } = await apiClient.post<HealthProfile>('/profile', payload);
  return data;
}

// ── GET /profile/:member_id ───────────────────────────────────────────────────

export async function getProfile(memberId: string): Promise<HealthProfile> {
  const { data } = await apiClient.get<HealthProfile>(`/profile/${memberId}`);
  return data;
}

// ── PUT /profile/:member_id ───────────────────────────────────────────────────

export async function updateProfile(
  memberId: string,
  payload: UpdateHealthProfileRequest,
): Promise<HealthProfile> {
  const { data } = await apiClient.put<HealthProfile>(
    `/profile/${memberId}`,
    payload,
  );
  return data;
}
