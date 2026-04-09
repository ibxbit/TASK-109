# VitalPath Health Operations — Frontend

A production-ready React/TypeScript single-page application that serves as the client
interface for the VitalPath Health Operations Rust/Actix-web backend.

---

## Tech Stack

| Layer | Library |
|---|---|
| Framework | React 18 + TypeScript |
| Build tool | Vite 5 |
| Routing | React Router v6 |
| Server state | TanStack Query v5 |
| Client state | Zustand v5 |
| API client | Axios |
| Forms | React Hook Form + Zod |
| Charts | Recharts |
| Styling | Tailwind CSS v3 |
| Icons | Lucide React |
| Unit tests | Vitest + React Testing Library |
| API mocking | MSW v2 |

---

## Prerequisites

- **Node.js** >= 18.x (LTS recommended)
- **npm** >= 9.x (comes with Node 18+)
- The **Rust/Actix-web backend** running and reachable (default: `http://localhost:8080`)

---

## Installation

```bash
# From the repo root
cd frontend

# Install all dependencies
npm install
```

---

## Development Server

```bash
# Start the Vite dev server with hot-module replacement
npm run dev
```

The app will be available at **http://localhost:5173**.

During development, all requests to `/api/*` are proxied to the backend via the Vite
dev-server proxy, which avoids browser CORS preflight issues.

> **Note on CORS:** The Rust backend does not ship with CORS headers configured.
> The Vite proxy handles this transparently in development. For production deployment,
> you must either:
> (a) Serve both frontend and backend on the same origin, OR
> (b) Add `actix-cors` middleware to the Rust backend and configure allowed origins.

---

## Environment Configuration

Copy `.env.example` to `.env` and adjust as needed:

```bash
cp .env.example .env
```

| Variable | Default | Description |
|---|---|---|
| `VITE_API_BASE_URL` | `http://localhost:8080` | Backend origin (used by Vite proxy only — not bundled into the JS) |
| `VITE_APP_NAME` | `VitalPath Health Operations` | Display name shown in the header |

---

## Production Build

```bash
# Type-check and build optimised static assets to dist/
npm run build

# Preview the production build locally
npm run preview
```

The `dist/` directory contains the static assets. Point your web server (nginx, Caddy,
etc.) at `dist/` and configure it to serve `index.html` for all routes (SPA fallback).

---

## Running Tests

```bash
# Run all tests once and print results
npm test

# Run tests in watch mode (re-runs on file changes)
npm run test:watch

# Run tests with V8 coverage report (output in coverage/)
npm run test:coverage

# Open the interactive Vitest UI
npm run test:ui
```

### Test Structure

```
src/tests/
├── setup.ts               # Global test setup (jest-dom matchers, MSW server)
├── utils/
│   ├── formatters.test.ts  # Unit tests for date/number formatters
│   └── validators.test.ts  # Unit tests for Zod schemas
├── api/
│   └── client.test.ts      # Axios client interceptor tests (401/403/500 handling)
└── components/
    ├── LoginPage.test.tsx   # Login form flow (success, CAPTCHA, lockout)
    └── MetricEntry.test.tsx # Add Metric Entry form flow
```

---

## Project Structure

```
src/
├── components/
│   ├── charts/         # Recharts wrappers (MetricChart, etc.)
│   ├── layout/         # AppShell, Sidebar, TopHeader
│   └── ui/             # Headless/primitive UI components
├── hooks/              # Custom React hooks
├── pages/              # Route-level page components
│   ├── auth/           # Login page
│   ├── members/        # Member list + health profile detail
│   ├── goals/          # Goals management
│   ├── work-orders/    # Work-order kanban board
│   ├── workflows/      # Workflow approvals
│   ├── analytics/      # Reports dashboard + CSV/XLSX export
│   └── audit/          # Audit log viewer (Admin only)
├── services/
│   └── api/            # Axios service modules — one file per backend resource
├── store/              # Zustand stores
├── tests/              # Vitest + RTL tests
├── types/              # Shared TypeScript types (mirrors backend models)
└── utils/              # Pure helper functions (formatters, validators, constants)
```

---

## API Endpoint Mapping

All service modules live in `src/services/api/` and wrap the Rust backend:

| Service file | Backend prefix | Description |
|---|---|---|
| `auth.ts` | `/auth` | Login, logout, current user |
| `profiles.ts` | `/profile` | Health profiles |
| `metrics.ts` | `/metrics` | Metric entries + summary |
| `goals.ts` | `/goals` | Goal CRUD |
| `workOrders.ts` | `/work-orders` | Work-order ticketing + transitions |
| `workflows.ts` | `/workflows` | Templates, instances, approval actions |
| `analytics.ts` | `/analytics` | Reports + XLSX export |
| `notifications.ts` | `/notifications` | In-app notifications + schedules |
| `audit.ts` | `/audit-logs` | Immutable audit log viewer |

---

## Role-Based Access Control

The app enforces the same four-role model as the backend:

| Role | UUID | Visible pages |
|---|---|---|
| Administrator | `00000000-0000-0000-0000-000000000001` | All pages including Audit Logs |
| Care Coach | `00000000-0000-0000-0000-000000000002` | Members, Goals, Work Orders, Analytics |
| Approver | `00000000-0000-0000-0000-000000000003` | Workflow Approvals only |
| Member | `00000000-0000-0000-0000-000000000004` | Own profile, goals, and notifications |

Navigation items and routes are hidden/guarded at the routing layer; the backend enforces
the same restrictions server-side.
