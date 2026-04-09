import { useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { useMutation } from '@tanstack/react-query';
import { useAuthStore } from '../store/authStore';
import * as authApi from '../services/api/auth';
import type { LoginRequest, LoginSuccess, CaptchaChallenge, LockedResponse } from '../types';

/**
 * Centralised auth hook.
 * Provides login, logout, and the current session state.
 */
export function useAuth() {
  const navigate = useNavigate();
  const store = useAuthStore();

  // ── Login ────────────────────────────────────────────────────────────────

  const loginMutation = useMutation({
    mutationFn: (payload: LoginRequest) => authApi.login(payload),
    onSuccess: (response) => {
      // Response is a discriminated union — check for token to detect LoginSuccess
      if ('token' in response) {
        const success = response as LoginSuccess;
        store.setCredentials(success.token, success.user);
        navigate('/');
      }
      // CaptchaChallenge and LockedResponse are returned as resolved values —
      // the caller must inspect the response to handle them.
    },
  });

  const login = useCallback(
    (payload: LoginRequest) => loginMutation.mutateAsync(payload),
    [loginMutation],
  );

  // ── Logout ───────────────────────────────────────────────────────────────

  const logoutMutation = useMutation({
    mutationFn: () => authApi.logout(),
    onSettled: () => {
      // Always clear local credentials, even if the backend call fails
      store.clearCredentials();
      navigate('/login');
    },
  });

  const logout = useCallback(
    () => logoutMutation.mutate(),
    [logoutMutation],
  );

  // ── Discriminators ───────────────────────────────────────────────────────

  function isCaptchaChallenge(r: unknown): r is CaptchaChallenge {
    return typeof r === 'object' && r !== null && 'captcha_required' in r;
  }

  function isLockedResponse(r: unknown): r is LockedResponse {
    return typeof r === 'object' && r !== null && 'locked' in r;
  }

  return {
    user:            store.user,
    token:           store.token,
    isAuthenticated: store.isAuthenticated,
    isAdmin:         store.isAdmin,
    isCareCoach:     store.isCareCoach,
    isApprover:      store.isApprover,
    isMember:        store.isMember,
    hasAnyRole:      store.hasAnyRole,

    login,
    logout,
    loginPending: loginMutation.isPending,
    loginError:   loginMutation.error as { message: string } | null,

    isCaptchaChallenge,
    isLockedResponse,
  };
}
