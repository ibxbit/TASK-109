import { Navigate, Outlet, Route, Routes, useLocation } from 'react-router-dom';
import { useAuthStore } from './store/authStore';
import { AppShell } from './components/layout/AppShell';
import { LoginPage } from './pages/auth/LoginPage';
import { MembersPage } from './pages/members/MembersPage';
import { GoalsPage } from './pages/goals/GoalsPage';
import { WorkOrdersPage } from './pages/work-orders/WorkOrdersPage';
import { WorkflowsPage } from './pages/workflows/WorkflowsPage';
import { AnalyticsPage } from './pages/analytics/AnalyticsPage';
import { AuditLogsPage } from './pages/audit/AuditLogsPage';
import { NotificationsPage } from './pages/notifications/NotificationsPage';
import { ROLE_IDS } from './utils/constants';

// ── Route guards ──────────────────────────────────────────────────────────────

function RequireAuth() {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  const location = useLocation();

  if (!isAuthenticated) {
    return <Navigate to="/login" state={{ from: location }} replace />;
  }
  return <Outlet />;
}

interface RequireRoleProps {
  allowRoles: string[];
}

function RequireRole({ allowRoles }: RequireRoleProps) {
  const roleId = useAuthStore((s) => s.user?.role_id);

  if (!roleId || !allowRoles.includes(roleId)) {
    return (
      <div className="flex items-center justify-center min-h-[60vh] text-center p-8">
        <div>
          <p className="text-4xl font-bold text-slate-300">403</p>
          <p className="text-lg font-semibold text-slate-700 mt-2">Access Denied</p>
          <p className="text-sm text-slate-500 mt-1">
            Your role does not have permission to view this page.
          </p>
        </div>
      </div>
    );
  }
  return <Outlet />;
}

// ── Default redirect based on role ───────────────────────────────────────────

function DefaultRedirect() {
  const roleId = useAuthStore((s) => s.user?.role_id);

  switch (roleId) {
    case ROLE_IDS.ADMINISTRATOR:
    case ROLE_IDS.CARE_COACH:
      return <Navigate to="/members" replace />;
    case ROLE_IDS.APPROVER:
      return <Navigate to="/workflows" replace />;
    case ROLE_IDS.MEMBER:
      return <Navigate to="/goals" replace />;
    default:
      return <Navigate to="/login" replace />;
  }
}

// ── App ───────────────────────────────────────────────────────────────────────

export function App() {
  const ALL_ROLES = Object.values(ROLE_IDS);
  const STAFF     = [ROLE_IDS.ADMINISTRATOR, ROLE_IDS.CARE_COACH];
  const APPROVERS = [ROLE_IDS.ADMINISTRATOR, ROLE_IDS.APPROVER];
  const ADMIN     = [ROLE_IDS.ADMINISTRATOR];

  return (
    <Routes>
      {/* Public */}
      <Route path="/login" element={<LoginPage />} />

      {/* Protected */}
      <Route element={<RequireAuth />}>
        <Route element={<AppShell />}>
          {/* Default redirect */}
          <Route index element={<DefaultRedirect />} />

          {/* Members & Profiles — Admin / Care Coach */}
          <Route element={<RequireRole allowRoles={STAFF} />}>
            <Route path="members" element={<MembersPage />} />
          </Route>

          {/* Goals — All authenticated roles */}
          <Route element={<RequireRole allowRoles={ALL_ROLES} />}>
            <Route path="goals" element={<GoalsPage />} />
          </Route>

          {/* Work Orders — Admin / Care Coach */}
          <Route element={<RequireRole allowRoles={STAFF} />}>
            <Route path="work-orders" element={<WorkOrdersPage />} />
          </Route>

          {/* Workflows — Admin / Approver */}
          <Route element={<RequireRole allowRoles={APPROVERS} />}>
            <Route path="workflows" element={<WorkflowsPage />} />
          </Route>

          {/* Analytics — Admin / Care Coach */}
          <Route element={<RequireRole allowRoles={STAFF} />}>
            <Route path="analytics" element={<AnalyticsPage />} />
          </Route>

          {/* Audit Logs — Admin only */}
          <Route element={<RequireRole allowRoles={ADMIN} />}>
            <Route path="audit-logs" element={<AuditLogsPage />} />
          </Route>

          {/* Notifications — All roles */}
          <Route element={<RequireRole allowRoles={ALL_ROLES} />}>
            <Route path="notifications" element={<NotificationsPage />} />
          </Route>
        </Route>
      </Route>

      {/* Catch-all */}
      <Route
        path="*"
        element={
          <div className="flex items-center justify-center min-h-screen bg-slate-50 text-center p-8">
            <div>
              <p className="text-5xl font-bold text-slate-200">404</p>
              <p className="text-xl font-semibold text-slate-700 mt-3">Page not found</p>
              <a
                href="/"
                className="mt-4 inline-block text-sm text-brand-600 hover:underline"
              >
                Go home
              </a>
            </div>
          </div>
        }
      />
    </Routes>
  );
}
