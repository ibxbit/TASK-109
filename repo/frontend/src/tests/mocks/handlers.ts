import { http, HttpResponse } from 'msw';

const BASE = '/api';

// ── Auth handlers ─────────────────────────────────────────────────────────────

const mockUser = {
  id:          '11111111-1111-1111-1111-111111111111',
  username:    'admin',
  role_id:     '00000000-0000-0000-0000-000000000001',
  org_unit_id: null,
  is_active:   true,
  created_at:  '2024-01-01T00:00:00Z',
  updated_at:  '2024-01-01T00:00:00Z',
};

export const handlers = [
  // POST /auth/login — success
  http.post(`${BASE}/auth/login`, async ({ request }) => {
    const body = await request.json() as Record<string, unknown>;
    if (body.username === 'admin' && body.password === 'password') {
      return HttpResponse.json({
        token:      'mock-jwt-token',
        user:       mockUser,
        expires_at: new Date(Date.now() + 30 * 60 * 1000).toISOString(),
      });
    }
    if (body.username === 'locked') {
      return HttpResponse.json(
        { locked: true, locked_until: new Date(Date.now() + 900_000).toISOString(), message: 'Account locked.' },
        { status: 200 },
      );
    }
    if (body.username === 'captcha') {
      return HttpResponse.json(
        { captcha_required: true, captcha_token: 'tok-abc', question: 'What is 3 + 4?' },
        { status: 200 },
      );
    }
    return HttpResponse.json({ message: 'Invalid credentials.' }, { status: 401 });
  }),

  // POST /auth/logout
  http.post(`${BASE}/auth/logout`, () => HttpResponse.json({ message: 'Logged out' })),

  // GET /auth/me
  http.get(`${BASE}/auth/me`, () =>
    HttpResponse.json({ user: mockUser }),
  ),

  // GET /notifications
  http.get(`${BASE}/notifications`, () =>
    HttpResponse.json([]),
  ),

  // GET /metrics
  http.get(`${BASE}/metrics`, () =>
    HttpResponse.json([]),
  ),

  // GET /metrics/summary
  http.get(`${BASE}/metrics/summary`, () =>
    HttpResponse.json([]),
  ),

  // GET /goals
  http.get(`${BASE}/goals`, () =>
    HttpResponse.json([]),
  ),

  // GET /work-orders
  http.get(`${BASE}/work-orders`, () =>
    HttpResponse.json([]),
  ),

  // GET /analytics
  http.get(`${BASE}/analytics`, () =>
    HttpResponse.json({
      query_params:  { org_unit_id: null, start_date: '2024-01-01', end_date: '2024-01-31' },
      member_count:  0,
      metrics:       { attendance: {}, cancellation: {}, conversion: {}, distribution: {}, popularity: {} },
    }),
  ),

  // GET /audit-logs
  http.get(`${BASE}/audit-logs`, () =>
    HttpResponse.json({ items: [], total: 0, page: 1, per_page: 50 }),
  ),

  // GET /notifications/subscriptions
  http.get(`${BASE}/notifications/subscriptions`, () =>
    HttpResponse.json([]),
  ),
];
