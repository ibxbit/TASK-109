import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { UserPublic } from '../types';
import { TOKEN_KEY, USER_KEY, ROLE_IDS } from '../utils/constants';

interface AuthState {
  token: string | null;
  user:  UserPublic | null;

  // Derived helpers
  isAuthenticated: boolean;
  isAdmin:     () => boolean;
  isCareCoach: () => boolean;
  isApprover:  () => boolean;
  isMember:    () => boolean;
  hasAnyRole:  (...roles: string[]) => boolean;

  // Actions
  setCredentials: (token: string, user: UserPublic) => void;
  clearCredentials: () => void;
  updateUser: (user: UserPublic) => void;
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set, get) => ({
      token: null,
      user:  null,
      isAuthenticated: false,

      isAdmin:     () => get().user?.role_id === ROLE_IDS.ADMINISTRATOR,
      isCareCoach: () => get().user?.role_id === ROLE_IDS.CARE_COACH,
      isApprover:  () => get().user?.role_id === ROLE_IDS.APPROVER,
      isMember:    () => get().user?.role_id === ROLE_IDS.MEMBER,

      hasAnyRole: (...roles: string[]) => {
        const roleId = get().user?.role_id;
        return roleId ? roles.includes(roleId) : false;
      },

      setCredentials: (token, user) => {
        localStorage.setItem(TOKEN_KEY, token);
        localStorage.setItem(USER_KEY, JSON.stringify(user));
        set({ token, user, isAuthenticated: true });
      },

      clearCredentials: () => {
        localStorage.removeItem(TOKEN_KEY);
        localStorage.removeItem(USER_KEY);
        set({ token: null, user: null, isAuthenticated: false });
      },

      updateUser: (user) => {
        localStorage.setItem(USER_KEY, JSON.stringify(user));
        set({ user });
      },
    }),
    {
      name: 'vp-auth',
      // Only persist token + user; the derived flags are computed on hydration.
      partialize: (state) => ({ token: state.token, user: state.user }),
      // Re-compute isAuthenticated after hydration from storage.
      onRehydrateStorage: () => (state) => {
        if (state) {
          state.isAuthenticated = !!(state.token && state.user);
        }
      },
    },
  ),
);
