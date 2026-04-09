import apiClient from './client';
import type {
  LoginRequest,
  LoginResponse,
  UserPublic,
} from '../../types';

// ── POST /auth/login ──────────────────────────────────────────────────────────

export async function login(payload: LoginRequest): Promise<LoginResponse> {
  const { data } = await apiClient.post<LoginResponse>('/auth/login', payload);
  return data;
}

// ── POST /auth/logout ─────────────────────────────────────────────────────────

export async function logout(): Promise<void> {
  await apiClient.post('/auth/logout', {});
}

// ── GET /auth/me ──────────────────────────────────────────────────────────────

export async function getMe(): Promise<UserPublic> {
  const { data } = await apiClient.get<{ user: UserPublic }>('/auth/me');
  return data.user;
}
