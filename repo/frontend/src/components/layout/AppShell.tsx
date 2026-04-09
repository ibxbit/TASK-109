import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';
import { TopHeader } from './TopHeader';
import { ErrorBoundary } from '../ui/ErrorBoundary';

/**
 * Main application shell — sidebar + header + scrollable content area.
 * Only rendered for authenticated routes (wrapped by ProtectedRoute in App.tsx).
 */
export function AppShell() {
  return (
    <div className="flex h-screen overflow-hidden bg-slate-50">
      <Sidebar />
      <div className="flex flex-col flex-1 overflow-hidden">
        <TopHeader />
        <main className="flex-1 overflow-y-auto">
          <ErrorBoundary>
            <Outlet />
          </ErrorBoundary>
        </main>
      </div>
    </div>
  );
}
